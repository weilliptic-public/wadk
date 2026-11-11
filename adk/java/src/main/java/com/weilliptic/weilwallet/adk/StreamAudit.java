package com.weilliptic.weilwallet.adk;

import com.weilliptic.weilwallet.OrgInfo;
import com.weilliptic.weilwallet.WeilAuth;

import java.util.LinkedHashMap;
import java.util.Map;

/**
 * Publishes each tool call's arguments to a WeilStream topic before the tool
 * runs: the Java counterpart of the Python SDK's {@code stream_audit}.
 *
 * <p>As in Python, the entry goes to the organisation's own publisher,
 * {@code publisher::<org>}, on topic {@code audits} unless another is named,
 * and its payload is the arguments as Python's {@code json.dumps} writes them:
 * {@code {"origin": "Delhi", "date": "15 November 2026"}}. It is signed by the
 * acting agent, so the stream records who was asked to do what.</p>
 *
 * <p>Python waits for the publish before running the function, and a failed
 * publish stops the call. {@link #on(String)} does the same: blocking, fail
 * closed. Override either on {@link #builder(String)}:</p>
 *
 * <pre>{@code
 * WeilPlugin.builder(identity)
 *     .withActions(StreamAudit.on("audits"))                     // as Python
 *     .withActions(StreamAudit.builder("audits").nonBlocking().done())  // never stops a tool
 *     .build();
 * }</pre>
 *
 * <p>The topic must exist, and the wallet must belong to an organisation.</p>
 */
public final class StreamAudit {

    /** The topic Python's {@code stream_audit} publishes to when none is named. */
    public static final String DEFAULT_TOPIC = "audits";

    private StreamAudit() {}

    /** Publish every tool call's arguments to {@link #DEFAULT_TOPIC}, as Python does. */
    public static WeilAction defaults() {
        return on(DEFAULT_TOPIC);
    }

    /** Publish every tool call's arguments to {@code topic}, waiting for each. */
    public static WeilAction on(String topic) {
        return builder(topic).done();
    }

    /**
     * The action preset, to adjust: {@code .nonBlocking()} so tools never
     * wait, {@code .failOpen()} so a failed publish does not stop the tool,
     * {@code .when(...)} to stream only some tools.
     */
    public static WeilAction.Builder builder(String topic) {
        if (topic == null || topic.isBlank()) {
            throw new IllegalArgumentException("stream_audit topic must be non-empty");
        }
        return WeilAction.named("stream_audit:" + topic)
            .on(Hook.BEFORE_TOOL)
            .blocking()
            .failClosed()
            .build(ev -> {
                Map<String, Object> args = new LinkedHashMap<>();
                args.put("topic", topic);
                args.put("payload", payload(ev.toolArgs()));
                return Call.to(Applet.byName(publisherFor(ev.identity())), "publish")
                    .args(args)
                    .hideArgs(false)
                    // Waits for the publish, as Python does, unless the
                    // action is non-blocking on a queued Dispatcher.
                    .fireAndForget(false)
                    .build();
            });
    }

    /**
     * Publish {@code arguments} to {@code topic} as {@code identity}, now, for
     * code outside an agent: a request handler, say, as Python's decorator is
     * used on FastAPI routes.
     */
    public static WeilResult publish(WeilIdentity identity, String topic, Map<String, ?> arguments)
        throws Exception {
        Map<String, Object> args = new LinkedHashMap<>();
        args.put("topic", topic);
        args.put("payload", payload(arguments));
        return identity.invoke(Call.to(Applet.byName(publisherFor(identity)), "publish")
            .args(args)
            .hideArgs(false)
            .fireAndForget(false)
            .build());
    }

    /** The arguments, written as Python's {@code json.dumps} writes them. */
    static String payload(Map<String, ?> arguments) {
        return WeilAuth.pythonJson(arguments == null ? Map.of() : arguments);
    }

    /** {@code publisher::<org>}, the publisher Python's {@code stream_audit} uses. */
    public static String publisherFor(WeilIdentity identity) {
        OrgInfo org = identity == null ? null : identity.client().org();
        if (org == null || org.name() == null || org.name().isBlank()) {
            throw new IllegalStateException("publishing to WeilStream requires a wallet linked to an organization");
        }
        return "publisher::" + org.name();
    }
}
