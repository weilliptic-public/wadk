// Command knowledge is adk_knowledge_agent: a search agent over an ADK
// knowledge base in Vertex AI Search, which appends the retrieved citations
// to its answers. The Python module has no main; this serves the agent with
// ADK's launcher, as `adk run` / `adk web` load the module:
//
//	go run ./examples/ported/adk_team/knowledge console
//
// VertexAiSearchTool is Gemini's built-in retrieval: as in Python, it runs
// only when WEIL_ADK_TEAM_MODEL is a Gemini model.
package main

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"strings"

	"google.golang.org/adk/v2/agent"
	"google.golang.org/adk/v2/agent/llmagent"
	"google.golang.org/adk/v2/model"
	"google.golang.org/adk/v2/tool"
	"google.golang.org/genai"

	"github.com/weilliptic-public/wadk/adk/go/weil_adk/examples/ported/adk_team/internal/team"
)

// citationRetrievalAfterModelCallback is
// citation_retrieval_after_model_callback: it appends "References:" and one
// JSON citation per retrieved grounding chunk to the model's reply.
func citationRetrievalAfterModelCallback(_ agent.Context, resp *model.LLMResponse, respErr error) (*model.LLMResponse, error) {
	if resp == nil || respErr != nil {
		return nil, nil
	}
	gm := resp.GroundingMetadata
	if gm == nil {
		return nil, nil
	}
	content := resp.Content
	if content == nil || len(content.Parts) == 0 {
		return nil, nil
	}

	var citations []*genai.Part
	for _, chunk := range gm.GroundingChunks {
		if chunk == nil || chunk.RetrievedContext == nil {
			continue
		}
		rc := chunk.RetrievedContext
		citation := fmt.Sprintf(`{"title": %s, "uri": %s, "snippet": %s}`,
			dumps(rc.Title), dumps(rc.URI), dumps(rc.Text))
		citations = append(citations, &genai.Part{Text: citation})
	}
	if len(citations) == 0 {
		return nil, nil
	}

	role := content.Role
	if role == "" {
		role = "model"
	}
	parts := append([]*genai.Part{}, content.Parts...)
	parts = append(parts, &genai.Part{Text: "References:\n"})
	parts = append(parts, citations...)
	out := *resp
	out.Content = &genai.Content{Role: role, Parts: parts}
	return &out, nil
}

// dumps is json.dumps of a string: null for "" (Python's None for an
// unset field), non-ASCII escaped, HTML characters kept.
func dumps(s string) string {
	if s == "" {
		return "null"
	}
	var buf bytes.Buffer
	enc := json.NewEncoder(&buf)
	enc.SetEscapeHTML(false)
	_ = enc.Encode(s)
	var b strings.Builder
	for _, r := range strings.TrimSuffix(buf.String(), "\n") {
		switch {
		case r < 0x80:
			b.WriteRune(r)
		case r > 0xffff:
			r -= 0x10000
			fmt.Fprintf(&b, `\u%04x\u%04x`, 0xd800+(r>>10), 0xdc00+(r&0x3ff))
		default:
			fmt.Fprintf(&b, `\u%04x`, r)
		}
	}
	return b.String()
}

func main() {
	ctx := context.Background()
	team.LoadDotEnv("")
	dataStoreID := team.Getenv("VERTEXAI_DATASTORE_ID", "")
	if dataStoreID == "" {
		team.Fatalf("ValueError: VERTEXAI_DATASTORE_ID environment variable not set. Set it to the" +
			" full resource name of your own Vertex AI Search data store, as" +
			" projects/.../locations/.../collections/.../dataStores/... .")
	}

	root := team.RootAgent(ctx, llmagent.Config{
		Model:       team.Model(ctx),
		Name:        "adk_knowledge_agent",
		Description: "Agent for performing Vertex AI Search to find ADK knowledge and" + " documentation",
		Instruction: `You are a specialized search agent for an ADK knowledge base.

      You can use the VertexAiSearchTool to search for ADK examples and documentation in the document store.
      `,
		Tools:               []tool.Tool{team.VertexAISearch(dataStoreID)},
		AfterModelCallbacks: []llmagent.AfterModelCallback{citationRetrievalAfterModelCallback},
	})
	team.Launch(ctx, root)
}
