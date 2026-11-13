package client

import (
	"bytes"
	"context"
	"encoding/json"
)

// Pensieve is a client for the Pensieve graph-memory applet (pensieve::weil):
// entities with observations, joined by directed relations, kept per agent
// id — the twin of Python's weil_wallet.Pensieve. Calls are signed by the
// client's wallet and wait for the applet; results are the decoded return
// values.
type Pensieve struct {
	client *WeilClient
	applet string
}

// Pensieve returns a Pensieve client signing with w.
func (w *WeilClient) Pensieve() *Pensieve {
	return &Pensieve{client: w, applet: PensieveApplet}
}

// WithApplet returns a copy that talks to another Pensieve deployment
// (service name or contract id).
func (p *Pensieve) WithApplet(applet string) *Pensieve {
	return &Pensieve{client: p.client, applet: applet}
}

// Call invokes any Pensieve method with the given arguments.
func (p *Pensieve) Call(ctx context.Context, method string, args map[string]any) (any, error) {
	raw, err := PythonDumpsValue(args, false)
	if err != nil {
		return nil, err
	}
	return p.callRaw(ctx, method, raw)
}

// callOrdered calls method with the arguments kv (key, value, key, value,
// ...) encoded as the Python SDK's json.dumps of its dict, keys in order.
func (p *Pensieve) callOrdered(ctx context.Context, method string, kv ...any) (any, error) {
	var buf bytes.Buffer
	buf.WriteByte('{')
	for i := 0; i < len(kv); i += 2 {
		if i > 0 {
			buf.WriteByte(',')
		}
		k, _ := json.Marshal(kv[i])
		v, err := json.Marshal(kv[i+1])
		if err != nil {
			return nil, err
		}
		buf.Write(k)
		buf.WriteByte(':')
		buf.Write(v)
	}
	buf.WriteByte('}')
	raw, err := PythonDumps(buf.Bytes(), false)
	if err != nil {
		return nil, err
	}
	return p.callRaw(ctx, method, raw)
}

func (p *Pensieve) callRaw(ctx context.Context, method string, raw []byte) (any, error) {
	var err error
	var res interface{ Decode(any) error }
	if isContractID(p.applet) {
		res, err = p.client.ExecuteContext(ctx, p.applet, method, string(raw), ExecuteOptions{})
	} else {
		res, err = p.client.ExecuteByName(ctx, p.applet, method, string(raw), ExecuteOptions{})
	}
	if err != nil {
		return nil, err
	}
	var out any
	if err := res.Decode(&out); err != nil {
		return nil, err
	}
	return out, nil
}

func (p *Pensieve) RememberEntity(ctx context.Context, agentID, name, entityType string, observations []string) (any, error) {
	return p.callOrdered(ctx, "remember_entity", "agent_id", agentID, "name", name, "entity_type", entityType, "observations", observations)
}

func (p *Pensieve) RememberRelation(ctx context.Context, agentID, source, relation, target string) (any, error) {
	return p.callOrdered(ctx, "remember_relation", "agent_id", agentID, "source", source, "relation", relation, "target", target)
}

// RememberBatch writes many entities and relations; graph is a JSON string
// or a value encoded to one.
func (p *Pensieve) RememberBatch(ctx context.Context, agentID string, graph any) (any, error) {
	s, ok := graph.(string)
	if !ok {
		b, err := PythonDumpsValue(graph, false) // json.dumps(graph_json)
		if err != nil {
			return nil, err
		}
		s = string(b)
	}
	return p.callOrdered(ctx, "remember_batch", "agent_id", agentID, "graph_json", s)
}

func (p *Pensieve) AddObservations(ctx context.Context, agentID, entity string, observations []string) (any, error) {
	return p.callOrdered(ctx, "add_observations", "agent_id", agentID, "entity", entity, "observations", observations)
}

func (p *Pensieve) ForgetObservations(ctx context.Context, agentID, entity string, observations []string) (any, error) {
	return p.callOrdered(ctx, "forget_observations", "agent_id", agentID, "entity", entity, "observations", observations)
}

func (p *Pensieve) ForgetEntities(ctx context.Context, agentID string, names []string) (any, error) {
	return p.callOrdered(ctx, "forget_entities", "agent_id", agentID, "names", names)
}

func (p *Pensieve) ForgetRelation(ctx context.Context, agentID, source, relation, target string) (any, error) {
	return p.callOrdered(ctx, "forget_relation", "agent_id", agentID, "source", source, "relation", relation, "target", target)
}

func (p *Pensieve) RecallEntity(ctx context.Context, agentID, name string) (any, error) {
	return p.callOrdered(ctx, "recall_entity", "agent_id", agentID, "name", name)
}

// SearchMemory finds entities matching text; limit nil means the applet's
// default.
func (p *Pensieve) SearchMemory(ctx context.Context, agentID, text string, limit *int) (any, error) {
	return p.callOrdered(ctx, "search_memory", "agent_id", agentID, "text", text, "limit", limit)
}

// Traverse walks the graph from start; nil options take the applet's
// defaults (every relation, direction "out", depth 2).
func (p *Pensieve) Traverse(ctx context.Context, agentID, start string, relation, direction *string, depth *int) (any, error) {
	return p.callOrdered(ctx, "traverse", "agent_id", agentID, "start", start, "relation", relation, "direction", direction, "depth", depth)
}

func (p *Pensieve) ListRelationTypes(ctx context.Context, agentID, name string) (any, error) {
	return p.callOrdered(ctx, "list_relation_types", "agent_id", agentID, "name", name)
}

func (p *Pensieve) RecallGraph(ctx context.Context, agentID string, limit *int) (any, error) {
	return p.callOrdered(ctx, "recall_graph", "agent_id", agentID, "limit", limit)
}

// isContractID reports whether s looks like a contract id rather than a
// service name (which always contains "::").
func isContractID(s string) bool {
	for i := 0; i+1 < len(s); i++ {
		if s[i] == ':' && s[i+1] == ':' {
			return false
		}
	}
	return true
}
