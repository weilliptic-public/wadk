package weiladk

import (
	"context"
	"reflect"
	"sync"
	"time"
	"unsafe"

	"google.golang.org/adk/v2/agent"
	"google.golang.org/adk/v2/tool"
	"google.golang.org/adk/v2/tool/functiontool"

	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/secured"
)

// SecuredUser gates an ADK function tool on the wallet of the agent running
// it holding the Execution or Management purpose in scope ("subgroup.org" or
// "org"):
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

// SecuredAdmin gates an ADK function tool on the wallet of the agent running
// it holding the Management purpose in scope, as SecuredUser does for
// Execution or Management.
func SecuredAdmin[In, Out any](scope string, fn functiontool.Func[In, Out]) functiontool.Func[In, Out] {
	return securedTool(scope, secured.Admin, fn)
}

func securedTool[In, Out any](scope string, level secured.Level, fn functiontool.Func[In, Out]) functiontool.Func[In, Out] {
	if err := secured.ValidateRule(scope); err != nil {
		panic(err)
	}
	access := "user"
	if level == secured.Admin {
		access = "admin"
	}
	f := functiontool.Func[In, Out](func(ctx agent.Context, in In) (Out, error) {
		// A WithSecureA2A plugin that recognised the tool (securedToolOf) has
		// written adk.tool.before already. Otherwise write it now, before the
		// check and the tool's own calls.
		if ctx != nil && !securedCallAudited(ctx.FunctionCallID()) {
			sc := securedCall{service: scope, access: access, at: time.Now()}
			sc.beforeAudited = auditToolBefore(ctx.FunctionCallID(), sc)
			rememberSecuredCall(ctx.FunctionCallID(), sc)
		}
		if err := requireActing(ctx, scope, level); err != nil {
			var zero Out
			return zero, err
		}
		return fn(ctx, in)
	})
	securedFuncs.Store(funcIdentity(&f), securedCall{service: scope, access: access})
	return f
}

// securedFuncs holds the scope and access of each function SecuredUser and
// SecuredAdmin returned, by its closure, so a plugin can tell a secured
// function tool before it runs.
var securedFuncs sync.Map // closure pointer → securedCall

// funcIdentity is the closure a func value points to, which tells two
// closures of the same function literal apart.
func funcIdentity[F any](f *F) unsafe.Pointer {
	return *(*unsafe.Pointer)(unsafe.Pointer(f))
}

// securedToolOf returns the scope and access of t if it is a function tool
// (functiontool.New) built on SecuredUser or SecuredAdmin.
func securedToolOf(t tool.Tool) (securedCall, bool) {
	v := reflect.ValueOf(t)
	if !v.IsValid() || v.Kind() != reflect.Pointer || v.IsNil() || v.Elem().Kind() != reflect.Struct {
		return securedCall{}, false
	}
	h := v.Elem().FieldByName("handler")
	if !h.IsValid() || h.Kind() != reflect.Func {
		return securedCall{}, false
	}
	// The field is unexported; read the closure pointer through its address.
	sc, ok := securedFuncs.Load(*(*unsafe.Pointer)(h.Addr().UnsafePointer()))
	if !ok {
		return securedCall{}, false
	}
	return sc.(securedCall), true
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
// access level ("user"/"admin"), and when it started.
type securedCall struct {
	service, access string
	at              time.Time
	// beforeAudited is set once adk.tool.before has been written.
	beforeAudited bool
}

// securedCalls holds the secured tool calls in flight, by ADK function call
// ID, until a plugin with WithSecureA2A audits them after the call. The
// plugin records the calls it recognises as they start (securedToolOf); a
// secured function it cannot see into records itself as it runs.
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

// securedCallAudited reports whether the call id has its adk.tool.before.
func securedCallAudited(id string) bool {
	v, ok := securedCalls.Load(id)
	return ok && v.(securedCall).beforeAudited
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
