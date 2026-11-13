package weilmcp

import (
	"bytes"
	"context"
	"encoding/json"
	"reflect"
	"slices"
	"strings"

	"github.com/modelcontextprotocol/go-sdk/mcp"

	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/client"
)

// AuditField is the result metadata field (_meta) in which Audit returns a
// tool call's audit entry, as the Python SDK's AUDIT_FIELD.
const AuditField = "audit_payload"

// Audit returns, with each successful result of h, an audit entry for the
// call — the twin of the Python SDK's @audit(), including on a result that
// reports a tool error (a handler that returns an error gets no entry):
//
//	mcp.AddTool(server, &mcp.Tool{Name: "lookup_order"},
//		weilmcp.SecuredUser("engg.weil", weilmcp.Audit("", lookupOrder)))
//
// The entry is the JSON object {"caller", "tool", "args"}: the verified
// caller's wallet (see Wallet), the tool's name (name, or the called tool's
// if empty) and the call's arguments. It goes in the result's _meta under
// AuditField; the calling agent submits it to the auditor applet from its
// own wallet (Go: weiladk.AuditMCPResults; Python: agent.audit_mcp_result or
// agent.weil_session). Put Audit inside SecuredUser/SecuredAdmin, so only
// permitted calls produce an entry.
func Audit[In, Out any](name string, h mcp.ToolHandlerFor[In, Out]) mcp.ToolHandlerFor[In, Out] {
	return func(ctx context.Context, req *mcp.CallToolRequest, in In) (*mcp.CallToolResult, Out, error) {
		res, out, err := h(ctx, req, in)
		if err != nil {
			return res, out, err
		}
		if res == nil {
			res = &mcp.CallToolResult{} // the SDK still fills Content from out
		}
		if err := attachAuditEntry(ctx, req, name, res, declaredArgs(in)); err != nil {
			var zero Out
			return nil, zero, err
		}
		return res, out, nil
	}
}

// AuditHandler is Audit for an untyped tool handler (Server.AddTool).
func AuditHandler(name string, h mcp.ToolHandler) mcp.ToolHandler {
	return func(ctx context.Context, req *mcp.CallToolRequest) (*mcp.CallToolResult, error) {
		res, err := h(ctx, req)
		if err != nil || res == nil {
			return res, err
		}
		if err := attachAuditEntry(ctx, req, name, res, nil); err != nil {
			return nil, err
		}
		return res, nil
	}
}

// attachAuditEntry puts the call's audit entry in res's _meta: Python's
// json.dumps({"caller", "tool", "args"}). declared, when non-nil, is the
// handler's decoded arguments (see declaredArgs) and is recorded as is;
// otherwise the request's arguments are, in their order.
func attachAuditEntry(ctx context.Context, req *mcp.CallToolRequest, name string, res *mcp.CallToolResult, declared json.RawMessage) error {
	args := json.RawMessage(`{}`)
	if req != nil && req.Params != nil {
		if name == "" {
			name = req.Params.Name
		}
		if len(req.Params.Arguments) > 0 {
			args = req.Params.Arguments
		}
	}
	if declared != nil {
		args = declared
	}
	ordered, err := reorderObject(args, nil)
	if err != nil {
		return err
	}
	head, err := json.Marshal(struct {
		Caller string `json:"caller"` // "" when there is no verified caller, as in Python
		Tool   string `json:"tool"`
	}{callerWallet(ctx, req), name})
	if err != nil {
		return err
	}
	raw := append(append(head[:len(head)-1], `,"args":`...), ordered...)
	raw = append(raw, '}')
	entry, err := client.PythonDumps(raw, false)
	if err != nil {
		return err
	}
	meta := mcp.Meta{}
	for k, v := range res.Meta {
		meta[k] = v
	}
	meta[AuditField] = string(entry)
	res.Meta = meta
	return nil
}

// reorderObject rewrites the JSON object raw with the keys in order first,
// then the rest in their order in raw.
func reorderObject(raw json.RawMessage, order []string) ([]byte, error) {
	dec := json.NewDecoder(bytes.NewReader(raw))
	tok, err := dec.Token()
	if err != nil {
		return nil, err
	}
	if d, ok := tok.(json.Delim); !ok || d != '{' {
		return raw, nil // not an object: kept as it is
	}
	var keys []string
	vals := map[string]json.RawMessage{}
	for dec.More() {
		kt, err := dec.Token()
		if err != nil {
			return nil, err
		}
		var v json.RawMessage
		if err := dec.Decode(&v); err != nil {
			return nil, err
		}
		k := kt.(string)
		if _, seen := vals[k]; !seen {
			keys = append(keys, k)
		}
		vals[k] = v
	}
	var sorted []string
	for _, k := range order {
		if _, ok := vals[k]; ok {
			sorted = append(sorted, k)
		}
	}
	for _, k := range keys {
		if !slices.Contains(sorted, k) {
			sorted = append(sorted, k)
		}
	}
	var buf bytes.Buffer
	buf.WriteByte('{')
	for i, k := range sorted {
		if i > 0 {
			buf.WriteByte(',')
		}
		kb, _ := json.Marshal(k)
		buf.Write(kb)
		buf.WriteByte(':')
		buf.Write(vals[k])
	}
	buf.WriteByte('}')
	return buf.Bytes(), nil
}

// declaredArgs is a typed handler's decoded arguments as JSON, fields in
// declaration order — the Go counterpart of Python's bound.arguments: the
// declared parameters only, as the handler received them (coerced to their
// types). nil when In is not a struct.
func declaredArgs[In any](in In) json.RawMessage {
	if fieldOrder[In]() == nil {
		return nil
	}
	var buf bytes.Buffer
	enc := json.NewEncoder(&buf)
	enc.SetEscapeHTML(false)
	if enc.Encode(in) != nil {
		return nil
	}
	return bytes.TrimSpace(buf.Bytes())
}

// fieldOrder is the JSON names of In's fields in declaration order, the Go
// counterpart of a Python tool's parameter order; nil if In is no struct.
func fieldOrder[In any]() []string {
	t := reflect.TypeFor[In]()
	for t.Kind() == reflect.Pointer {
		t = t.Elem()
	}
	if t.Kind() != reflect.Struct {
		return nil
	}
	var names []string
	for i := 0; i < t.NumField(); i++ {
		f := t.Field(i)
		if !f.IsExported() {
			continue
		}
		name, _, _ := strings.Cut(f.Tag.Get("json"), ",")
		switch name {
		case "-":
			continue
		case "":
			name = f.Name
		}
		names = append(names, name)
	}
	return names
}

// callerWallet is Wallet(ctx), or, when the tool runs outside the request's
// context, the wallet the request's own auth headers verify to.
func callerWallet(ctx context.Context, req *mcp.CallToolRequest) string {
	if wallet := Wallet(ctx); wallet != "" {
		return wallet
	}
	if req != nil && req.Extra != nil && req.Extra.Header != nil {
		if caller, err := client.VerifyAuthHeaders(req.Extra.Header, maxAge); err == nil {
			return caller.Address
		}
	}
	return ""
}
