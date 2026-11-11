package com.weilliptic.weilwallet.adk;

/**
 * What a blocking action's <em>infrastructure</em> failure does to its hook.
 *
 * <p>A failure here means the call could not be made or its arguments could not
 * be built. A deliberate rejection from the action's result handler always
 * vetoes, whatever this policy says.</p>
 */
public enum FailurePolicy {

    /** Report the failure and let the hook proceed. */
    FAIL_OPEN,

    /** Fail the hook. For a before-hook this stops the step. */
    FAIL_CLOSED
}
