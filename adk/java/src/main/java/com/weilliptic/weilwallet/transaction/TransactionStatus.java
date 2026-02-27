package com.weilliptic.weilwallet.transaction;

/**
 * Lifecycle states for a transaction.
 */
public enum TransactionStatus {
    InProgress("InProgress"),
    Confirmed("Confirmed"),
    Finalized("Finalized"),
    Failed("Failed");

    private final String value;

    TransactionStatus(String value) {
        this.value = value;
    }

    public String getValue() {
        return value;
    }

    public static TransactionStatus fromString(String s) {
        if (s == null) return InProgress;
        for (TransactionStatus status : values()) {
            if (status.value.equals(s)) return status;
        }
        return InProgress;
    }
}
