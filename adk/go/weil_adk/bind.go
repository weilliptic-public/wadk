package weiladk

import (
	"context"
	"encoding/json"
	"errors"
	"sync"

	"google.golang.org/adk/v2/agent"
	"google.golang.org/adk/v2/agent/llmagent"
)

// ErrNoIdentity is returned when no identity is bound to the acting agent.
var ErrNoIdentity = errors.New("weiladk: no identity for this agent: create it with weiladk.NewLLMAgent or attach one with weiladk.Bind")

// Bindings are kept by agent name, since that is what ADK's callback and
// tool contexts expose. Names are unique within an agent tree; when several
// bound agents share a name (e.g. two apps in one process), the run's root
// agent — recorded by weiladk plugins — tells them apart.
var (
	bindMu   sync.RWMutex
	bindings = map[string][]binding{}
	parents  = map[agent.Agent]agent.Agent{} // sub-agent → its parent, for trees seen by Bind or a run
	known    = map[string][]agent.Agent{}    // every agent in those trees, by name
	runRoots sync.Map                        // invocation ID → root agent.Agent
)

type binding struct {
	agent agent.Agent
	id    *Identity
}

// Bind attaches an identity to an agent and returns the agent. Everything the
// agent does on chain — applet tools, plugin hooks, Invoke and Submit from
// its callbacks, A2A calls — is signed by that identity.
//
// Bind works with any ADK agent (LLM, workflow, custom or remote).
//
// Identity follows the agent tree, as in the Python SDK: a local sub-agent
// with no identity of its own acts as its nearest bound ancestor.
func Bind(a agent.Agent, id *Identity) agent.Agent {
	if a == nil || id == nil {
		return a
	}
	bindMu.Lock()
	defer bindMu.Unlock()
	recordTree(a)
	list := bindings[a.Name()]
	for i := range list {
		if list[i].agent == a {
			list[i].id = id
			return a
		}
	}
	bindings[a.Name()] = append(list, binding{agent: a, id: id})
	return a
}

// rememberTree records every agent under root, with its parent.
func rememberTree(root agent.Agent) {
	bindMu.Lock()
	defer bindMu.Unlock()
	recordTree(root)
}

// recordTree is rememberTree with bindMu held.
func recordTree(a agent.Agent) {
	remember(a)
	for _, sub := range a.SubAgents() {
		if sub == nil || parents[sub] == a {
			continue
		}
		parents[sub] = a
		recordTree(sub)
	}
}

func remember(a agent.Agent) {
	for _, k := range known[a.Name()] {
		if k == a {
			return
		}
	}
	known[a.Name()] = append(known[a.Name()], a)
}

// NewLLMAgent is llmagent.New with an identity attached: the same Config,
// plus the wallet the agent acts under.
func NewLLMAgent(id *Identity, cfg llmagent.Config) (agent.Agent, error) {
	a, err := llmagent.New(cfg)
	if err != nil {
		return nil, err
	}
	// NewAgentTool reads these: ADK keeps an agent's schemas internal.
	agentSchemas.Store(a, llmSchemas{input: cfg.InputSchema, output: cfg.OutputSchema})
	return Bind(a, id), nil
}

// IdentityOf returns the identity of the agent acting in ctx — any ADK
// callback, tool or invocation context — or nil if it has none.
func IdentityOf(ctx context.Context) *Identity {
	if ctx == nil {
		return nil
	}
	if c, ok := ctx.(interface {
		AgentName() string
		InvocationID() string
	}); ok {
		if id := lookup(c.AgentName(), c.InvocationID()); id != nil {
			return id
		}
	}
	if a := actingAgent(ctx); a != nil {
		if id := ownerOf(a); id != nil {
			return id
		}
	}
	if id, ok := ctx.Value(identityKey{}).(*Identity); ok {
		return id
	}
	return nil
}

// actingAgent returns the agent acting in an ADK context, or nil. Tool
// contexts expose only the agent's name, so it is looked up in the run's
// tree, or else among the agents of the trees Bind and runs have seen.
func actingAgent(ctx context.Context) agent.Agent {
	if ctx == nil {
		return nil
	}
	if c, ok := ctx.(interface {
		AgentName() string
		InvocationID() string
	}); ok {
		if root, ok := runRoots.Load(c.InvocationID()); ok {
			if a := root.(agent.Agent).FindAgent(c.AgentName()); a != nil {
				return a
			}
		}
		bindMu.RLock()
		list := known[c.AgentName()]
		bindMu.RUnlock()
		if len(list) == 1 {
			return list[0]
		}
	}
	// Last: ADK's tool contexts log an error on Agent() and return nil.
	if c, ok := ctx.(interface{ Agent() agent.Agent }); ok {
		return c.Agent()
	}
	return nil
}

// ownerOf returns a's identity, or else that of its nearest bound ancestor.
func ownerOf(a agent.Agent) *Identity {
	bindMu.RLock()
	defer bindMu.RUnlock()
	for ; a != nil; a = parents[a] {
		if id := boundToLocked(a); id != nil {
			return id
		}
	}
	return nil
}

// enclosingOf returns the identity of a's nearest bound ancestor, not a
// itself: the agent that invoked a, when a is nested (see
// CallerFromContext).
func enclosingOf(a agent.Agent) *Identity {
	bindMu.RLock()
	defer bindMu.RUnlock()
	for a = parents[a]; a != nil; a = parents[a] {
		if id := boundToLocked(a); id != nil {
			return id
		}
	}
	return nil
}

func boundTo(a agent.Agent) *Identity {
	bindMu.RLock()
	defer bindMu.RUnlock()
	return boundToLocked(a)
}

func boundToLocked(a agent.Agent) *Identity {
	for _, b := range bindings[a.Name()] {
		if b.agent == a {
			return b.id
		}
	}
	return nil
}

func lookup(name, invocationID string) *Identity {
	bindMu.RLock()
	list := bindings[name]
	bindMu.RUnlock()
	switch len(list) {
	case 0:
		return nil
	case 1:
		return list[0].id
	}
	root, ok := runRoots.Load(invocationID)
	if !ok {
		return nil // ambiguous without the run's root
	}
	for _, b := range list {
		if inTree(root.(agent.Agent), b.agent) {
			return b.id
		}
	}
	return nil
}

func inTree(root, a agent.Agent) bool {
	if root == a {
		return true
	}
	for _, sub := range root.SubAgents() {
		if inTree(sub, a) {
			return true
		}
	}
	return false
}

type identityKey struct{}

// ContextWithIdentity returns a context in which IdentityOf reports id, for
// code that runs outside an agent (e.g. setup, background jobs).
func ContextWithIdentity(ctx context.Context, id *Identity) context.Context {
	return context.WithValue(ctx, identityKey{}, id)
}

// Invoke runs call as the agent acting in ctx and waits for the result.
// Use it from ADK callbacks and tools.
func Invoke(ctx context.Context, call Call) (*Result, error) {
	id := IdentityOf(ctx)
	if id == nil {
		return nil, ErrNoIdentity
	}
	return id.Invoke(ctx, call)
}

// Submit queues call to run as the agent acting in ctx and returns at once;
// onDone, if non-nil, receives the outcome later. Inside a weiladk plugin's
// callbacks the plugin's dispatcher is used, elsewhere DefaultDispatcher.
func Submit(ctx context.Context, call Call, onDone func(*Result, error)) error {
	id := IdentityOf(ctx)
	if id == nil {
		return ErrNoIdentity
	}
	args, err := call.encodeArgs()
	if err != nil {
		return err
	}
	call.Args = json.RawMessage(args) // snapshot: ADK may reuse maps once the caller returns
	return dispatcherFrom(ctx).Submit(ctx, id, call, onDone)
}

type dispatcherKey struct{}

func dispatcherFrom(ctx context.Context) *Dispatcher {
	if d, ok := ctx.Value(dispatcherKey{}).(*Dispatcher); ok {
		return d
	}
	return DefaultDispatcher()
}

var defaultDispatcher = sync.OnceValue(func() *Dispatcher { return NewDispatcher(DispatcherConfig{}) })

// DefaultDispatcher is the shared background queue used by Submit and by
// plugins that are not given their own (see WithDispatcher).
func DefaultDispatcher() *Dispatcher { return defaultDispatcher() }
