// Package claudemodel runs Google ADK (Go, v2) agents on Claude through the
// Anthropic Messages API.
//
// It translates ADK's genai requests into Messages API calls — system
// instruction, function tools, tool calls and results — and Claude's answers
// back into genai content. Thinking blocks are carried through ADK's history as
// thought parts (signature in ThoughtSignature) so they are replayed unchanged
// on the next turn, as the API requires.
package claudemodel

import (
	"context"
	"encoding/base64"
	"encoding/json"
	"fmt"
	"iter"
	"slices"
	"strings"

	"github.com/anthropics/anthropic-sdk-go"
	"github.com/anthropics/anthropic-sdk-go/option"
	"github.com/anthropics/anthropic-sdk-go/packages/param"
	"github.com/anthropics/anthropic-sdk-go/shared/constant"
	"google.golang.org/adk/v2/model"
	"google.golang.org/genai"
)

// DefaultModel is the Claude model used when Config.Model is empty.
const DefaultModel = "claude-opus-5-5"

// redactedPrefix marks a thought part that carries a redacted_thinking block's
// opaque data instead of a signature.
const redactedPrefix = "redacted:"

// Config configures a Claude model — the counterpart of genai.ClientConfig
// in gemini.NewModel. A nil Config uses the defaults.
type Config struct {
	// APIKey for the Anthropic API. Default: the ANTHROPIC_API_KEY
	// environment variable.
	APIKey string
	// MaxTokens caps each response. Default 32000 (requests are streamed,
	// so large values do not risk HTTP timeouts).
	MaxTokens int64
	// Effort trades depth for speed and cost. Default "medium".
	Effort anthropic.BetaOutputConfigEffort
	// DisableFallbacks turns off server-side refusal fallbacks, which are on
	// by default: a request declined by a safety classifier is re-served by
	// a fallback model chosen by refusal category, inside the same call.
	DisableFallbacks bool
	// ClientOptions further configure the Anthropic client (base URL,
	// retries, HTTP client, ...).
	ClientOptions []option.RequestOption
}

type claude struct {
	model  string
	cfg    Config
	client anthropic.Client
}

// NewModel returns an ADK model backed by Claude, mirroring
// gemini.NewModel(ctx, modelName, cfg). An empty modelName uses DefaultModel.
func NewModel(ctx context.Context, modelName string, cfg *Config) (model.LLM, error) {
	c := Config{}
	if cfg != nil {
		c = *cfg
	}
	if modelName == "" {
		modelName = DefaultModel
	}
	if c.MaxTokens <= 0 {
		c.MaxTokens = 32000
	}
	if c.Effort == "" {
		c.Effort = anthropic.BetaOutputConfigEffortMedium
	}
	opts := slices.Clone(c.ClientOptions)
	if c.APIKey != "" {
		opts = append(opts, option.WithAPIKey(c.APIKey))
	}
	return &claude{model: modelName, cfg: c, client: anthropic.NewClient(opts...)}, nil
}

func (c *claude) Name() string { return c.model }

// GenerateContent streams one Messages API call. With stream, text deltas are
// yielded as partial responses before the complete one.
func (c *claude) GenerateContent(ctx context.Context, req *model.LLMRequest, stream bool) iter.Seq2[*model.LLMResponse, error] {
	return func(yield func(*model.LLMResponse, error) bool) {
		params, err := c.params(req)
		if err != nil {
			yield(nil, err)
			return
		}

		s := c.client.Beta.Messages.NewStreaming(ctx, params)
		defer s.Close()
		msg := anthropic.BetaMessage{}
		for s.Next() {
			event := s.Current()
			if err := msg.Accumulate(event); err != nil {
				yield(nil, fmt.Errorf("claudemodel: accumulate stream: %w", err))
				return
			}
			if !stream {
				continue
			}
			if delta, ok := event.AsAny().(anthropic.BetaRawContentBlockDeltaEvent); ok {
				if text, ok := delta.Delta.AsAny().(anthropic.BetaTextDelta); ok && text.Text != "" {
					partial := &model.LLMResponse{
						Content: genai.NewContentFromText(text.Text, genai.RoleModel),
						Partial: true,
					}
					if !yield(partial, nil) {
						return
					}
				}
			}
		}
		if err := s.Err(); err != nil {
			yield(nil, fmt.Errorf("claudemodel: %w", err))
			return
		}
		yield(toLLMResponse(&msg), nil)
	}
}

func (c *claude) params(req *model.LLMRequest) (anthropic.BetaMessageNewParams, error) {
	modelID := c.model
	if req.Model != "" && strings.HasPrefix(req.Model, "claude-") {
		modelID = req.Model
	}

	params := anthropic.BetaMessageNewParams{
		Model:        anthropic.Model(modelID),
		MaxTokens:    c.cfg.MaxTokens,
		OutputConfig: anthropic.BetaOutputConfigParam{Effort: c.cfg.Effort},
		// Agent loops resend a growing, append-only history: cache it.
		CacheControl: anthropic.NewBetaCacheControlEphemeralParam(),
	}
	if !c.cfg.DisableFallbacks {
		params.Betas = append(params.Betas, anthropic.AnthropicBetaServerSideFallback2026_07_01)
		params.Fallbacks = anthropic.BetaFallbacksParamUnion{OfDefault: constant.ValueOf[constant.Default]()}
	}

	if cfg := req.Config; cfg != nil {
		if system := contentText(cfg.SystemInstruction); system != "" {
			params.System = []anthropic.BetaTextBlockParam{{Text: system}}
		}
		tools, err := toTools(cfg.Tools)
		if err != nil {
			return params, err
		}
		params.Tools = tools
		format, err := responseFormat(cfg)
		if err != nil {
			return params, err
		}
		if format != nil {
			params.OutputConfig.Format = anthropic.BetaJSONOutputFormatParam{Schema: format}
		}
	}

	params.Messages = toMessages(req.Contents)
	if len(params.Messages) == 0 {
		return params, fmt.Errorf("claudemodel: request has no messages")
	}
	return params, nil
}

func contentText(c *genai.Content) string {
	if c == nil {
		return ""
	}
	var parts []string
	for _, p := range c.Parts {
		if p != nil && p.Text != "" && !p.Thought {
			parts = append(parts, p.Text)
		}
	}
	return strings.Join(parts, "\n\n")
}

// toTools converts ADK function declarations to Claude tool definitions.
func toTools(tools []*genai.Tool) ([]anthropic.BetaToolUnionParam, error) {
	var out []anthropic.BetaToolUnionParam
	for _, t := range tools {
		if t == nil {
			continue
		}
		for _, fd := range t.FunctionDeclarations {
			schema, err := inputSchema(fd)
			if err != nil {
				return nil, fmt.Errorf("claudemodel: tool %s: %w", fd.Name, err)
			}
			tool := anthropic.BetaToolParam{Name: fd.Name, InputSchema: schema}
			if fd.Description != "" {
				tool.Description = param.NewOpt(fd.Description)
			}
			out = append(out, anthropic.BetaToolUnionParam{OfTool: &tool})
		}
	}
	return out, nil
}

func inputSchema(fd *genai.FunctionDeclaration) (anthropic.BetaToolInputSchemaParam, error) {
	var src any
	switch {
	case fd.ParametersJsonSchema != nil:
		src = fd.ParametersJsonSchema
	case fd.Parameters != nil:
		src = fd.Parameters
	}

	schema := map[string]any{}
	if src != nil {
		raw, err := json.Marshal(src)
		if err != nil {
			return anthropic.BetaToolInputSchemaParam{}, err
		}
		if err := json.Unmarshal(raw, &schema); err != nil {
			return anthropic.BetaToolInputSchemaParam{}, err
		}
		lowercaseTypes(schema) // genai.Schema spells types "OBJECT", "STRING", ...
	}

	out := anthropic.BetaToolInputSchemaParam{Properties: map[string]any{}}
	if props, ok := schema["properties"]; ok && props != nil {
		out.Properties = props
	}
	if req, ok := schema["required"].([]any); ok {
		for _, r := range req {
			if s, ok := r.(string); ok {
				out.Required = append(out.Required, s)
			}
		}
	}
	extra := map[string]any{}
	for k, v := range schema {
		switch k {
		case "type", "properties", "required":
		default:
			extra[k] = v
		}
	}
	if len(extra) > 0 {
		out.ExtraFields = extra
	}
	return out, nil
}

// responseFormat turns the request's response schema (an agent's
// OutputSchema) into a structured-output JSON schema, or nil when there is
// none. Claude otherwise answers in prose and ADK's output validation fails.
func responseFormat(cfg *genai.GenerateContentConfig) (map[string]any, error) {
	var src any
	switch {
	case cfg.ResponseJsonSchema != nil:
		src = cfg.ResponseJsonSchema
	case cfg.ResponseSchema != nil:
		src = cfg.ResponseSchema
	default:
		return nil, nil
	}
	raw, err := json.Marshal(src)
	if err != nil {
		return nil, err
	}
	var schema map[string]any
	if err := json.Unmarshal(raw, &schema); err != nil {
		return nil, err
	}
	lowercaseTypes(schema)
	strictObjects(schema)
	return schema, nil
}

// structuredKeys are the schema keywords kept for structured outputs;
// genai.Schema extras such as propertyOrdering, nullable or example are
// dropped (nullable becomes a "null" type).
var structuredKeys = map[string]bool{
	"type": true, "properties": true, "required": true, "items": true, "enum": true,
	"description": true, "anyOf": true, "additionalProperties": true, "const": true,
	"format": true, "$defs": true, "$ref": true, "title": true, "default": true,
}

// strictObjects drops keywords structured outputs reject and closes every
// object (additionalProperties: false), which they require.
func strictObjects(v any) {
	switch t := v.(type) {
	case map[string]any:
		if n, _ := t["nullable"].(bool); n {
			if ty, ok := t["type"].(string); ok {
				t["type"] = []any{ty, "null"}
			}
		}
		for k := range t {
			if !structuredKeys[k] {
				delete(t, k)
			}
		}
		if t["type"] == "object" {
			if _, ok := t["additionalProperties"]; !ok {
				t["additionalProperties"] = false
			}
		}
		for k, child := range t {
			if k == "properties" || k == "$defs" {
				if m, ok := child.(map[string]any); ok {
					for _, c := range m {
						strictObjects(c)
					}
				}
				continue
			}
			strictObjects(child)
		}
	case []any:
		for _, child := range t {
			strictObjects(child)
		}
	}
}

func lowercaseTypes(v any) {
	switch t := v.(type) {
	case map[string]any:
		for k, child := range t {
			if s, ok := child.(string); ok && k == "type" {
				t[k] = strings.ToLower(s)
				continue
			}
			lowercaseTypes(child)
		}
	case []any:
		for _, child := range t {
			lowercaseTypes(child)
		}
	}
}

// toMessages converts ADK's history to Messages API turns: "model" becomes
// assistant and everything else user, consecutive same-role turns merge, and
// a user turn leads with its tool results.
func toMessages(contents []*genai.Content) []anthropic.BetaMessageParam {
	var msgs []anthropic.BetaMessageParam
	// Calls without an ID (none from Claude) get one, matched to their
	// response by name in order.
	pending := map[string][]string{}
	synthetic := 0

	for _, c := range contents {
		if c == nil {
			continue
		}
		role := anthropic.BetaMessageParamRoleUser
		if c.Role == genai.RoleModel {
			role = anthropic.BetaMessageParamRoleAssistant
		}

		var results, blocks []anthropic.BetaContentBlockParamUnion
		for _, p := range c.Parts {
			if p == nil {
				continue
			}
			switch {
			case p.FunctionCall != nil:
				id := p.FunctionCall.ID
				if id == "" {
					synthetic++
					id = fmt.Sprintf("toolu_adk_%d", synthetic)
					pending[p.FunctionCall.Name] = append(pending[p.FunctionCall.Name], id)
				}
				args := p.FunctionCall.Args
				if args == nil {
					args = map[string]any{}
				}
				blocks = append(blocks, anthropic.NewBetaToolUseBlock(id, args, p.FunctionCall.Name))
			case p.FunctionResponse != nil:
				id := p.FunctionResponse.ID
				if q := pending[p.FunctionResponse.Name]; id == "" && len(q) > 0 {
					id, pending[p.FunctionResponse.Name] = q[0], q[1:]
				}
				content, isError := toolResultContent(p.FunctionResponse.Response)
				results = append(results, anthropic.NewBetaToolResultBlock(id, content, isError))
			case p.Thought:
				// Thinking must be replayed exactly as received; parts
				// without a signature did not come from Claude.
				sig := string(p.ThoughtSignature)
				switch {
				case strings.HasPrefix(sig, redactedPrefix):
					blocks = append(blocks, anthropic.NewBetaRedactedThinkingBlock(strings.TrimPrefix(sig, redactedPrefix)))
				case sig != "" && role == anthropic.BetaMessageParamRoleAssistant:
					blocks = append(blocks, anthropic.NewBetaThinkingBlock(sig, p.Text))
				}
			case p.InlineData != nil:
				blocks = append(blocks, inlineBlock(p.InlineData))
			case p.Text != "":
				blocks = append(blocks, anthropic.NewBetaTextBlock(p.Text))
			}
		}

		content := append(results, blocks...)
		if len(content) == 0 {
			continue
		}
		if n := len(msgs); n > 0 && msgs[n-1].Role == role {
			if role == anthropic.BetaMessageParamRoleUser && len(results) > 0 {
				msgs[n-1].Content = append(results, append(msgs[n-1].Content, blocks...)...)
			} else {
				msgs[n-1].Content = append(msgs[n-1].Content, content...)
			}
			continue
		}
		msgs = append(msgs, anthropic.BetaMessageParam{Role: role, Content: content})
	}

	// The conversation must open with a user turn and, with no prefill on
	// current models, end with one.
	if len(msgs) > 0 && msgs[0].Role != anthropic.BetaMessageParamRoleUser {
		msgs = append([]anthropic.BetaMessageParam{anthropic.NewBetaUserMessage(anthropic.NewBetaTextBlock("Continue."))}, msgs...)
	}
	if n := len(msgs); n > 0 && msgs[n-1].Role != anthropic.BetaMessageParamRoleUser {
		msgs = append(msgs, anthropic.NewBetaUserMessage(anthropic.NewBetaTextBlock("Continue.")))
	}
	return msgs
}

// toolResultContent renders an ADK function response for Claude. ADK reports
// a failed tool as {"error": "..."}.
// inlineBlock turns an inline file part (an uploaded image, a loaded
// artifact) into the content block Claude reads it as: images and PDFs as
// base64, text/* as a plain-text document. Other types become a note naming
// the file, so the model knows it was there.
func inlineBlock(b *genai.Blob) anthropic.BetaContentBlockParamUnion {
	mime, _, _ := strings.Cut(strings.ToLower(b.MIMEType), ";")
	mime = strings.TrimSpace(mime)
	switch {
	case mime == "image/jpg":
		mime = "image/jpeg"
		fallthrough
	case mime == "image/jpeg" || mime == "image/png" || mime == "image/gif" || mime == "image/webp":
		return anthropic.NewBetaImageBlock(anthropic.BetaBase64ImageSourceParam{
			Data: base64.StdEncoding.EncodeToString(b.Data), MediaType: anthropic.BetaBase64ImageSourceMediaType(mime)})
	case mime == "application/pdf":
		return anthropic.NewBetaDocumentBlock(anthropic.BetaBase64PDFSourceParam{Data: base64.StdEncoding.EncodeToString(b.Data)})
	case strings.HasPrefix(mime, "text/") || mime == "application/json":
		return anthropic.NewBetaDocumentBlock(anthropic.BetaPlainTextSourceParam{Data: string(b.Data)})
	}
	name := b.DisplayName
	if name == "" {
		name = "file"
	}
	return anthropic.NewBetaTextBlock(fmt.Sprintf("[%s (%s, %d bytes): this file type cannot be shown to the model]", name, b.MIMEType, len(b.Data)))
}

func toolResultContent(resp map[string]any) (string, bool) {
	if errValue, ok := resp["error"]; ok && len(resp) == 1 {
		return fmt.Sprint(errValue), true
	}
	raw, err := json.Marshal(resp)
	if err != nil {
		return fmt.Sprint(resp), false
	}
	return string(raw), false
}

func toLLMResponse(msg *anthropic.BetaMessage) *model.LLMResponse {
	content := &genai.Content{Role: genai.RoleModel}
	for _, block := range msg.Content {
		switch b := block.AsAny().(type) {
		case anthropic.BetaTextBlock:
			content.Parts = append(content.Parts, genai.NewPartFromText(b.Text))
		case anthropic.BetaThinkingBlock:
			content.Parts = append(content.Parts, &genai.Part{Text: b.Thinking, Thought: true, ThoughtSignature: []byte(b.Signature)})
		case anthropic.BetaRedactedThinkingBlock:
			content.Parts = append(content.Parts, &genai.Part{Thought: true, ThoughtSignature: []byte(redactedPrefix + b.Data)})
		case anthropic.BetaToolUseBlock:
			args := map[string]any{}
			if raw := b.JSON.Input.Raw(); raw != "" {
				_ = json.Unmarshal([]byte(raw), &args)
			}
			content.Parts = append(content.Parts, &genai.Part{FunctionCall: &genai.FunctionCall{ID: b.ID, Name: b.Name, Args: args}})
		}
	}

	resp := &model.LLMResponse{
		Content:      content,
		TurnComplete: true,
		ModelVersion: string(msg.Model),
		UsageMetadata: &genai.GenerateContentResponseUsageMetadata{
			PromptTokenCount:        int32(msg.Usage.InputTokens + msg.Usage.CacheReadInputTokens + msg.Usage.CacheCreationInputTokens),
			CachedContentTokenCount: int32(msg.Usage.CacheReadInputTokens),
			CandidatesTokenCount:    int32(msg.Usage.OutputTokens),
			TotalTokenCount:         int32(msg.Usage.InputTokens + msg.Usage.CacheReadInputTokens + msg.Usage.CacheCreationInputTokens + msg.Usage.OutputTokens),
		},
		CustomMetadata: map[string]any{"anthropic_stop_reason": string(msg.StopReason)},
	}

	switch msg.StopReason {
	case anthropic.BetaStopReasonMaxTokens:
		resp.FinishReason = genai.FinishReasonMaxTokens
	case anthropic.BetaStopReasonRefusal:
		resp.FinishReason = genai.FinishReasonSafety
		resp.ErrorCode = "refusal"
		resp.ErrorMessage = msg.StopDetails.Explanation
	default:
		resp.FinishReason = genai.FinishReasonStop
	}
	return resp
}
