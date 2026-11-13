package weiladk

import (
	"fmt"
	"time"
)

// Mode decides whether a hook waits for an action's applet call.
type Mode int

const (
	// NonBlocking queues the call on the plugin's Dispatcher and returns from
	// the hook at once. The agent never waits on the chain; results and
	// failures arrive later through OnResult and PluginConfig.OnError.
	NonBlocking Mode = iota
	// Blocking runs the call inside the hook and waits for it. The action can
	// then veto the hook (see OnResult and FailClosed) — e.g. deny a tool
	// call that an applet does not authorize.
	Blocking
)

func (m Mode) String() string {
	if m == Blocking {
		return "blocking"
	}
	return "non-blocking"
}

// FailurePolicy decides what a Blocking action's failure does to its hook. A
// failure is a Build error, an unresolvable identity, or a failed call.
type FailurePolicy int

const (
	// FailOpen reports the failure to PluginConfig.OnError and lets the
	// hook proceed.
	FailOpen FailurePolicy = iota
	// FailClosed fails the hook with an *ActionError. For before-hooks this
	// stops the step: a failed BeforeTool action skips the tool and hands the
	// model the error instead.
	FailClosed
)

// Action binds an applet call to ADK hooks.
//
// Build always runs inside the hook, so it may read the Event's payload and
// contexts freely. For NonBlocking actions only the call itself is deferred,
// and its arguments are encoded before the hook returns.
type Action struct {
	// Name labels the action in errors and reports.
	Name string
	// Hooks the action fires on.
	Hooks []Hook
	Mode  Mode
	// When, if set, filters the events the action fires for.
	When func(ev *Event) bool
	// Build returns the call to make, or nil to skip this event.
	Build func(ev *Event) (*Call, error)
	// OnResult, if set, sees each successful call's result.
	//
	// Blocking: it runs inside the hook, and a non-nil error vetoes the hook
	// whatever the FailurePolicy — this is how an applet's answer gates the
	// agent. NonBlocking: it runs on a dispatcher worker after the hook has
	// returned, so it must not touch ev.AgentContext or
	// ev.InvocationContext; its errors go to PluginConfig.OnError.
	OnResult func(ev *Event, res *Result) error
	// OnFailure applies to Blocking actions. NonBlocking actions always fail
	// open.
	OnFailure FailurePolicy
	// Timeout bounds a Blocking action's call. Zero leaves only the hook's
	// own context deadline. NonBlocking calls use the Dispatcher's
	// CallTimeout.
	Timeout time.Duration
	// Identity, if set, overrides the plugin's resolver for this action.
	Identity IdentityResolver
}

func (a *Action) label() string {
	if a.Name != "" {
		return a.Name
	}
	return "action"
}

// ActionError is returned from a hook that an action failed or vetoed.
type ActionError struct {
	Action string
	Hook   Hook
	Err    error
}

func (e *ActionError) Error() string {
	return fmt.Sprintf("weiladk: action %q on %s: %v", e.Action, e.Hook, e.Err)
}

func (e *ActionError) Unwrap() error { return e.Err }

// CallTo is a convenience Build for actions that always call the same applet
// method, with arguments derived from the event. args may be nil.
func CallTo(applet Applet, method string, args func(ev *Event) (any, error)) func(ev *Event) (*Call, error) {
	return func(ev *Event) (*Call, error) {
		call := &Call{Applet: applet, Method: method}
		if args != nil {
			v, err := args(ev)
			if err != nil {
				return nil, err
			}
			call.Args = v
		}
		return call, nil
	}
}
