package weiladk

import (
	"context"
	"sync"
	"time"

	"google.golang.org/adk/v2/agent"
	"google.golang.org/adk/v2/tool/functiontool"

	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/secured"
)

// SecuredUser gates an ADK function tool on the wallet of the agent running
// it holding the Execution or Management purpose in scope ("subgroup.org" or
// "org") — the twin of Python's @secured_user on an ADK tool:
//
//	search, _ := functiontool.New(functiontool.Config{Name: "search_flights"},
//		weiladk.SecuredUser("engg.weil", searchFlights))
//
// The tool authorizes as the acting agent (IdentityOf): its own identity, or
// that of its nearest bound ancestor. A remote A2A caller is checked once,
// at SecureA2A's service gate, not again here. A refusal is a
// *secured.NotPermittedError, which reaches the model as the tool's error.
func SecuredUser[In, Out any](scope string, fn functiontool.Func[In, Out]) functiontool.Func[In, Out] {
	return securedTool(scope, secured.User, fn)
}

// SecuredAdmin gates an ADK function tool on its caller holding the
// Management purpose in scope (Python: @secured_admin).
func SecuredAdmin[In, Out any](scope string, fn functiontool.Func[In, Out]) functiontool.Func[In, Out] {
	return securedTool(scope, secured.Admin, fn)
}

func securedTool[In, Out any](scope string, level secured.Level, fn functiontool.Func[In, Out]) functiontool.Func[In, Out] {
	if _, err := secured.ParseScope(scope); err != nil {
		panic(err)
	}
	access := "user"
	if level == secured.Admin {
		access = "admin"
	}
	return func(ctx agent.Context, in In) (Out, error) {
		if ctx != nil {
			sc := securedCall{service: scope, access: access, at: time.Now()}
			// Under a WithSecureA2A plugin, write adk.tool.before now, before
			// the check and the tool's own calls, as Python's before-tool
			// callback does.
			sc.beforeAudited = auditToolBefore(ctx.FunctionCallID(), sc)
			rememberSecuredCall(ctx.FunctionCallID(), sc)
		}
		if err := requireActing(ctx, scope, level); err != nil {
			var zero Out
			return zero, err
		}
		return fn(ctx, in)
	}
}

// requireActing checks the acting agent's wallet against scope and level,
// with the Checker SecureA2A put in ctx or else one that asks Sentinel, and
// publishes its verdict, as that agent.
func requireActing(ctx context.Context, scope string, level secured.Level) error {
	id := IdentityOf(ctx)
	if id == nil {
		return &secured.NotPermittedError{Service: scope, Level: level}
	}
	checker := secured.CheckerFromContext(ctx)
	if checker == nil {
		checker = localChecker(id)
	}
	return checker.Check(ctx, id.Address(), scope, level)
}

// securedCall is what a secured tool's audit records name: its scope and
// access level ("user"/"admin", Python's secured_info), and when it started.
type securedCall struct {
	service, access string
	at              time.Time
	// beforeAudited is set once adk.tool.before has been written.
	beforeAudited bool
}

// securedCalls holds the secured tool calls in flight, by ADK function call
// ID, until a plugin with WithSecureA2A audits them after the call. ADK-Go
// does not let a plugin tell a secured function tool from another before it
// runs, so the tool itself says so.
var (
	securedCalls   sync.Map // function call ID → securedCall
	securedSweepMu sync.Mutex
	securedSweepAt time.Time
)

func rememberSecuredCall(id string, c securedCall) {
	if id == "" {
		return
	}
	securedCalls.Store(id, c)
	// Without such a plugin nothing takes the entries back out: drop stale
	// ones now and then.
	securedSweepMu.Lock()
	defer securedSweepMu.Unlock()
	if time.Since(securedSweepAt) < time.Minute {
		return
	}
	securedSweepAt = time.Now()
	securedCalls.Range(func(k, v any) bool {
		if time.Since(v.(securedCall).at) > 10*time.Minute {
			securedCalls.Delete(k)
		}
		return true
	})
}

func takeSecuredCall(id string) (securedCall, bool) {
	v, ok := securedCalls.LoadAndDelete(id)
	if !ok {
		return securedCall{}, false
	}
	return v.(securedCall), true
}

// localCheckers holds one Checker per identity, so its permission cache
// outlives a single tool call.
var localCheckers sync.Map // *Identity → *secured.Checker

// localChecker returns the Checker for checks made as id: it asks id's
// Sentinel and publishes verdicts as id.
func localChecker(id *Identity) *secured.Checker {
	if c, ok := localCheckers.Load(id); ok {
		return c.(*secured.Checker)
	}
	c, _ := localCheckers.LoadOrStore(id, secured.NewChecker(
		secured.UsingSentinel(id.client.Sentinel()),
		secured.PublishVerdictsAs(id.client)))
	return c.(*secured.Checker)
}
