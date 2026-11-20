package client

import (
	"bytes"
	"context"
	"encoding/json"
	"net/http"
)

// Trace propagation headers, shared with the Python SDK (weil_ai.auth). The
// trace context itself rides inside the signed message (X-Message), as
// {"trace": {"trace_id": ..., "parent_id": ...}}; these headers only mirror
// it, for debugging, and are not trusted on their own.
const (
	HeaderTraceID       = "X-Weil-Trace-Id"
	HeaderTraceParentID = "X-Weil-Trace-Parent-Id"
)

// TraceContext is the trace a remote call belongs to: the caller's trace and
// the node the callee's events go under ("" for none). An agent calling
// another signs it into its auth headers, so the callee records its events
// into the caller's trace instead of starting one of its own.
type TraceContext struct {
	TraceID  string
	ParentID string
}

type traceContextKey struct{}

// WithTraceContext returns ctx carrying tc, which AuthHeaders,
// AuthHeadersForBody and AuthTransport then sign into the request's auth
// headers. A nil tc, or one without a TraceID, leaves ctx as it is.
func WithTraceContext(ctx context.Context, tc *TraceContext) context.Context {
	if tc == nil || tc.TraceID == "" {
		return ctx
	}
	return context.WithValue(ctx, traceContextKey{}, tc)
}

// TraceContextFrom returns the trace context ctx carries (see
// WithTraceContext), or nil.
func TraceContextFrom(ctx context.Context) *TraceContext {
	tc, _ := ctx.Value(traceContextKey{}).(*TraceContext)
	return tc
}

// signed is tc as it rides in the signed message.
func (tc *TraceContext) signed() map[string]any {
	m := map[string]any{"trace_id": tc.TraceID}
	if tc.ParentID != "" {
		m["parent_id"] = tc.ParentID
	}
	return m
}

// TraceContextFromHeaders returns the trace context signed into the auth
// headers h, or nil — Python's trace_context_from_headers. Call it only once
// the headers have verified (VerifyAuthHeaders also sets AuthCaller.Trace).
// A mirror header (HeaderTraceID, HeaderTraceParentID) that disagrees with
// the signed context discards it.
func TraceContextFromHeaders(h http.Header) *TraceContext {
	message := h.Get(HeaderMessage)
	if message == "" {
		return nil
	}
	// Numbers are kept as written, so a numeric ID reads as Python's str()
	// of it.
	dec := json.NewDecoder(bytes.NewReader([]byte(message)))
	dec.UseNumber()
	var signed map[string]any
	if dec.Decode(&signed) != nil {
		return nil
	}
	tc := normalizeTraceContext(signed["trace"])
	if tc == nil {
		return nil
	}
	if v := h.Values(HeaderTraceID); len(v) > 0 && v[0] != tc.TraceID {
		return nil
	}
	if v := h.Values(HeaderTraceParentID); len(v) > 0 && v[0] != tc.ParentID {
		return nil
	}
	return tc
}

// normalizeTraceContext reads a signed "trace" object: a non-empty trace_id
// and an optional parent_id, each a string or number. Anything else is no
// context.
func normalizeTraceContext(v any) *TraceContext {
	m, ok := v.(map[string]any)
	if !ok {
		return nil
	}
	traceID, ok := traceIDString(m["trace_id"])
	if !ok || traceID == "" {
		return nil
	}
	tc := &TraceContext{TraceID: traceID}
	if parent, ok := traceIDString(m["parent_id"]); ok {
		tc.ParentID = parent
	}
	return tc
}

func traceIDString(v any) (string, bool) {
	switch v := v.(type) {
	case string:
		return v, true
	case json.Number:
		return v.String(), true
	}
	return "", false
}
