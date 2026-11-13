// Package weilmcp secures MCP servers built with the official MCP Go SDK
// (github.com/modelcontextprotocol/go-sdk) by WeilChain wallet permissions —
// the Go twin of the Python SDK's weil_ai.mcp:
//
//	Python (weil_ai)                         Go (weilmcp)
//	app.add_middleware(weil_middleware())    weilmcp.Middleware(handler, opts...)
//	@secured_user("engg.weil")               mcp.AddTool(s, t, weilmcp.SecuredUser("engg.weil", h))
//	@secured_admin("engg.weil")              mcp.AddTool(s, t, weilmcp.SecuredAdmin("engg.weil", h))
//	current_wallet_addr()                    weilmcp.Wallet(ctx)
//	build_auth_headers(wallet) (client)      weilmcp.HTTPClient(c) in mcp.StreamableClientTransport
//
// Go and Python clients and servers interoperate: both sign and verify the
// same X-Wallet-Address / X-Signature / X-Message / X-Timestamp headers.
package weilmcp

import (
	"context"
	"errors"
	"net/http"

	"github.com/modelcontextprotocol/go-sdk/mcp"

	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/client"
	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/secured"
)

// Middleware verifies the Weil auth headers of every POST to the MCP
// endpoint (401 when they fail) and configures the permission checks the
// secured tools behind it run, e.g.
//
//	handler := mcp.NewStreamableHTTPHandler(func(*http.Request) *mcp.Server { return server }, nil)
//	http.ListenAndServe(addr, weilmcp.Middleware(handler, secured.WithSentinel(client.WithSentinelHost(host))))
func Middleware(next http.Handler, opts ...secured.Option) http.Handler {
	checker := secured.NewChecker(opts...)
	setDefaultChecker(checker)
	return secured.Middleware(next, append(opts, secured.WithChecker(checker))...)
}

// SecuredUser gates a tool on the caller's wallet holding the Execution or
// Management purpose in scope ("subgroup.org" or "org"). A caller without it
// gets an error result (IsError) with the reason, and the tool does not run.
func SecuredUser[In, Out any](scope string, h mcp.ToolHandlerFor[In, Out]) mcp.ToolHandlerFor[In, Out] {
	return securedFor(scope, secured.User, h)
}

// SecuredAdmin gates a tool on the caller's wallet holding the Management
// purpose in scope.
func SecuredAdmin[In, Out any](scope string, h mcp.ToolHandlerFor[In, Out]) mcp.ToolHandlerFor[In, Out] {
	return securedFor(scope, secured.Admin, h)
}

// Secured gates an untyped tool handler (Server.AddTool) at level.
func Secured(scope string, level secured.Level, h mcp.ToolHandler) mcp.ToolHandler {
	mustScope(scope)
	return func(ctx context.Context, req *mcp.CallToolRequest) (*mcp.CallToolResult, error) {
		ctx, denied, err := gate(ctx, req, scope, level)
		if denied != nil || err != nil {
			return denied, err
		}
		return h(ctx, req)
	}
}

func securedFor[In, Out any](scope string, level secured.Level, h mcp.ToolHandlerFor[In, Out]) mcp.ToolHandlerFor[In, Out] {
	mustScope(scope)
	return func(ctx context.Context, req *mcp.CallToolRequest, in In) (*mcp.CallToolResult, Out, error) {
		ctx, denied, err := gate(ctx, req, scope, level)
		if denied != nil || err != nil {
			var zero Out
			return denied, zero, err
		}
		return h(ctx, req, in)
	}
}

// Wallet returns the verified caller's account address inside a secured
// tool, or "" (Python: current_wallet_addr).
func Wallet(ctx context.Context) string {
	return secured.WalletFromContext(ctx)
}

// HTTPClient returns an http.Client that signs every request with c's
// wallet, for mcp.StreamableClientTransport{HTTPClient: ...} — or for ADK's
// mcptoolset — talking to a Weil-secured MCP server (Go or Python).
func HTTPClient(c *client.WeilClient) *http.Client {
	return c.AuthHTTPClient()
}

// gate finds the verified caller — from Middleware's context, else by
// verifying the headers of the HTTP request that carried this call, so a
// tool stays protected even if the middleware is missing — and checks it.
func gate(ctx context.Context, req *mcp.CallToolRequest, scope string, level secured.Level) (context.Context, *mcp.CallToolResult, error) {
	if _, ok := secured.CallerFromContext(ctx); !ok && req != nil && req.Extra != nil && req.Extra.Header != nil {
		if caller, err := client.VerifyAuthHeaders(req.Extra.Header, maxAge); err == nil {
			ctx = secured.ContextWithCaller(ctx, caller)
		}
	}
	if !hasChecker(ctx) {
		ctx = secured.ContextWithChecker(ctx, currentDefault())
	}
	err := secured.Require(ctx, scope, level)
	var npe *secured.NotPermittedError
	switch {
	case err == nil:
		return ctx, nil, nil
	case errors.As(err, &npe):
		res := &mcp.CallToolResult{IsError: true}
		res.Content = []mcp.Content{&mcp.TextContent{Text: err.Error()}}
		return ctx, res, nil
	default:
		return ctx, nil, err
	}
}

const maxAge = client.MaxAuthAge

func mustScope(scope string) {
	if _, err := secured.ParseScope(scope); err != nil {
		panic(err)
	}
}
