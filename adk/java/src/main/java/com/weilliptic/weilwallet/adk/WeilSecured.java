package com.weilliptic.weilwallet.adk;

import java.lang.annotation.Documented;
import java.lang.annotation.ElementType;
import java.lang.annotation.Retention;
import java.lang.annotation.RetentionPolicy;
import java.lang.annotation.Target;

/**
 * Lets a tool method run only for a wallet permitted in a subgroup, as the
 * Python SDK's {@code @secured_user} and {@code @secured_admin} do.
 *
 * <pre>{@code
 * @WeilSecured("engg.weil")                      // Execution or Management
 * public static Map<String, Object> searchFlights(...) { ... }
 *
 * @WeilSecured(value = "finance.weil", admin = true)   // Management only
 * public static Map<String, Object> payInvoice(...) { ... }
 * }</pre>
 *
 * <p>Read from the method behind a {@link com.google.adk.tools.FunctionTool}
 * by the plugin {@link Weil} builds (or {@link PermissionGate#annotated}),
 * just before the tool runs. The wallet checked is the local agent's own;
 * on a served agent the A2A caller is checked once, at the server's service
 * gate, not here. Tools with no method to annotate, such
 * as an MCP server's, take an explicit {@link PermissionGate} instead.</p>
 */
@Documented
@Retention(RetentionPolicy.RUNTIME)
@Target(ElementType.METHOD)
public @interface WeilSecured {

    /**
     * The subgroup and organisation, {@code "<subgroup>.<org>"}, such as
     * {@code "engg.weil"}, or a boolean rule over several, such as
     * {@code "infrastructure.weil & !finance.weil"}, with the same meaning as
     * the Rust {@code #[secured_user]} macro (see
     * {@link com.weilliptic.weilwallet.SecuredRule}).
     */
    String value();

    /** Management purpose only, as {@code @secured_admin}; otherwise Execution or Management. */
    boolean admin() default false;
}
