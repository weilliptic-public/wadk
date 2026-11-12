package weiladk

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"strings"
	"time"

	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/client"
	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/secured"
)

// AgentMethod is a method an agent serves to other agents over HTTP (see
// AgentHTTPHandler): Python's public methods of a @weil_ai.agent object,
// called with positional args and keyword kwargs.
type AgentMethod func(ctx context.Context, args []any, kwargs map[string]any) (any, error)

// invokeRequest is the body of POST /invoke.
type invokeRequest struct {
	Method string         `json:"method"`
	Args   []any          `json:"args"`
	Kwargs map[string]any `json:"kwargs"`
}

// AgentHTTPHandler serves methods at POST /invoke — the Python SDK's
// agent_http_app(agent, service=...) / agent.to_secure_http(service=...). The
// body is {"method": ..., "args": [...], "kwargs": {...}} and the reply
// {"result": ...}. As in Python:
//
//   - a body that is not JSON is 400 {"error": "Invalid JSON body"}, one that
//     is not an object 400 {"error": "Invalid request body"};
//   - the Weil auth headers must verify and be bound to the body (any Weil
//     SDK's AuthHeadersForBody / get_auth_headers(body=...)), else 401
//     {"error": "Wallet address verification failed"};
//   - a method name starting with "_" is 400 "Invalid method", an unknown
//     one 404 "Method not found";
//   - with WithService, the caller must hold the Execution or Management
//     purpose on it, else 403 with the refusal (or "permission check for
//     service '<s>' failed: <err>"), before any method runs;
//   - a method refusing the caller (secured.ErrNotPermitted) is 403 with its
//     error; any other method error is a 500.
//
// The method runs with the verified caller in its context, where
// secured.Require and secured.CallerFromContext find it, checked against
// id's Sentinel. Of opts only WithService and WithMaxAge apply.
func AgentHTTPHandler(id *Identity, methods map[string]AgentMethod, opts ...A2AOption) http.Handler {
	o := newA2AOptions(opts)
	checker := secured.NewChecker(secured.UsingSentinel(id.client.Sentinel()), secured.PublishVerdictsAs(id.client))
	// The service gate only reads permissions, as Python's wallet_has_purpose.
	gate := secured.NewChecker(secured.UsingSentinel(id.client.Sentinel()), secured.WithoutVerdicts())
	mux := http.NewServeMux()
	mux.HandleFunc("POST /invoke", func(w http.ResponseWriter, r *http.Request) {
		raw, err := io.ReadAll(r.Body)
		if err != nil || !json.Valid(raw) {
			writeJSON(w, http.StatusBadRequest, map[string]any{"error": "Invalid JSON body"})
			return
		}
		var obj map[string]json.RawMessage
		if json.Unmarshal(raw, &obj) != nil || obj == nil {
			writeJSON(w, http.StatusBadRequest, map[string]any{"error": "Invalid request body"})
			return
		}
		caller, err := client.VerifyAuthRequest(r.Header, json.RawMessage(raw), o.maxAge)
		if err != nil {
			writeJSON(w, http.StatusUnauthorized, map[string]any{"error": "Wallet address verification failed"})
			return
		}
		// As Python: a method that is not a string, or starts with "_", is
		// invalid; any other unknown name (even "") is not found.
		var name string
		if json.Unmarshal(obj["method"], &name) != nil || strings.HasPrefix(name, "_") {
			writeJSON(w, http.StatusBadRequest, map[string]any{"error": "Invalid method"})
			return
		}
		var req invokeRequest
		_ = json.Unmarshal(raw, &req)
		method, ok := methods[req.Method]
		if !ok {
			writeJSON(w, http.StatusNotFound, map[string]any{"error": "Method not found"})
			return
		}
		if o.service != "" {
			_, err := gate.Grant(r.Context(), caller.Address, o.service, gatePurposes...)
			switch {
			case errors.Is(err, secured.ErrNotPermitted):
				writeJSON(w, http.StatusForbidden, map[string]any{"error": err.Error()})
				return
			case err != nil:
				writeJSON(w, http.StatusForbidden, map[string]any{
					"error": fmt.Sprintf("permission check for service '%s' failed: %v", o.service, rootCause(err))})
				return
			}
		}
		if req.Args == nil {
			req.Args = []any{}
		}
		if req.Kwargs == nil {
			req.Kwargs = map[string]any{}
		}
		ctx := secured.ContextWithChecker(secured.ContextWithCaller(ContextWithIdentity(r.Context(), id), caller), checker)
		result, err := method(ctx, req.Args, req.Kwargs)
		switch {
		case errors.Is(err, secured.ErrNotPermitted):
			writeJSON(w, http.StatusForbidden, map[string]any{"error": err.Error()})
		case err != nil:
			writePlain(w, http.StatusInternalServerError, "Internal Server Error")
		default:
			writeJSON(w, http.StatusOK, map[string]any{"result": result})
		}
	})
	return mux
}

// writeJSON encodes v before writing the status, so a value that cannot be
// encoded is a 500, as in Starlette.
func writeJSON(w http.ResponseWriter, status int, v any) {
	b, err := json.Marshal(v)
	if err != nil {
		writePlain(w, http.StatusInternalServerError, "Internal Server Error")
		return
	}
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)
	_, _ = w.Write(b)
}

// RemoteAgent calls the methods of an agent served with AgentHTTPHandler (or
// Python's agent_http_app), signing each request as its caller — Python's
// agent.remote(base_url).
type RemoteAgent struct {
	caller  *Identity
	baseURL string
	http    *http.Client
}

// Remote returns a RemoteAgent for the agent served at baseURL, called as i.
func (i *Identity) Remote(baseURL string) *RemoteAgent {
	return &RemoteAgent{caller: i, baseURL: strings.TrimRight(baseURL, "/"), http: &http.Client{
		Timeout: 60 * time.Second,
		// A redirect is an error, as with httpx's raise_for_status.
		CheckRedirect: func(*http.Request, []*http.Request) error { return http.ErrUseLastResponse },
	}}
}

// Call invokes method with args and kwargs (either may be nil) and returns
// its result. A non-2xx reply is an error carrying the status and body, as
// Python's raise_for_status.
func (r *RemoteAgent) Call(ctx context.Context, method string, args []any, kwargs map[string]any) (any, error) {
	if args == nil {
		args = []any{}
	}
	if kwargs == nil {
		kwargs = map[string]any{}
	}
	payload := invokeRequest{Method: method, Args: args, Kwargs: kwargs}
	headers, err := r.caller.client.AuthHeadersForBody(ctx, payload)
	if err != nil {
		return nil, err
	}
	body, err := json.Marshal(payload)
	if err != nil {
		return nil, err
	}
	req, err := http.NewRequestWithContext(ctx, http.MethodPost, r.baseURL+"/invoke", bytes.NewReader(body))
	if err != nil {
		return nil, err
	}
	req.Header.Set("Content-Type", "application/json")
	for k, v := range headers {
		req.Header.Set(k, v)
	}
	resp, err := r.http.Do(req)
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()
	raw, _ := io.ReadAll(resp.Body)
	if resp.StatusCode < 200 || resp.StatusCode >= 300 {
		return nil, fmt.Errorf("remote agent %s %s: HTTP %d: %s", r.baseURL, method, resp.StatusCode, strings.TrimSpace(string(raw)))
	}
	var out struct {
		Result any `json:"result"`
	}
	if err := json.Unmarshal(raw, &out); err != nil {
		return nil, err
	}
	return out.Result, nil
}
