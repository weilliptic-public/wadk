package com.weilliptic.weilwallet.adk;

import com.google.adk.agents.BaseAgent;
import com.google.adk.agents.InvocationContext;
import com.google.adk.agents.LlmAgent;
import com.google.adk.tools.AgentTool;
import com.google.adk.tools.BaseTool;
import com.google.adk.tools.FunctionTool;

import com.weilliptic.weilwallet.WeilClient;

import java.lang.reflect.Field;
import java.util.ArrayList;
import java.util.Collections;
import java.util.IdentityHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.function.Predicate;

/**
 * A blocking gate that lets a tool run only when a wallet holds Execution or
 * Management purpose in an organisation subgroup.
 *
 * <p>The authorization decision is made from on-chain state rather than from a
 * prompt instruction, so the model's intent is irrelevant: a vetoed
 * {@link Hook#BEFORE_TOOL} means the tool never runs, and the model is handed
 * {@code {"error": "Permission denied: ..."}} as the tool's result, which it
 * can explain.</p>
 *
 * <pre>{@code
 * WeilPlugin.builder(identity)
 *     .withActions(PermissionGate.on("engg.weil", identity))
 *     .build();
 * }</pre>
 *
 * <p>{@link #on} mirrors the Python SDK's {@code @secured_user};
 * {@link #admin} mirrors {@code @secured_admin} and accepts Management only.</p>
 *
 * <p>The wallet checked is the caller's: on an agent served by
 * {@link WeilAgentServer}, the wallet the server verified for the request
 * (see {@link WeilAgentServer#callerWallet}). A run nobody called over A2A,
 * such as a local conversation, has no caller, and the agent's own wallet is
 * checked instead.</p>
 */
public final class PermissionGate {

    /** The WeilStream topic each check's verdict is published to, as in the Python SDK. */
    public static final String SECURED_CHECK_TOPIC = "security";

    private PermissionGate() {}

    /** The purposes a tool gate asks: {@code admin} takes Management alone. */
    static String toolPurposes(boolean admin) {
        return admin ? "Management" : "Execution or Management";
    }

    /**
     * The publish call recording an agent's A2A check on
     * {@link #SECURED_CHECK_TOPIC}, naming the purpose the caller passed on,
     * or both when it failed:
     *
     * <ul>
     *   <li>{@code "<wallet> passed @secured Execution check for agent <agent> in <service>"},
     *       or {@code Management} when that is what it holds;</li>
     *   <li>{@code "<wallet> failed @secured Execution and Management checks for agent ..."}.</li>
     * </ul>
     *
     * <p>Signed by whoever submits it, which is the agent that made the check;
     * the payload names the wallet that was checked.</p>
     *
     * @param grantedBy the purpose the wallet holds, or null when refused
     */
    public static Call agentVerdictCall(
        String checkedWallet, String agentName, String svcName, String grantedBy) {
        return verdictCall(checkedWallet, "agent " + agentName, svcName, false, grantedBy);
    }

    /**
     * The publish call recording a tool's check on {@link #SECURED_CHECK_TOPIC},
     * naming the purpose the wallet passed on, or every purpose it failed:
     *
     * <ul>
     *   <li>{@code "<wallet> passed @secured Execution check for tool <tool> in <service>"},
     *       or {@code Management} when that is what it holds;</li>
     *   <li>{@code "<wallet> failed @secured Execution and Management checks for tool ..."}
     *       for a user tool;</li>
     *   <li>{@code "<wallet> failed @secured Management check for tool ..."}
     *       for an {@code admin} tool.</li>
     * </ul>
     *
     * @param grantedBy the purpose the wallet holds, or null when refused
     */
    public static Call toolVerdictCall(
        String checkedWallet, String toolName, String svcName, boolean admin, String grantedBy) {
        return verdictCall(checkedWallet, "tool " + (toolName == null ? "(unnamed)" : toolName),
            svcName, admin, grantedBy);
    }

    /**
     * Published through the checked service's own organisation,
     * {@code publisher::<org>} ("finance.weil" goes to publisher::weil), as
     * the Python SDK publishes its verdicts.
     */
    private static Call verdictCall(
        String checkedWallet, String target, String svcName, boolean admin, String grantedBy) {
        String checks = grantedBy != null ? grantedBy + " check"
            : admin ? "Management check" : "Execution and Management checks";
        return Call.to(Applet.byName(publisherFor(svcName)), "publish")
            .args(Map.of(
                "topic", SECURED_CHECK_TOPIC,
                "payload", checkedWallet + " " + (grantedBy != null ? "passed" : "failed")
                    + " @secured " + checks + " for " + target + " in " + svcName))
            .hideArgs(false)
            .fireAndForget(true)
            .build();
    }

    /** {@code publisher::<org>} for a {@code "<subgroup>.<org>"} service. */
    static String publisherFor(String svcName) {
        String svc = svcName.trim();
        int dot = svc.lastIndexOf('.');
        return "publisher::" + (dot >= 0 ? svc.substring(dot + 1) : svc);
    }

    /**
     * Gate every tool call on {@code svcName}, checking the caller's wallet.
     *
     * <p>{@code svcName} is a subgroup within an organisation, such as
     * "engg.weil", which is resolved against that organisation's identity
     * applet — not the name of an applet itself.</p>
     */
    public static WeilAction on(String svcName, WeilIdentity identity) {
        return on(svcName, identity, null, null);
    }

    /** Gate the tools matching {@code when} on {@code svcName}. */
    public static WeilAction on(
        String svcName,
        WeilIdentity identity,
        String sentinelHost,
        Predicate<WeilEvent> when) {
        return gate(svcName, identity, sentinelHost, when, true);
    }

    /**
     * Gate every tool call on {@code svcName}, admitting only a wallet that
     * holds Management purpose. An Execution key is refused.
     */
    public static WeilAction admin(String svcName, WeilIdentity identity) {
        return admin(svcName, identity, null, null);
    }

    /** Gate the tools matching {@code when} on Management purpose in {@code svcName}. */
    public static WeilAction admin(
        String svcName,
        WeilIdentity identity,
        String sentinelHost,
        Predicate<WeilEvent> when) {
        return gate(svcName, identity, sentinelHost, when, false);
    }

    private static WeilAction gate(
        String svcName,
        WeilIdentity identity,
        String sentinelHost,
        Predicate<WeilEvent> when,
        boolean allowExecution) {

        // The permission check is a read-only query, not an applet call made as
        // this agent, so it runs in the action's own build step rather than
        // through Call/invoke. Returning null means "nothing to submit".
        return WeilAction.named((allowExecution ? "permission:" : "admin-permission:") + svcName)
            .on(Hook.BEFORE_TOOL)
            .when(when)
            .blocking()
            .failClosed()
            .build(ev -> {
                check(ev, identity, svcName, sentinelHost, allowExecution);
                return null;
            })
            .done();
    }

    /**
     * Gate every tool whose method carries {@link WeilSecured}, on the
     * service and level the annotation names, and every agent tool made by
     * {@link Weil#agentTool(BaseAgent, String)}, on its service. Other tools
     * pass untouched.
     *
     * <p>Also refuses an {@link AgentTool} made without plugins, ADK's
     * default, whose agent holds secured tools: ADK runs that agent with no
     * plugin, so nothing would check them. Make such a tool with
     * {@link Weil#agentTool} instead.</p>
     */
    public static WeilAction annotated(WeilIdentity identity, String sentinelHost) {
        return WeilAction.named("permission:@WeilSecured")
            .on(Hook.BEFORE_TOOL)
            .when(ev -> requirementOf(ev.tool()) != null || !uncheckedInside(ev.tool()).isEmpty())
            .blocking()
            .failClosed()
            .build(ev -> {
                List<String> unchecked = uncheckedInside(ev.tool());
                if (!unchecked.isEmpty()) {
                    String message = ev.toolName() + " runs its agent without the Weil plugin, so "
                        + String.join(", ", unchecked) + " would run with no permission check."
                        + " Make it with Weil.agentTool(agent), or AgentTool.create(agent,"
                        + " skipSummarization, true).";
                    System.err.println("[weil] refused " + message);
                    throw new WeilAction.VetoException("Refused: " + message);
                }
                Requirement required = requirementOf(ev.tool());
                check(ev, identity, required.service(), sentinelHost, !required.admin());
                return null;
            })
            .done();
    }

    /**
     * The {@code @WeilSecured} tools an {@link AgentTool} would run unchecked:
     * those anywhere in its agent's tree when it was made without plugins, and
     * none otherwise.
     */
    static List<String> uncheckedInside(BaseTool tool) {
        if (!(tool instanceof AgentTool agentTool) || includesPlugins(agentTool)) {
            return List.of();
        }
        List<String> secured = new ArrayList<>();
        collectSecured(agentTool.getAgent(), Collections.newSetFromMap(new IdentityHashMap<>()), secured);
        return secured;
    }

    /**
     * Whether ADK hands this tool's agent the caller's plugins. ADK keeps the
     * flag private; if it cannot be read, the tool counts as without them,
     * so a secured tool inside is refused rather than run unchecked.
     */
    private static boolean includesPlugins(AgentTool tool) {
        try {
            Field field = AgentTool.class.getDeclaredField("includePlugins");
            field.setAccessible(true);
            return field.getBoolean(tool);
        } catch (ReflectiveOperationException | RuntimeException e) {
            return false;
        }
    }

    /**
     * Adds the name of every secured tool reachable from {@code agent}: its
     * own tools, the agents behind its agent tools, and its sub-agents.
     * Nothing under a runner without plugins is checked, whatever the nested
     * agent tools ask for, so all of them count.
     */
    private static void collectSecured(BaseAgent agent, Set<BaseAgent> seen, List<String> secured) {
        if (agent == null || !seen.add(agent)) {
            return;
        }
        if (agent instanceof LlmAgent llm) {
            for (Object tool : llm.toolsUnion()) {
                if (tool instanceof BaseTool base && requirementOf(base) != null) {
                    secured.add(base.name());
                }
                if (tool instanceof AgentTool nested) {
                    collectSecured(nested.getAgent(), seen, secured);
                }
            }
        }
        for (BaseAgent sub : agent.subAgents()) {
            collectSecured(sub, seen, secured);
        }
    }

    /** The permission a tool asks for: a service, and whether only Management will do. */
    record Requirement(String service, boolean admin) {}

    /**
     * What {@code tool} requires: the {@link WeilSecured} on its method, or
     * the service of an agent tool made by {@link Weil#agentTool(BaseAgent,
     * String)}; null for a tool that requires nothing.
     */
    static Requirement requirementOf(BaseTool tool) {
        if (tool instanceof SecuredAgentTool agentTool) {
            return new Requirement(agentTool.service, false);
        }
        if (tool instanceof FunctionTool function && function.func() != null) {
            WeilSecured secured = function.func().getAnnotation(WeilSecured.class);
            return secured == null ? null : new Requirement(secured.value(), secured.admin());
        }
        return null;
    }

    /** Ask the chain, publish the verdict, and veto the tool when it is refused. */
    private static void check(
        WeilEvent ev, WeilIdentity identity, String svcName, String sentinelHost,
        boolean allowExecution) throws Exception {
        String wallet = checkedWallet(ev, identity);
        String grantedBy = grantedBy(wallet, svcName, sentinelHost, allowExecution);
        publishVerdict(identity,
            toolVerdictCall(wallet, ev.toolName(), svcName, !allowExecution, grantedBy));
        if (grantedBy == null) {
            throw new WeilAction.VetoException(
                "Permission denied: wallet " + wallet + " does not have permission to run "
                    + (ev.toolName() == null ? "this tool" : ev.toolName()) + ". It needs "
                    + toolPurposes(!allowExecution) + " permission in " + svcName + ".");
        }
    }

    /**
     * The purpose that lets {@code wallet} in, or null when none does.
     * Execution is asked first, as a calling agent normally holds it;
     * Management passes an agent or a user tool too, and is all an admin
     * tool takes.
     */
    static String grantedBy(
        String wallet, String svcName, String sentinelHost, boolean allowExecution)
        throws Exception {
        if (allowExecution && WeilClient.walletHasExecutionPermission(wallet, svcName, sentinelHost)) {
            return "Execution";
        }
        if (WeilClient.walletHasManagementPermission(wallet, svcName, sentinelHost)) {
            return "Management";
        }
        return null;
    }

    /** The served request's verified caller, else the wallet this run acts as. */
    private static String checkedWallet(WeilEvent ev, WeilIdentity identity) {
        InvocationContext ctx = ev.invocationContext() != null
            ? ev.invocationContext()
            : ev.callbackContext() != null ? ev.callbackContext().invocationContext() : null;
        String caller = WeilAgentServer.callerWallet(ctx);
        if (caller != null) {
            return caller;
        }
        return ev.identity() != null ? ev.identity().address() : identity.address();
    }

    private static final Dispatcher VERDICTS = Dispatcher.inline();

    /**
     * Publish the verdict before the gate answers, so it cannot be lost with
     * the process. It is sent fire-and-forget, so the gate waits for the
     * platform to accept it, not for the chain. A verdict that still fails is
     * reported, and changes nothing: it is not a reason to refuse or allow.
     */
    private static void publishVerdict(WeilIdentity identity, Call verdict) {
        VERDICTS.submit(identity, verdict, (result, error) -> {
            if (error != null) {
                System.err.println("[weil] permission verdict not published: " + error);
            }
        });
    }
}
