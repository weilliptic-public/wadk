package com.weilliptic.weilwallet.transaction;

import com.fasterxml.jackson.annotation.JsonIgnoreProperties;

/**
 * Canonical result envelope returned by the chain for a submitted transaction.
 */
@JsonIgnoreProperties(ignoreUnknown = true)
public class TransactionResult {

    private TransactionStatus status = TransactionStatus.InProgress;
    private long blockHeight = 0;
    private String batchId = "";
    private String batchAuthor = "";
    private int txIdx = 0;
    private String txnResult = "";
    private String creationTime = "";

    public TransactionResult() {}

    public TransactionResult(TransactionStatus status, long blockHeight, String batchId,
                             String batchAuthor, int txIdx, String txnResult, String creationTime) {
        this.status = status;
        this.blockHeight = blockHeight;
        this.batchId = batchId;
        this.batchAuthor = batchAuthor;
        this.txIdx = txIdx;
        this.txnResult = txnResult;
        this.creationTime = creationTime != null ? creationTime : "";
    }

    public static TransactionResult fromJson(java.util.Map<String, Object> data) {
        String statusStr = data.containsKey("status") ? String.valueOf(data.get("status")) : "InProgress";
        return new TransactionResult(
            TransactionStatus.fromString(statusStr),
            data.containsKey("block_height") ? ((Number) data.get("block_height")).longValue() : 0,
            data.containsKey("batch_id") ? String.valueOf(data.get("batch_id")) : "",
            data.containsKey("batch_author") ? String.valueOf(data.get("batch_author")) : "",
            data.containsKey("tx_idx") ? ((Number) data.get("tx_idx")).intValue() : 0,
            data.containsKey("txn_result") ? String.valueOf(data.get("txn_result")) : "",
            data.containsKey("creation_time") ? String.valueOf(data.get("creation_time")) : ""
        );
    }

    public TransactionStatus getStatus() { return status; }
    public void setStatus(TransactionStatus status) { this.status = status; }
    public long getBlockHeight() { return blockHeight; }
    public void setBlockHeight(long blockHeight) { this.blockHeight = blockHeight; }
    public String getBatchId() { return batchId; }
    public void setBatchId(String batchId) { this.batchId = batchId; }
    public String getBatchAuthor() { return batchAuthor; }
    public void setBatchAuthor(String batchAuthor) { this.batchAuthor = batchAuthor; }
    public int getTxIdx() { return txIdx; }
    public void setTxIdx(int txIdx) { this.txIdx = txIdx; }
    public String getTxnResult() { return txnResult; }
    public void setTxnResult(String txnResult) { this.txnResult = txnResult; }
    public String getCreationTime() { return creationTime; }
    public void setCreationTime(String creationTime) { this.creationTime = creationTime; }
}
