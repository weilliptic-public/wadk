package weiladk

import (
	"context"
	"log/slog"

	"github.com/modelcontextprotocol/go-sdk/mcp"

	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/client"
)

// MCPAuditField is the MCP result metadata field (_meta) in which a Weil MCP
// server returns a tool call's audit entry: weilmcp.AuditField in Go,
// AUDIT_FIELD in the Python SDK.
const MCPAuditField = "audit_payload"

// AuditMCPResults returns MCP client middleware that submits the audit entry
// a Weil MCP server returns with a tool result (weilmcp.Audit, or Python's
// @audit()) to the auditor applet, signed by id — the twin of the Python
// SDK's agent.weil_session(session). Give each agent's MCP client its own
// wallet, and pass the client to ADK's mcptoolset:
//
//	mcpClient := mcp.NewClient(&mcp.Implementation{Name: "shop", Version: "1.0.0"}, nil)
//	mcpClient.AddSendingMiddleware(weiladk.AuditMCPResults(shopID))
//	tools, err := mcptoolset.New(mcptoolset.Config{Client: mcpClient, Transport: transport})
//
// The entry is queued on the dispatcher (see WithFlushAfterRun), so the tool
// call does not wait for it; a failure is logged and never fails the call.
func AuditMCPResults(id *Identity) mcp.Middleware {
	return func(next mcp.MethodHandler) mcp.MethodHandler {
		return func(ctx context.Context, method string, req mcp.Request) (mcp.Result, error) {
			res, err := next(ctx, method, req)
			if err != nil || method != "tools/call" || id == nil {
				return res, err
			}
			result, ok := res.(*mcp.CallToolResult)
			if !ok || result == nil {
				return res, nil
			}
			entry, _ := result.Meta[MCPAuditField].(string)
			if entry == "" {
				return res, nil
			}
			tool := ""
			if p, ok := req.GetParams().(*mcp.CallToolParams); ok {
				tool = p.Name
			}
			report := func(err error) {
				slog.Warn("weiladk mcp audit failed", "tool", tool, "wallet", id.Address(), "error", err)
			}
			// As the Python SDK's weil_session: agent.audit(entry) --
			// {"log", "org", "subgroup"}, arguments public. It runs in the
			// background (submitAudit), so the result is not held up, and
			// waits for the auditor to run it, resending an entry the
			// platform loses. One that fails for good is reported, not
			// raised (Python's default raise_audit_errors=False).
			submitAudit(id, Call{Applet: Auditor, Method: "audit", Args: pyArgs(client.AuditArgs(entry, id.Org())), Retry: client.DurableRetry}, report)
			return res, nil
		}
	}
}
