package weilmcp

import (
	"context"
	"sync/atomic"

	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/secured"
)

// The checker the last Middleware configured, for tool calls whose context
// does not descend from the HTTP request (the MCP SDK may run them on the
// session's context).
var defaultChecker atomic.Pointer[secured.Checker]

func setDefaultChecker(c *secured.Checker) { defaultChecker.Store(c) }

func currentDefault() *secured.Checker {
	if c := defaultChecker.Load(); c != nil {
		return c
	}
	return secured.NewChecker()
}

// SetChecker sets the Checker secured tools use when no Middleware
// configured one, e.g. for a server mounted without Middleware.
func SetChecker(c *secured.Checker) { setDefaultChecker(c) }

func hasChecker(ctx context.Context) bool { return secured.CheckerFromContext(ctx) != nil }
