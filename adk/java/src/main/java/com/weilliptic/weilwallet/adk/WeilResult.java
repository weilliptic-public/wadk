package com.weilliptic.weilwallet.adk;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.weilliptic.weilwallet.ContractId;
import com.weilliptic.weilwallet.transaction.TransactionResult;

/**
 * The outcome of a {@link WeilIdentity#invoke(Call)}.
 *
 * <p>Applet methods return a {@code {"Ok": ...}} / {@code {"Err": ...}}
 * envelope; {@link #decode()} unwraps it so callers can tell "the contract
 * said no" from "the call failed".</p>
 */
public final class WeilResult {

    private static final ObjectMapper JSON = new ObjectMapper();

    private final Call call;
    private final ContractId contractId;
    private final TransactionResult txn;

    WeilResult(Call call, ContractId contractId, TransactionResult txn) {
        this.call = call;
        this.contractId = contractId;
        this.txn = txn;
    }

    public Call call() {
        return call;
    }

    public ContractId contractId() {
        return contractId;
    }

    public TransactionResult transaction() {
        return txn;
    }

    /** The raw {@code txn_result} payload, or null when the call was fire-and-forget. */
    public String raw() {
        return txn == null ? null : txn.getTxnResult();
    }

    /**
     * Unwrap the applet's {@code Ok} value.
     *
     * @throws AppletException when the applet returned {@code Err}
     */
    public JsonNode decode() throws AppletException {
        String raw = raw();
        if (raw == null || raw.isEmpty()) {
            return null;
        }
        JsonNode node;
        try {
            node = JSON.readTree(raw);
        } catch (Exception e) {
            throw new AppletException("applet returned unparseable result: " + raw, e);
        }
        if (node.has("Err")) {
            throw new AppletException("applet " + contractId + "." + call.method()
                + " returned Err: " + node.get("Err").toString(), null);
        }
        return node.has("Ok") ? node.get("Ok") : node;
    }

    /** Convenience: decode and read as a boolean, false when absent. */
    public boolean decodeBoolean() throws AppletException {
        JsonNode ok = decode();
        if (ok == null) {
            return false;
        }
        if (ok.isBoolean()) {
            return ok.asBoolean();
        }
        return "true".equalsIgnoreCase(ok.asText());
    }

    /** Thrown when the applet itself rejected the call, as opposed to a transport failure. */
    public static final class AppletException extends Exception {
        AppletException(String message, Throwable cause) {
            super(message, cause);
        }
    }
}
