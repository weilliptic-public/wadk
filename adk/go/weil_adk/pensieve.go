package weiladk

import (
	"fmt"

	"google.golang.org/adk/v2/agent"
	"google.golang.org/adk/v2/tool"
	"google.golang.org/adk/v2/tool/functiontool"

	"github.com/weilliptic-public/wadk/adk/go/weil_wallet/client"
)

// PensieveTools returns ADK tools over the Pensieve graph-memory applet
// (pensieve::weil), scoped to the calling agent's wallet: each agent reads
// and writes only its own memory. They are the same tools, with the same
// names and arguments, as the Python SDK's make_adk_pensieve_tools:
//
//	weiladk.NewLLMAgent(id, llmagent.Config{..., Tools: append(tools, weiladk.PensieveTools()...)})
//
// remember_entity, remember_relation, remember_batch, add_observations,
// forget_observations, forget_entities, forget_relation, recall_entity,
// search_memory, traverse, list_relation_types, recall_graph.
func PensieveTools() []tool.Tool {
	type (
		entityArgs struct {
			Name         string   `json:"name" jsonschema:"The name of the entity, like \"Olive Garden\" or \"self\". Use \"self\" for facts about the agent or the user it is acting for."`
			EntityType   string   `json:"entity_type" jsonschema:"The category of the entity, like \"person\", \"restaurant\", \"company\", \"project\" or \"preference\". Use a short, reusable noun."`
			Observations []string `json:"observations" jsonschema:"Atomic properties that belong to this entity alone and name nothing else. Put related things in entities plus relations, not observations."`
		}
		relationArgs struct {
			Source   string `json:"source" jsonschema:"The name of the entity the relation starts at, like \"self\"."`
			Relation string `json:"relation" jsonschema:"The predicate, in active voice, like \"likes_restaurant\" or \"works_at\". Reuse predicates across entities so the graph stays queryable."`
			Target   string `json:"target" jsonschema:"The name of the entity the relation points to, like \"Olive Garden\"."`
		}
		batchArgs struct {
			GraphJSON string `json:"graph_json" jsonschema:"A JSON object with optional \"entities\" and \"relations\" arrays. Include every entity a relation references, including \"self\"; observations may be omitted."`
		}
		observationArgs struct {
			Entity       string   `json:"entity" jsonschema:"The name of the entity, like \"Olive Garden\" or \"self\". Use \"self\" for facts about the agent or the user it is acting for."`
			Observations []string `json:"observations" jsonschema:"Atomic properties that belong to this entity alone and name nothing else. Put related things in entities plus relations, not observations."`
		}
		namesArgs struct {
			Names []string `json:"names" jsonschema:"The names of the entities to delete."`
		}
		nameArgs struct {
			Name string `json:"name" jsonschema:"The name of the entity, like \"Olive Garden\" or \"self\". Use \"self\" for facts about the agent or the user it is acting for."`
		}
		searchArgs struct {
			Text  string `json:"text" jsonschema:"The text to look for, like \"restaurant\" or \"olive\". Matching is case-insensitive substring matching, not semantic search."`
			Limit *int   `json:"limit,omitempty" jsonschema:"Maximum number of entities to return. Check the truncated field on graph results before treating the result as complete."`
		}
		traverseArgs struct {
			Start     string  `json:"start" jsonschema:"The name of the entity to start walking from."`
			Relation  *string `json:"relation,omitempty" jsonschema:"Only follow this predicate, like \"works_at\". Omit to follow every predicate."`
			Direction *string `json:"direction,omitempty" jsonschema:"\"out\" to follow relations forward, \"in\" backward, or \"both\" to ignore direction. Defaults to \"out\"."`
			Depth     *int    `json:"depth,omitempty" jsonschema:"How many hops to walk, from 1 to 5. Defaults to 2."`
		}
		limitArgs struct {
			Limit *int `json:"limit,omitempty" jsonschema:"Maximum number of entities to return. Check the truncated field on graph results before treating the result as complete."`
		}
	)

	return []tool.Tool{
		memoryTool("remember_entity", "Write one entity into this agent's memory, creating it if it is new. The entity keeps existing observations, gains new observations, and takes the new name and type; write entities before relating them.",
			func(p *client.Pensieve, ctx agent.Context, me string, a entityArgs) (any, error) {
				return p.RememberEntity(ctx, me, a.Name, a.EntityType, a.Observations)
			}),
		memoryTool("remember_relation", "Connect two existing entities in this agent's memory with a directed relation. Writing a relation that already exists is a no-op that returns the original.",
			func(p *client.Pensieve, ctx agent.Context, me string, a relationArgs) (any, error) {
				return p.RememberRelation(ctx, me, a.Source, a.Relation, a.Target)
			}),
		memoryTool("remember_batch", "Write many entities and relations in one call. Use a JSON object with optional entities and relations arrays; entities are written first, so relations in the same payload may reference them.",
			func(p *client.Pensieve, ctx agent.Context, me string, a batchArgs) (any, error) {
				return p.RememberBatch(ctx, me, a.GraphJSON)
			}),
		memoryTool("add_observations", "Add observations to an entity that already exists in this agent's memory. Observations already recorded are ignored, so this is safe to call repeatedly.",
			func(p *client.Pensieve, ctx agent.Context, me string, a observationArgs) (any, error) {
				return p.AddObservations(ctx, me, a.Entity, a.Observations)
			}),
		memoryTool("forget_observations", "Remove exact observations from an entity, leaving the entity and its relations in place. Use this when a fact is superseded rather than deleting the whole entity.",
			func(p *client.Pensieve, ctx agent.Context, me string, a observationArgs) (any, error) {
				return p.ForgetObservations(ctx, me, a.Entity, a.Observations)
			}),
		memoryTool("forget_entities", "Delete entities from this agent's memory and every relation touching them, in either direction.",
			func(p *client.Pensieve, ctx agent.Context, me string, a namesArgs) (any, error) {
				return p.ForgetEntities(ctx, me, a.Names)
			}),
		memoryTool("forget_relation", "Delete one relation from this agent's memory, leaving both endpoint entities in place. Returns false when the relation was not in memory.",
			func(p *client.Pensieve, ctx agent.Context, me string, a relationArgs) (any, error) {
				return p.ForgetRelation(ctx, me, a.Source, a.Relation, a.Target)
			}),
		memoryTool("recall_entity", "Read one entity from this agent's memory together with every relation that touches it. This is the cheapest way to answer what is known about one entity.",
			func(p *client.Pensieve, ctx agent.Context, me string, a nameArgs) (any, error) {
				return p.RecallEntity(ctx, me, a.Name)
			}),
		memoryTool("search_memory", "Find entities whose name, type, or observations contain the given text, together with relations running between the matches. Matching is case-insensitive substring matching, not semantic search.",
			func(p *client.Pensieve, ctx agent.Context, me string, a searchArgs) (any, error) {
				return p.SearchMemory(ctx, me, a.Text, a.Limit)
			}),
		memoryTool("traverse", "Walk this agent's memory graph outward from one entity and return everything reachable within depth hops, with connecting relations. Use this when a question spans related things.",
			func(p *client.Pensieve, ctx agent.Context, me string, a traverseArgs) (any, error) {
				return p.Traverse(ctx, me, a.Start, a.Relation, a.Direction, a.Depth)
			}),
		memoryTool("list_relation_types", "List the distinct predicates recorded on an entity in this agent's memory, in either direction. Call this before traverse to discover useful relations.",
			func(p *client.Pensieve, ctx agent.Context, me string, a nameArgs) (any, error) {
				return p.ListRelationTypes(ctx, me, a.Name)
			}),
		memoryTool("recall_graph", "Read this agent's whole memory graph. Prefer search_memory or traverse once memory is large, and always check the truncated field before treating the result as complete.",
			func(p *client.Pensieve, ctx agent.Context, me string, a limitArgs) (any, error) {
				return p.RecallGraph(ctx, me, a.Limit)
			}),
	}
}

func memoryTool[In any](name, description string, fn func(p *client.Pensieve, ctx agent.Context, me string, in In) (any, error)) tool.Tool {
	t, err := functiontool.New(functiontool.Config{Name: name, Description: description},
		func(ctx agent.Context, in In) (map[string]any, error) {
			id := IdentityOf(ctx)
			if id == nil {
				return nil, ErrNoIdentity
			}
			out, err := fn(id.client.Pensieve(), ctx, id.Address(), in)
			if err != nil {
				return nil, err
			}
			// As Python's ADK: an object result reaches the model as it is,
			// anything else wrapped as {"result": ...}.
			if m, ok := out.(map[string]any); ok {
				return m, nil
			}
			return map[string]any{"result": out}, nil
		})
	if err != nil {
		panic(fmt.Sprintf("weiladk: pensieve tool %s: %v", name, err))
	}
	return t
}
