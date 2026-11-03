package com.weilliptic.weilwallet;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.google.adk.JsonBaseModel;
import com.google.adk.tools.AgentTool;
import com.google.genai.types.Content;
import com.google.genai.types.Part;

import com.weilliptic.weilwallet.adk.Applet;
import com.weilliptic.weilwallet.adk.Call;
import com.weilliptic.weilwallet.adk.Hook;
import com.weilliptic.weilwallet.adk.StreamAudit;
import com.weilliptic.weilwallet.adk.WeilAction;
import com.weilliptic.weilwallet.adk.WeilEvent;
import com.weilliptic.weilwallet.adk.WeilIdentity;
import com.weilliptic.weilwallet.adk.WeilPlugin;
import com.weilliptic.weilwallet.adk.WeilResult;

import java.util.ArrayList;
import java.util.Deque;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.UUID;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.ConcurrentLinkedDeque;

/**
 * One trace per agent run on the {@code weilchain::weil} applet, as the
 * Python SDK's {@code weil_ai.tracer} keeps it: the user's prompt as the
 * root, then every tool call as a node with its arguments and result,
 * published as one entry on the organisation's {@code audits} topic when the
 * run ends.
 *
 * <p>The trace is a graph, not a list. Each node is the applet's
 * {@code TraceEvent}, {@code {id, parent_id, event_type, content}}, and
 * {@code parent_id} says what it follows: null for the prompt itself.
 * Tools the model asks for in one response are siblings under the same
 * parent, whether ADK runs them one after another or in parallel; a tool the
 * model asks for later follows the last tool that finished before it.</p>
 *
 * <ol>
 *   <li><b>Run start</b> ({@link Hook#USER_MESSAGE}): a new trace id, and
 *       {@code add_trace(trace_id, root, [])} with the user's message as the
 *       root. The applet records the signing wallet as the trace's invoker.</li>
 *   <li><b>Each model response</b> ({@link Hook#AFTER_MODEL}): the tools it
 *       asks for will hang off the last tool that finished, or off the
 *       prompt when none has.</li>
 *   <li><b>Each tool</b> ({@link Hook#BEFORE_TOOL}, then {@link Hook#AFTER_TOOL}):
 *       a node id and parent before it runs, then {@code build_trace(trace_id,
 *       event)} after, with {@code event_type} {@code tool_call} and
 *       {@code content} {@code {"args", "result", "tool"}} as Python's
 *       {@code json.dumps(..., sort_keys=True)} writes it. The arguments carry
 *       {@code "_weil": {"framework": "adk", "tool_call_id": ...}}. After the
 *       tool, so its result is in the trace; a refused tool's refusal is its
 *       result.</li>
 *   <li><b>Run end</b> ({@link Hook#AFTER_RUN}, or {@link Hook#RUN_ERROR}):
 *       {@code get_trace}, which returns the trace drawn as a Mermaid
 *       flowchart, then {@code index_trace} for similarity search (not waited
 *       for), then that flowchart published to {@code publisher::<org>} on
 *       topic {@code audits} (not waited for).</li>
 * </ol>
 *
 * <p>Sub-agents an agent transfers to run inside the same run, so they add
 * to its trace. A sub-agent behind an {@link AgentTool} that is handed the
 * plugin runs as a run of its own; like Python's nested trace, it adds a
 * {@code subtrace} node, {@code {"root": request}}, under the tool call that
 * started it, and its tools follow that node in the same trace. A trace
 * whose start failed records nothing more, since {@code build_trace} would
 * otherwise create an empty one with no prompt. Every step is best effort: a
 * failure is reported through the plugin's {@code onError} and never stops
 * the agent. Starting and each node wait for the chain, one at a time per
 * trace.</p>
 */
public final class TraceAction {

    /** The applet traces are kept on: {@code weilchain::weil}. */
    public static final Applet TRACER = Applet.systemic("weilchain");

    /** The topic a finished trace is published on, as in the Python SDK. */
    public static final String TOPIC = "audits";

    private static final ObjectMapper JSON = new ObjectMapper();

    /** Signs and submits a call as an identity; a seam for tests. */
    @FunctionalInterface
    interface Invoker {
        WeilResult invoke(WeilIdentity identity, Call call) throws Exception;
    }

    /** One run: its own trace, or a part of another run's when an {@link AgentTool} started it. */
    private static final class Run {
        /** The run whose trace this is: this run, unless an {@link AgentTool} started it. */
        final Run owner;
        final String traceId;
        final String root;
        /** Held while a call to the applet for this trace is in flight. */
        final Object submit = new Object();
        volatile boolean started;

        // Guarded by owner.
        /** The last node this run finished, which the model's next tools follow. */
        String last;
        /** The parent of the tools the model asked for in its latest response. */
        String turnParent;
        final Map<String, Node> calls = new HashMap<>();

        /** A run with a trace of its own. */
        Run(String root) {
            this.owner = this;
            this.traceId = UUID.randomUUID().toString();
            this.root = root;
        }

        /** A run that adds to {@code owner}'s trace, its tools following {@code base}. */
        Run(Run owner, String root, String base) {
            this.owner = owner;
            this.traceId = owner.traceId;
            this.root = root;
            this.last = base;
            this.turnParent = base;
        }

        boolean ownsTrace() {
            return owner == this;
        }
    }

    /** A tool call's node, decided before the tool runs. */
    private record Node(String id, String parentId, Pending child) {}

    /** An {@link AgentTool} call whose sub-agent run has not started yet. */
    private record Pending(String root, Run run, String parentId) {}

    private final Map<String, Run> runs = new ConcurrentHashMap<>();
    /** {@link AgentTool} calls in flight, by the request their sub-agent will start with. */
    private final Map<String, Deque<Pending>> pending = new ConcurrentHashMap<>();
    private final Invoker invoker;

    private TraceAction(Invoker invoker) {
        this.invoker = invoker;
    }

    /** The actions that trace every run, for {@link WeilPlugin.Builder#withActions}. */
    public static WeilAction[] actions() {
        return new TraceAction(WeilIdentity::invoke).build();
    }

    /** As {@link #actions()}, submitting through {@code invoker}. */
    static WeilAction[] actions(Invoker invoker) {
        return new TraceAction(invoker).build();
    }

    private WeilAction[] build() {
        // Each action decides in its build step and submits its own calls, so
        // it returns no Call for the plugin to send. Failing open: a trace
        // that cannot be written is reported, and the agent carries on.
        return new WeilAction[] {
            WeilAction.named("trace:start").on(Hook.USER_MESSAGE).blocking().failOpen()
                .build(ev -> {
                    start(ev);
                    return null;
                })
                .done(),
            WeilAction.named("trace:turn").on(Hook.AFTER_MODEL).blocking().failOpen()
                .build(ev -> {
                    turn(ev);
                    return null;
                })
                .done(),
            WeilAction.named("trace:tool-start").on(Hook.BEFORE_TOOL).blocking().failOpen()
                .build(ev -> {
                    beforeTool(ev);
                    return null;
                })
                .done(),
            WeilAction.named("trace:tool").on(Hook.AFTER_TOOL, Hook.TOOL_ERROR).blocking().failOpen()
                .build(ev -> {
                    afterTool(ev);
                    return null;
                })
                .done(),
            WeilAction.named("trace:end").on(Hook.AFTER_RUN, Hook.RUN_ERROR).blocking().failOpen()
                .build(ev -> {
                    finish(ev);
                    return null;
                })
                .done(),
        };
    }

    private void start(WeilEvent ev) throws Exception {
        String runId = ev.invocationId();
        if (runId == null || ev.identity() == null || runs.containsKey(runId)) {
            return; // already traced: this run's own trace takes the events
        }
        String root = root(ev);
        Pending parent = claim(root);
        if (parent != null) {
            startChild(ev, runId, root, parent);
            return;
        }
        Run run = new Run(root);
        if (runs.putIfAbsent(runId, run) != null) {
            return;
        }
        Map<String, Object> args = new LinkedHashMap<>();
        args.put("trace_id", run.traceId);
        args.put("root", root);
        args.put("trace", List.of());
        invoker.invoke(ev.identity(), call("add_trace", args, false)).decode();
        run.started = true;
    }

    /** A sub-agent run behind an {@link AgentTool}: a {@code subtrace} node in its caller's trace. */
    private void startChild(WeilEvent ev, String runId, String root, Pending parent) throws Exception {
        Run owner = parent.run().owner;
        // A sub-agent asked the trace's own question adds no node, as in Python.
        if (root.equals(owner.root)) {
            runs.putIfAbsent(runId, new Run(owner, root, parent.parentId()));
            return;
        }
        String nodeId = UUID.randomUUID().toString();
        if (runs.putIfAbsent(runId, new Run(owner, root, nodeId)) != null) {
            return;
        }
        Map<String, Object> content = new LinkedHashMap<>();
        content.put("root", root);
        submit(ev.identity(), owner, node(nodeId, parent.parentId(), "subtrace", content));
    }

    private void turn(WeilEvent ev) {
        Run run = run(ev);
        if (run == null) {
            return;
        }
        synchronized (run.owner) {
            run.turnParent = run.last;
        }
    }

    private void beforeTool(WeilEvent ev) {
        Run run = run(ev);
        if (run == null || !run.owner.started) {
            return;
        }
        Pending child = null;
        String id = UUID.randomUUID().toString();
        synchronized (run.owner) {
            if (ev.tool() instanceof AgentTool) {
                child = new Pending(childRoot(ev.toolArgs()), run, id);
            }
            run.calls.put(callKey(ev), new Node(id, run.turnParent, child));
        }
        if (child != null) {
            pending.computeIfAbsent(child.root(), k -> new ConcurrentLinkedDeque<>()).add(child);
        }
    }

    private void afterTool(WeilEvent ev) throws Exception {
        Run run = run(ev);
        if (run == null || !run.owner.started) {
            return;
        }
        Node node;
        synchronized (run.owner) {
            node = run.calls.remove(callKey(ev));
            if (node == null) {
                // A tool refused before this trace saw it start.
                node = new Node(UUID.randomUUID().toString(), run.turnParent, null);
            }
            if (ev.hook() == Hook.AFTER_TOOL) {
                run.last = node.id();
            }
        }
        if (node.child() != null) {
            Deque<Pending> waiting = pending.get(node.child().root());
            if (waiting != null) {
                waiting.remove(node.child());
            }
        }
        if (ev.hook() != Hook.AFTER_TOOL) {
            return; // a failed tool ends the run, which publishes what it has
        }
        Map<String, Object> content = new LinkedHashMap<>();
        content.put("tool", ev.toolName());
        content.put("args", jsonable(withMetadata(ev.toolArgs(), ev.toolCallId())));
        content.put("result", jsonable(ev.toolResult()));
        submit(ev.identity(), run.owner, node(node.id(), node.parentId(), "tool_call", content));
    }

    private void finish(WeilEvent ev) throws Exception {
        Run run = ev.invocationId() == null ? null : runs.remove(ev.invocationId());
        if (run == null || !run.ownsTrace() || !run.started) {
            return; // a sub-agent's run is published with its caller's
        }
        WeilIdentity identity = ev.identity();
        Map<String, Object> byId = Map.of("trace_id", run.traceId);
        JsonNode trace = parsed(invoker.invoke(identity, call("get_trace", byId, false)).decode());

        // Indexing is follow-up work: sent, not waited for, and its failure
        // does not stop the trace from being published.
        Exception indexFailure = null;
        try {
            invoker.invoke(identity, call("index_trace", byId, true));
        } catch (Exception e) {
            indexFailure = e;
        }

        Map<String, Object> publish = new LinkedHashMap<>();
        publish.put("topic", TOPIC);
        publish.put("payload", WeilAuth.pythonJson(trace));
        invoker.invoke(identity, Call.to(Applet.byName(StreamAudit.publisherFor(identity)), "publish")
            .args(publish)
            .hideArgs(false)
            .fireAndForget(true)
            .build());
        if (indexFailure != null) {
            throw indexFailure;
        }
    }

    /** {@code build_trace} for one node; one at a time per trace, so nodes land in the order they finish. */
    private void submit(WeilIdentity identity, Run owner, Map<String, Object> event) throws Exception {
        Map<String, Object> args = new LinkedHashMap<>();
        args.put("trace_id", owner.traceId);
        args.put("event", event);
        synchronized (owner.submit) {
            invoker.invoke(identity, call("build_trace", args, false)).decode();
        }
    }

    /** The applet's {@code TraceEvent} record. */
    private static Map<String, Object> node(String id, String parentId, String type, Map<String, Object> content) {
        Map<String, Object> event = new LinkedHashMap<>();
        event.put("id", id);
        event.put("parent_id", parentId);
        event.put("event_type", type);
        event.put("content", WeilAuth.sortedPythonJson(content));
        return event;
    }

    private Run run(WeilEvent ev) {
        return ev.invocationId() == null ? null : runs.get(ev.invocationId());
    }

    /** The oldest {@link AgentTool} call waiting for a sub-agent run that starts with {@code root}. */
    private Pending claim(String root) {
        Deque<Pending> waiting = pending.get(root);
        return waiting == null ? null : waiting.pollFirst();
    }

    /** Pairs a tool's start with its end: the model's call id, else the call's own context. */
    private static String callKey(WeilEvent ev) {
        String id = ev.toolCallId();
        return id != null ? "call:" + id
            : "context:" + System.identityHashCode(ev.callbackContext()) + ":" + ev.toolName();
    }

    /** The arguments with the framework's call id under {@code _weil}, as Python records them. */
    private static Object withMetadata(Map<String, Object> args, String callId) {
        if (callId == null) {
            return args;
        }
        Map<String, Object> out = new LinkedHashMap<>();
        if (args != null) {
            out.putAll(args);
        } else {
            out.put("value", null);
        }
        Map<String, Object> weil = new LinkedHashMap<>();
        weil.put("framework", "adk");
        weil.put("tool_call_id", callId);
        out.put("_weil", weil);
        return out;
    }

    /** The message {@link AgentTool} starts its sub-agent with: the {@code request}, or all arguments as JSON. */
    static String childRoot(Map<String, Object> args) {
        if (args != null && args.size() == 1 && args.get("request") instanceof String request) {
            return request;
        }
        try {
            return JsonBaseModel.getMapper().writeValueAsString(args == null ? Map.of() : args);
        } catch (Exception e) {
            return String.valueOf(args);
        }
    }

    private static Call call(String method, Map<String, Object> args, boolean fireAndForget) {
        return Call.to(TRACER, method).args(args).hideArgs(false).fireAndForget(fireAndForget).build();
    }

    /** The user's prompt, or the run's id when the message carries no text. */
    static String root(WeilEvent ev) {
        Content message = ev.userMessage();
        List<String> texts = new ArrayList<>();
        if (message != null) {
            for (Part part : message.parts().orElse(List.of())) {
                part.text().filter(t -> !t.isBlank()).ifPresent(texts::add);
            }
        }
        if (!texts.isEmpty()) {
            return String.join("\n", texts);
        }
        return WeilAuth.canonicalJson(Map.of("invocation_id", String.valueOf(ev.invocationId())));
    }

    /** {@code value} as JSON can carry it, or its text when it cannot. */
    private static Object jsonable(Object value) {
        if (value == null) {
            return null;
        }
        try {
            return JSON.valueToTree(value);
        } catch (IllegalArgumentException e) {
            return String.valueOf(value);
        }
    }

    /** The applet returns its answer as text inside {@code Ok}; read it once more when it is JSON. */
    private static JsonNode parsed(JsonNode value) throws Exception {
        if (value != null && value.isTextual()) {
            String text = value.asText().trim();
            if (text.startsWith("{") || text.startsWith("[")) {
                return JSON.readTree(text);
            }
        }
        return value;
    }
}
