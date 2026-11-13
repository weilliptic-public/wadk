package com.weilliptic.weilwallet.adk;

import com.weilliptic.weilwallet.ContractId;
import com.weilliptic.weilwallet.WeilClient;

import java.io.IOException;
import java.util.Objects;

/**
 * Identifies an applet, either by contract address or by service name.
 *
 * <p>Mirrors {@code weiladk.Applet} in the Go SDK. Name lookups go through
 * Sentinel's {@code /get_applet_address}.</p>
 */
public final class Applet {

    /** Domain that systemic applets are registered under. */
    public static final String SYSTEMIC_DOMAIN = "weil";

    private final ContractId id;
    private final String name;

    private Applet(ContractId id, String name) {
        this.id = id;
        this.name = name;
    }

    /** Refer to an applet by its contract address. */
    public static Applet byId(ContractId contractId) {
        return new Applet(Objects.requireNonNull(contractId, "contractId"), null);
    }

    /** Refer to an applet by service name, e.g. {@code "counter::acme"}. */
    public static Applet byName(String svcName) {
        return new Applet(null, Objects.requireNonNull(svcName, "svcName"));
    }

    /** Shorthand for a systemic applet: {@code systemic("auditor")} is {@code auditor::weil}. */
    public static Applet systemic(String name) {
        return byName(name + "::" + SYSTEMIC_DOMAIN);
    }

    /** The systemic auditor applet. */
    public static Applet auditor() {
        return systemic("auditor");
    }

    /** The systemic Pensieve graph-memory applet. */
    public static Applet pensieve() {
        return systemic("pensieve");
    }

    public ContractId id() {
        return id;
    }

    public String name() {
        return name;
    }

    /**
     * Resolve to a contract address, asking Sentinel when this is a name reference.
     */
    public ContractId resolve(String sentinelHost, String walletAddr)
        throws IOException, InterruptedException {
        if (id != null) {
            return id;
        }
        return WeilClient.resolveApplet(name, sentinelHost, walletAddr);
    }

    @Override
    public String toString() {
        return id != null ? id.toString() : name;
    }
}
