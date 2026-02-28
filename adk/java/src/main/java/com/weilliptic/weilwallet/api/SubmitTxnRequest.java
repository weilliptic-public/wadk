package com.weilliptic.weilwallet.api;

import com.weilliptic.weilwallet.transaction.TransactionHeader;

import java.util.LinkedHashMap;
import java.util.Map;

/**
 * Request wrapper for submit transaction API. Serializes to the payload dict expected by the platform.
 */
public class SubmitTxnRequest {

    private TransactionPayload transaction;

    public SubmitTxnRequest() {}

    public SubmitTxnRequest(TransactionPayload transaction) {
        this.transaction = transaction;
    }

    public TransactionPayload getTransaction() { return transaction; }
    public void setTransaction(TransactionPayload transaction) { this.transaction = transaction; }

    /**
     * Build the JSON-serializable payload (with 'type' for serde compatibility).
     */
    @SuppressWarnings("unchecked")
    public Map<String, Object> toPayloadMap() {
        TransactionPayload txn = this.transaction;
        if (txn == null || txn.getTxnHeader() == null || txn.getVerifier() == null || txn.getUserTxn() == null) {
            throw new IllegalStateException("incomplete transaction");
        }
        TransactionHeader h = txn.getTxnHeader();
        Map<String, Object> txnHeader = new LinkedHashMap<>();
        txnHeader.put("nonce", h.getNonce());
        txnHeader.put("public_key", h.getPublicKey());
        txnHeader.put("from_addr", h.getFromAddr());
        txnHeader.put("to_addr", h.getToAddr());
        txnHeader.put("signature", h.getSignature());
        txnHeader.put("weilpod_counter", h.getWeilpodCounter());
        txnHeader.put("creation_time", h.getCreationTime());

        Map<String, Object> verifier = new LinkedHashMap<>();
        verifier.put("type", txn.getVerifier().getType());

        Map<String, Object> userTxn = new LinkedHashMap<>();
        userTxn.put("type", txn.getUserTxn().getType());
        userTxn.put("contract_address", txn.getUserTxn().getContractAddress().toString());
        userTxn.put("contract_method", txn.getUserTxn().getContractMethod());
        userTxn.put("contract_input_bytes", txn.getUserTxn().getContractInputBytes());
        userTxn.put("should_hide_args", txn.getUserTxn().isShouldHideArgs());

        Map<String, Object> transaction = new LinkedHashMap<>();
        transaction.put("is_xpod", txn.isXpod());
        transaction.put("txn_header", txnHeader);
        transaction.put("verifier", verifier);
        transaction.put("user_txn", userTxn);

        Map<String, Object> root = new LinkedHashMap<>();
        root.put("transaction", transaction);
        return root;
    }
}
