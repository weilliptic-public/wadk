package com.weilliptic.weilwallet.api;

import com.weilliptic.weilwallet.transaction.TransactionHeader;

/**
 * Full transaction for submission: header, verifier, user_txn.
 */
public class TransactionPayload {
    private boolean isXpod = false;
    private TransactionHeader txnHeader;
    private Verifier verifier;
    private UserTransaction userTxn;

    public TransactionPayload() {}

    public TransactionPayload(boolean isXpod, TransactionHeader txnHeader, Verifier verifier, UserTransaction userTxn) {
        this.isXpod = isXpod;
        this.txnHeader = txnHeader;
        this.verifier = verifier;
        this.userTxn = userTxn;
    }

    public boolean isXpod() { return isXpod; }
    public void setXpod(boolean xpod) { isXpod = xpod; }
    public TransactionHeader getTxnHeader() { return txnHeader; }
    public void setTxnHeader(TransactionHeader txnHeader) { this.txnHeader = txnHeader; }
    public Verifier getVerifier() { return verifier; }
    public void setVerifier(Verifier verifier) { this.verifier = verifier; }
    public UserTransaction getUserTxn() { return userTxn; }
    public void setUserTxn(UserTransaction userTxn) { this.userTxn = userTxn; }
}
