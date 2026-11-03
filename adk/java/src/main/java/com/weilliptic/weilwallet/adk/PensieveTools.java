package com.weilliptic.weilwallet.adk;

import com.fasterxml.jackson.databind.JsonNode;
import com.google.adk.tools.BaseTool;
import com.google.adk.tools.ToolContext;
import com.google.genai.types.FunctionDeclaration;
import com.google.genai.types.Schema;
import com.google.genai.types.Type;

import io.reactivex.rxjava3.core.Single;

import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.Optional;

/**
 * The Pensieve graph memory, as tools a model can call.
 *
 * <p>Twelve tools over one {@link Pensieve}: four that write, three that
 * forget, five that read. Memory is scoped to the agent's own wallet, so the
 * model never supplies an agent id and cannot reach another agent's graph.</p>
 *
 * <pre>{@code
 * LlmAgent agent = LlmAgent.builder()
 *     .name("travel_planner")
 *     .model(model)
 *     .tools(PensieveTools.all(identity))
 *     .build();
 * }</pre>
 *
 * <p>Each call waits for the applet, so twelve available tools mean a chain
 * round-trip whenever the model reaches for one. {@link #reading} and
 * {@link #writing} hand out subsets for an agent that only needs half of it.</p>
 *
 * <p>The parameter descriptions are the ones the Python SDK uses, and they are
 * doing real work: they are what leads a model to reuse predicates and to file
 * facts about itself under {@code "self"} instead of inventing a new entity
 * each turn. Reword them and the graph gets messier.</p>
 */
public final class PensieveTools {

    // Descriptions shared across tools, kept identical to the Python SDK's.
    private static final String ENTITY_NAME =
        "The name of the entity, like \"Olive Garden\" or \"self\". Use \"self\" for facts "
            + "about the agent or the user it is acting for.";
    private static final String ENTITY_TYPE =
        "The category of the entity, like \"person\", \"restaurant\", \"company\", \"project\" "
            + "or \"preference\". Use a short, reusable noun.";
    private static final String OBSERVATIONS =
        "Atomic properties that belong to this entity alone and name nothing else. Put related "
            + "things in entities plus relations, not observations.";
    private static final String SOURCE =
        "The name of the entity the relation starts at, like \"self\".";
    private static final String RELATION =
        "The predicate, in active voice, like \"likes_restaurant\" or \"works_at\". Reuse "
            + "predicates across entities so the graph stays queryable.";
    private static final String TARGET =
        "The name of the entity the relation points to, like \"Olive Garden\".";
    private static final String GRAPH_JSON =
        "A JSON object with optional \"entities\" and \"relations\" arrays. Include every entity "
            + "a relation references, including \"self\"; observations may be omitted.";
    private static final String NAMES =
        "The names of the entities to delete.";
    private static final String SEARCH_TEXT =
        "The text to look for, like \"restaurant\" or \"olive\". Matching is case-insensitive "
            + "substring matching, not semantic search.";
    private static final String LIMIT =
        "Maximum number of entities to return. Check the truncated field on graph results "
            + "before treating the result as complete.";
    private static final String START =
        "The name of the entity to start walking from.";
    private static final String OPTIONAL_RELATION =
        "Only follow this predicate, like \"works_at\". Omit to follow every predicate.";
    private static final String DIRECTION =
        "\"out\" to follow relations forward, \"in\" backward, or \"both\" to ignore direction. "
            + "Defaults to \"out\".";
    private static final String DEPTH =
        "How many hops to walk, from 1 to 5. Defaults to 2.";

    private PensieveTools() {}

    /** Whether {@code tool} is one of these memory tools. */
    public static boolean isMemoryTool(Object tool) {
        return tool instanceof PensieveTool;
    }

    /** All twelve tools, over the agent's own memory. */
    public static List<BaseTool> all(WeilIdentity identity) {
        return all(new Pensieve(identity));
    }

    /** All twelve tools, over a given memory. */
    public static List<BaseTool> all(Pensieve memory) {
        List<BaseTool> tools = new ArrayList<>(writing(memory));
        tools.addAll(forgetting(memory));
        tools.addAll(reading(memory));
        return List.copyOf(tools);
    }

    /** The four writing tools: remember_entity, remember_relation, remember_batch, add_observations. */
    public static List<BaseTool> writing(Pensieve memory) {
        Objects.requireNonNull(memory, "memory");
        return List.of(
            tool(memory, "remember_entity",
                "Write one entity into this agent's memory, creating it if it is new. The entity "
                    + "keeps existing observations, gains new observations, and takes the new name "
                    + "and type; write entities before relating them.",
                param("name", Type.Known.STRING, ENTITY_NAME, true),
                param("entity_type", Type.Known.STRING, ENTITY_TYPE, true),
                listParam("observations", OBSERVATIONS, true)),

            tool(memory, "remember_relation",
                "Connect two existing entities in this agent's memory with a directed relation. "
                    + "Writing a relation that already exists is a no-op that returns the original.",
                param("source", Type.Known.STRING, SOURCE, true),
                param("relation", Type.Known.STRING, RELATION, true),
                param("target", Type.Known.STRING, TARGET, true)),

            tool(memory, "remember_batch",
                "Write many entities and relations in one call. Use a JSON object with optional "
                    + "entities and relations arrays; entities are written first, so relations in "
                    + "the same payload may reference them.",
                param("graph_json", Type.Known.STRING, GRAPH_JSON, true)),

            tool(memory, "add_observations",
                "Add observations to an entity that already exists in this agent's memory. "
                    + "Observations already recorded are ignored, so this is safe to call "
                    + "repeatedly.",
                param("entity", Type.Known.STRING, ENTITY_NAME, true),
                listParam("observations", OBSERVATIONS, true)));
    }

    /** The three deleting tools: forget_observations, forget_entities, forget_relation. */
    public static List<BaseTool> forgetting(Pensieve memory) {
        Objects.requireNonNull(memory, "memory");
        return List.of(
            tool(memory, "forget_observations",
                "Remove observations from an entity in this agent's memory, leaving the entity "
                    + "itself in place.",
                param("entity", Type.Known.STRING, ENTITY_NAME, true),
                listParam("observations", OBSERVATIONS, true)),

            tool(memory, "forget_entities",
                "Delete entities from this agent's memory, along with the relations that touch "
                    + "them.",
                listParam("names", NAMES, true)),

            tool(memory, "forget_relation",
                "Delete one relation from this agent's memory, leaving both entities in place.",
                param("source", Type.Known.STRING, SOURCE, true),
                param("relation", Type.Known.STRING, RELATION, true),
                param("target", Type.Known.STRING, TARGET, true)));
    }

    /** The five reading tools: recall_entity, search_memory, traverse, list_relation_types, recall_graph. */
    public static List<BaseTool> reading(Pensieve memory) {
        Objects.requireNonNull(memory, "memory");
        return List.of(
            tool(memory, "recall_entity",
                "Read one entity from this agent's memory by exact name, with its observations.",
                param("name", Type.Known.STRING, ENTITY_NAME, true)),

            tool(memory, "search_memory",
                "Find entities in this agent's memory whose name, type or observations contain "
                    + "the given text.",
                param("text", Type.Known.STRING, SEARCH_TEXT, true),
                param("limit", Type.Known.INTEGER, LIMIT, false)),

            tool(memory, "traverse",
                "Walk this agent's memory outward from one entity, following relations, to find "
                    + "what it is connected to.",
                param("start", Type.Known.STRING, START, true),
                param("relation", Type.Known.STRING, OPTIONAL_RELATION, false),
                param("direction", Type.Known.STRING, DIRECTION, false),
                param("depth", Type.Known.INTEGER, DEPTH, false)),

            tool(memory, "list_relation_types",
                "List the predicates attached to one entity in this agent's memory, to discover "
                    + "how it is connected before traversing.",
                param("name", Type.Known.STRING, ENTITY_NAME, true)),

            tool(memory, "recall_graph",
                "Read this agent's whole memory graph: its entities and the relations between "
                    + "them.",
                param("limit", Type.Known.INTEGER, LIMIT, false)));
    }

    // ── Internals ────────────────────────────────────────────────────────────

    private static BaseTool tool(Pensieve memory, String name, String description, Param... params) {
        return new PensieveTool(memory, name, description, List.of(params));
    }

    private static Param param(String name, Type.Known type, String description, boolean required) {
        return new Param(name, type, description, required, false);
    }

    private static Param listParam(String name, String description, boolean required) {
        return new Param(name, Type.Known.STRING, description, required, true);
    }

    /** One declared argument: its name, JSON type, description and whether it is required. */
    private record Param(
        String name, Type.Known type, String description, boolean required, boolean list) {

        Schema schema() {
            if (list) {
                return Schema.builder()
                    .type(Type.Known.ARRAY)
                    .description(description)
                    .items(Schema.builder().type(Type.Known.STRING).build())
                    .build();
            }
            return Schema.builder().type(type).description(description).build();
        }
    }

    /**
     * One memory operation, dispatched to {@link Pensieve} by method name.
     *
     * <p>One class rather than twelve: every tool differs only in its declared
     * arguments and which method it reaches, and the model is told the
     * difference through the declaration.</p>
     */
    private static final class PensieveTool extends BaseTool {

        private final Pensieve memory;
        private final List<Param> params;

        PensieveTool(Pensieve memory, String name, String description, List<Param> params) {
            super(name, description);
            this.memory = memory;
            this.params = params;
        }

        @Override
        public Optional<FunctionDeclaration> declaration() {
            Map<String, Schema> properties = new LinkedHashMap<>();
            List<String> required = new ArrayList<>();
            for (Param p : params) {
                properties.put(p.name(), p.schema());
                if (p.required()) {
                    required.add(p.name());
                }
            }
            Schema.Builder parameters = Schema.builder()
                .type(Type.Known.OBJECT)
                .properties(properties);
            if (!required.isEmpty()) {
                parameters.required(required);
            }
            return Optional.of(FunctionDeclaration.builder()
                .name(name())
                .description(description())
                .parameters(parameters.build())
                .build());
        }

        @Override
        public Single<Map<String, Object>> runAsync(Map<String, Object> args, ToolContext context) {
            return Single.fromCallable(() -> invoke(args));
        }

        /**
         * Hand the model either the applet's answer or the reason it failed.
         *
         * <p>A refusal is returned rather than thrown so the model can explain
         * it, as {@link WeilA2A#remoteTool} does with a remote agent's
         * error.</p>
         */
        private Map<String, Object> invoke(Map<String, Object> args) {
            try {
                JsonNode result = dispatch(args);
                Map<String, Object> out = new LinkedHashMap<>();
                out.put("result", result == null ? null : result);
                return out;
            } catch (InterruptedException e) {
                Thread.currentThread().interrupt();
                return Map.of("error", name() + " was interrupted");
            } catch (Exception e) {
                return Map.of("error", name() + " failed: " + e.getMessage());
            }
        }

        private JsonNode dispatch(Map<String, Object> args) throws Exception {
            return switch (name()) {
                case "remember_entity" -> memory.rememberEntity(
                    str(args, "name"), str(args, "entity_type"), strings(args, "observations"));
                case "remember_relation" -> memory.rememberRelation(
                    str(args, "source"), str(args, "relation"), str(args, "target"));
                case "remember_batch" -> memory.rememberBatch(str(args, "graph_json"));
                case "add_observations" -> memory.addObservations(
                    str(args, "entity"), strings(args, "observations"));
                case "forget_observations" -> memory.forgetObservations(
                    str(args, "entity"), strings(args, "observations"));
                case "forget_entities" -> memory.forgetEntities(strings(args, "names"));
                case "forget_relation" -> memory.forgetRelation(
                    str(args, "source"), str(args, "relation"), str(args, "target"));
                case "recall_entity" -> memory.recallEntity(str(args, "name"));
                case "search_memory" -> memory.searchMemory(
                    str(args, "text"), integer(args, "limit"));
                case "traverse" -> memory.traverse(
                    str(args, "start"), str(args, "relation"),
                    str(args, "direction"), integer(args, "depth"));
                case "list_relation_types" -> memory.listRelationTypes(str(args, "name"));
                case "recall_graph" -> memory.recallGraph(integer(args, "limit"));
                default -> throw new IllegalStateException("unknown memory tool: " + name());
            };
        }

        private static String str(Map<String, Object> args, String key) {
            Object value = args.get(key);
            return value == null ? null : String.valueOf(value);
        }

        private static Integer integer(Map<String, Object> args, String key) {
            Object value = args.get(key);
            if (value == null) {
                return null;
            }
            if (value instanceof Number n) {
                return n.intValue();
            }
            try {
                return Integer.valueOf(String.valueOf(value).trim());
            } catch (NumberFormatException e) {
                return null;
            }
        }

        /** A model may send a single string where a list is declared; accept both. */
        private static List<String> strings(Map<String, Object> args, String key) {
            Object value = args.get(key);
            if (value == null) {
                return List.of();
            }
            if (value instanceof List<?> list) {
                List<String> out = new ArrayList<>(list.size());
                for (Object item : list) {
                    if (item != null) {
                        out.add(String.valueOf(item));
                    }
                }
                return out;
            }
            return List.of(String.valueOf(value));
        }
    }
}
