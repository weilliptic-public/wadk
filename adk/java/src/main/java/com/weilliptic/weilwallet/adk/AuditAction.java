package com.weilliptic.weilwallet.adk;

import com.fasterxml.jackson.databind.ObjectMapper;

import com.google.genai.types.Content;
import com.google.genai.types.Part;

import com.weilliptic.weilwallet.OrgInfo;

import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.time.format.DateTimeFormatter;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * Records agent activity through the systemic auditor applet, signed by the
 * acting agent's identity.
 *
 * <p>Mirrors {@code weiladk.AuditAction} in the Go SDK. This is an ordinary
 * {@link WeilAction}: combine it with others, or build a different trail on the
 * same primitives.</p>
 *
 * <p>By default it records <em>who did what, when</em> plus SHA-256 digests of
 * tool arguments and results rather than their contents, so a tamper-evident
 * trail can live on a public chain without publishing the payloads.</p>
 *
 * <p>{@link #withContent()} records the payloads as well: the user's prompt,
 * the model's reply, and tool arguments and results, each capped at
 * {@link #MAX_CONTENT_CHARS}. They are readable by anyone who can read the
 * chain, so turn it on only where that is acceptable.</p>
 */
public final class AuditAction {

    private static final ObjectMapper JSON = new ObjectMapper();

    /** The hooks recorded when none are given. */
    public static final List<Hook> DEFAULT_HOOKS = List.of(
        Hook.USER_MESSAGE,
        Hook.BEFORE_AGENT,
        Hook.AFTER_AGENT,
        Hook.AFTER_MODEL,
        Hook.BEFORE_TOOL,
        Hook.AFTER_TOOL,
        Hook.AFTER_RUN);

    /** Longest prompt, reply, argument or result text recorded by {@link #withContent()}. */
    public static final int MAX_CONTENT_CHARS = 4000;

    private AuditAction() {}

    /** The audit action with default hooks; a failed write is reported, never stops a step. */
    public static WeilAction defaults() {
        return builder().done();
    }

    /**
     * The audit action with default hooks, recording payloads in the clear:
     * the prompt on {@code user_message}, the reply on {@code after_model},
     * and tool arguments and results on the tool hooks.
     */
    public static WeilAction withContent() {
        return builder(Applet.auditor(), true).done();
    }

    /** The audit action over a chosen set of hooks. */
    public static WeilAction on(Hook... hooks) {
        return builder().on(hooks).done();
    }

    /**
     * A builder pre-wired to the auditor applet and the default record shape.
     *
     * <p>Override anything on it: {@code .on(...)} to change the hooks,
     * {@code .blocking().failClosed()} to refuse to proceed with unaudited
     * steps, {@code .build(...)} for a different record.</p>
     */
    public static WeilAction.Builder builder() {
        return builder(Applet.auditor());
    }

    /** As {@link #builder()}, against a different applet. */
    public static WeilAction.Builder builder(Applet applet) {
        return builder(applet, false);
    }

    /** As {@link #builder(Applet)}, recording payloads in the clear when {@code includeContent}. */
    public static WeilAction.Builder builder(Applet applet, boolean includeContent) {
        return WeilAction.named("audit")
            .on(DEFAULT_HOOKS)
            .nonBlocking()
            .build(ev -> {
                String log = JSON.writeValueAsString(record(ev, includeContent));
                Map<String, Object> args = new LinkedHashMap<>();
                args.put("log", log);
                // Attribute the entry to the organisation, as the Python SDK
                // does: the applet reads these as arguments rather than
                // re-deriving them from the signed header.
                OrgInfo org = ev.identity() != null ? ev.identity().client().org() : null;
                args.put("org", org != null ? org.name() : null);
                args.put("subgroup", org != null ? org.subgroup() : null);
                return Call.to(applet, "audit")
                    .args(args)
                    // Readable on chain. Hiding the arguments stores a
                    // commitment alone, which is all a reader gets back -- the
                    // record itself, with who/when/which tool, is then lost.
                    .hideArgs(false)
                    // Submitted without waiting for the chain to execute it,
                    // as the Python SDK does. A trail is not worth stalling
                    // the agent for: waiting puts a chain round-trip inside
                    // every hook, and a write that waits long enough has its
                    // deadline elapse before the node gets to it.
                    .fireAndForget(true)
                    .build();
            });
    }

    /**
     * The default audit entry: who acted, where in the run, and digests of the
     * payloads rather than the payloads themselves.
     */
    public static Map<String, Object> record(WeilEvent ev) {
        return record(ev, false);
    }

    /**
     * As {@link #record(WeilEvent)}, adding the payloads themselves when
     * {@code includeContent}. The digests stay either way, so a record with
     * content can still be checked against one without.
     */
    public static Map<String, Object> record(WeilEvent ev, boolean includeContent) {
        Map<String, Object> r = new LinkedHashMap<>();
        r.put("v", 1);
        r.put("hook", ev.hook().wireName());
        r.put("time", DateTimeFormatter.ISO_INSTANT.format(ev.time()));
        if (ev.agentName() != null) {
            r.put("agent", ev.agentName());
        }
        if (ev.invocationId() != null) {
            r.put("invocation", ev.invocationId());
        }
        if (ev.identity() != null) {
            r.put("wallet", ev.identity().address());
        }
        if (ev.toolName() != null) {
            r.put("tool", ev.toolName());
            r.put("args_sha256", hashJson(ev.toolArgs()));
            if (ev.toolResult() != null) {
                r.put("result_sha256", hashJson(ev.toolResult()));
            }
        }
        if (ev.llm() != null) {
            r.put("llm", ev.llm());
        }
        // The model that answered when the reply names it, otherwise the one
        // the agent asked for.
        String model = ev.llmResponse() == null
            ? null
            : ev.llmResponse().modelVersion().orElse(null);
        if (model == null) {
            model = ev.model();
        }
        if (model != null) {
            r.put("model", model);
        }
        if (ev.llmResponse() != null) {
            r.put("response_sha256", hashJson(ev.llmResponse().toJson()));
        }
        if (ev.error() != null) {
            r.put("error", String.valueOf(ev.error().getMessage()));
        }
        if (includeContent) {
            putContent(r, ev);
        }
        return r;
    }

    private static void putContent(Map<String, Object> r, WeilEvent ev) {
        if (ev.userMessage() != null) {
            putIfPresent(r, "prompt", cap(text(ev.userMessage())));
        }
        if (ev.toolName() != null) {
            putIfPresent(r, "args", capJson(ev.toolArgs()));
            putIfPresent(r, "result", capJson(ev.toolResult()));
        }
        Content reply = ev.llmResponse() == null ? null : ev.llmResponse().content().orElse(null);
        if (reply != null) {
            putIfPresent(r, "response", cap(text(reply)));
            // A turn that calls tools often says nothing; what it asked for is
            // the reply, so record that too.
            List<Map<String, Object>> calls = new ArrayList<>();
            for (Part part : reply.parts().orElse(List.of())) {
                part.functionCall().ifPresent(call -> {
                    Map<String, Object> c = new LinkedHashMap<>();
                    c.put("name", call.name().orElse(null));
                    c.put("args", capJson(call.args().orElse(Map.of())));
                    calls.add(c);
                });
            }
            if (!calls.isEmpty()) {
                r.put("tool_calls", calls);
            }
        }
    }

    /**
     * The text parts of {@code content}, joined. Not {@code Content.text()},
     * which logs a warning for every reply that also calls a tool.
     */
    private static String text(Content content) {
        StringBuilder sb = new StringBuilder();
        for (Part part : content.parts().orElse(List.of())) {
            part.text().ifPresent(sb::append);
        }
        return sb.toString();
    }

    private static void putIfPresent(Map<String, Object> r, String key, Object value) {
        if (value != null && !(value instanceof String s && s.isEmpty())) {
            r.put(key, value);
        }
    }

    /** {@code text}, cut to {@link #MAX_CONTENT_CHARS} with a note saying how long it was. */
    static String cap(String text) {
        if (text == null || text.length() <= MAX_CONTENT_CHARS) {
            return text;
        }
        return text.substring(0, MAX_CONTENT_CHARS)
            + "... [truncated, " + text.length() + " chars]";
    }

    /** {@code value} as is when its JSON fits, otherwise its JSON as capped text. */
    static Object capJson(Map<String, Object> value) {
        if (value == null) {
            return null;
        }
        try {
            String json = JSON.writeValueAsString(value);
            return json.length() <= MAX_CONTENT_CHARS ? value : cap(json);
        } catch (Exception e) {
            return cap(String.valueOf(value));
        }
    }

    /**
     * Hex SHA-256 of {@code value}'s JSON encoding, or null when it cannot be
     * encoded. Map keys are written in sorted order, so equal maps hash equally.
     */
    public static String hashJson(Object value) {
        if (value == null) {
            return null;
        }
        try {
            byte[] json = JSON
                .writer()
                .withFeatures(com.fasterxml.jackson.databind.SerializationFeature.ORDER_MAP_ENTRIES_BY_KEYS)
                .writeValueAsBytes(value);
            MessageDigest md = MessageDigest.getInstance("SHA-256");
            byte[] digest = md.digest(json);
            StringBuilder sb = new StringBuilder(digest.length * 2);
            for (byte b : digest) {
                sb.append(Character.forDigit((b >> 4) & 0xF, 16));
                sb.append(Character.forDigit(b & 0xF, 16));
            }
            return sb.toString();
        } catch (Exception e) {
            return null;
        }
    }

    static byte[] utf8(String s) {
        return s.getBytes(StandardCharsets.UTF_8);
    }
}
