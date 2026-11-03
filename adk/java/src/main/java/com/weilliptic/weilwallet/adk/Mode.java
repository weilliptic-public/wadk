package com.weilliptic.weilwallet.adk;

/**
 * Whether a hook waits for an action's applet call.
 */
public enum Mode {

    /**
     * Never stops a step: failures are reported to the plugin's error handler,
     * and the action cannot veto. With the plugin's default inline
     * {@link Dispatcher} the call is still made before the hook returns, so
     * none is lost; with a queued one it is made in the background and the
     * agent never waits.
     *
     * <p>Use for logging, auditing and telemetry.</p>
     */
    NON_BLOCKING,

    /**
     * Run the call inside the hook and wait for it. The action can then veto
     * the step from the applet's answer.
     *
     * <p>Use for permissions, spending limits and approvals.</p>
     */
    BLOCKING
}
