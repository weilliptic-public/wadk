package com.weilliptic.weilwallet.adk;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;

import java.io.IOException;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;

/**
 * Graph memory for an agent, held by the Pensieve applet.
 *
 * <p>Memory is a graph of <em>entities</em> ("Olive Garden", "self"), each
 * carrying free-text <em>observations</em>, joined by directed
 * <em>relations</em> ("self" → {@code likes_restaurant} → "Olive Garden"). It
 * is scoped to the calling wallet: every method sends this identity's address
 * as {@code agent_id}, so one agent cannot read or write another's memory.</p>
 *
 * <p>Unlike {@link AuditAction}, these calls <strong>wait</strong> for the
 * applet to run. The read methods return the graph, and a fire-and-forget
 * submit comes back with nothing to read; the write methods return the record
 * they stored. Each call is therefore a chain round-trip.</p>
 *
 * <pre>{@code
 * Pensieve memory = new Pensieve(identity);
 * memory.rememberEntity("self", "preference", List.of("prefers window seats"));
 * memory.rememberRelation("self", "likes_airline", "IndiGo");
 * JsonNode graph = memory.recallGraph(null);
 * }</pre>
 *
 * <p>To expose this to a model as tools rather than calling it directly, see
 * {@link PensieveTools}.</p>
 */
public final class Pensieve {

    private static final ObjectMapper JSON = new ObjectMapper();

    private final WeilIdentity identity;
    private final Applet applet;

    /** Memory held by the systemic {@code pensieve::weil} applet. */
    public Pensieve(WeilIdentity identity) {
        this(identity, Applet.pensieve());
    }

    /** As {@link #Pensieve(WeilIdentity)}, against a different applet. */
    public Pensieve(WeilIdentity identity, Applet applet) {
        this.identity = Objects.requireNonNull(identity, "identity");
        this.applet = Objects.requireNonNull(applet, "applet");
    }

    /** The wallet this memory belongs to, sent as {@code agent_id}. */
    public String agentId() {
        return identity.address();
    }

    // ── Writing ──────────────────────────────────────────────────────────────

    /**
     * Write one entity, creating it when it is new.
     *
     * <p>An existing entity keeps the observations it has, gains these, and
     * takes this type. Write an entity before relating it to anything.</p>
     */
    public JsonNode rememberEntity(String name, String entityType, List<String> observations)
        throws IOException, InterruptedException, WeilResult.AppletException {
        return call("remember_entity", args(
            "name", name,
            "entity_type", entityType,
            "observations", observations));
    }

    /** Join two existing entities with a directed relation. Writing a duplicate is a no-op. */
    public JsonNode rememberRelation(String source, String relation, String target)
        throws IOException, InterruptedException, WeilResult.AppletException {
        return call("remember_relation", args(
            "source", source,
            "relation", relation,
            "target", target));
    }

    /**
     * Write many entities and relations at once.
     *
     * @param graphJson a JSON object with optional {@code entities} and
     *                  {@code relations} arrays. Entities are written first, so
     *                  relations in the same payload may reference them.
     */
    public JsonNode rememberBatch(String graphJson)
        throws IOException, InterruptedException, WeilResult.AppletException {
        return call("remember_batch", args("graph_json", graphJson));
    }

    /** Add observations to an entity that already exists. Known ones are ignored. */
    public JsonNode addObservations(String entity, List<String> observations)
        throws IOException, InterruptedException, WeilResult.AppletException {
        return call("add_observations", args(
            "entity", entity,
            "observations", observations));
    }

    // ── Forgetting ───────────────────────────────────────────────────────────

    /** Remove observations from an entity, leaving the entity itself. */
    public JsonNode forgetObservations(String entity, List<String> observations)
        throws IOException, InterruptedException, WeilResult.AppletException {
        return call("forget_observations", args(
            "entity", entity,
            "observations", observations));
    }

    /** Delete entities outright, with the relations that touch them. */
    public JsonNode forgetEntities(List<String> names)
        throws IOException, InterruptedException, WeilResult.AppletException {
        return call("forget_entities", args("names", names));
    }

    /** Delete one relation, leaving both entities in place. */
    public JsonNode forgetRelation(String source, String relation, String target)
        throws IOException, InterruptedException, WeilResult.AppletException {
        return call("forget_relation", args(
            "source", source,
            "relation", relation,
            "target", target));
    }

    // ── Reading ──────────────────────────────────────────────────────────────

    /** One entity with its observations, or null when it is not in memory. */
    public JsonNode recallEntity(String name)
        throws IOException, InterruptedException, WeilResult.AppletException {
        return call("recall_entity", args("name", name));
    }

    /**
     * Entities matching {@code text}.
     *
     * <p>Case-insensitive substring matching over names, types and
     * observations — not semantic search.</p>
     *
     * @param limit maximum entities to return, or null for the applet's default
     */
    public JsonNode searchMemory(String text, Integer limit)
        throws IOException, InterruptedException, WeilResult.AppletException {
        return call("search_memory", args("text", text, "limit", limit));
    }

    /**
     * Walk the graph outward from one entity.
     *
     * @param relation  follow only this predicate, or null for every predicate
     * @param direction {@code "out"}, {@code "in"} or {@code "both"}; null means out
     * @param depth     hops to walk, 1 to 5; null means the applet's default
     */
    public JsonNode traverse(String start, String relation, String direction, Integer depth)
        throws IOException, InterruptedException, WeilResult.AppletException {
        return call("traverse", args(
            "start", start,
            "relation", relation,
            "direction", direction,
            "depth", depth));
    }

    /** The predicates attached to one entity, for discovering how it is connected. */
    public JsonNode listRelationTypes(String name)
        throws IOException, InterruptedException, WeilResult.AppletException {
        return call("list_relation_types", args("name", name));
    }

    /**
     * The whole graph for this wallet.
     *
     * <p>Check the result's {@code truncated} field before treating it as
     * complete.</p>
     */
    public JsonNode recallGraph(Integer limit)
        throws IOException, InterruptedException, WeilResult.AppletException {
        return call("recall_graph", args("limit", limit));
    }

    // ── Internals ────────────────────────────────────────────────────────────

    /**
     * Invoke an applet method and unwrap its {@code Ok} value.
     *
     * <p>Blocking and with arguments visible, matching the Python SDK: a
     * fire-and-forget submit returns before the method runs, so a read would
     * come back empty.</p>
     */
    private JsonNode call(String method, Map<String, Object> args)
        throws IOException, InterruptedException, WeilResult.AppletException {
        WeilResult result = identity.invoke(Call.to(applet, method)
            .args(args)
            .hideArgs(false)
            .fireAndForget(false)
            .build());
        return reparse(result.decode());
    }

    /**
     * The applet returns some values as a JSON string inside {@code Ok}, so a
     * text node that is itself JSON is parsed once more. A plain string, such
     * as a name, is returned as it is.
     */
    private static JsonNode reparse(JsonNode ok) {
        if (ok == null || !ok.isTextual()) {
            return ok;
        }
        String text = ok.asText().trim();
        if (text.isEmpty() || !(text.startsWith("{") || text.startsWith("["))) {
            return ok;
        }
        try {
            return JSON.readTree(text);
        } catch (Exception e) {
            return ok;
        }
    }

    /**
     * The argument object, always carrying {@code agent_id}.
     *
     * <p>A {@link LinkedHashMap} rather than {@code Map.of}: optional
     * arguments are sent as explicit nulls, which {@code Map.of} rejects.</p>
     */
    private Map<String, Object> args(Object... keyValues) {
        Map<String, Object> args = new LinkedHashMap<>();
        args.put("agent_id", agentId());
        for (int i = 0; i + 1 < keyValues.length; i += 2) {
            args.put(String.valueOf(keyValues[i]), keyValues[i + 1]);
        }
        return args;
    }
}
