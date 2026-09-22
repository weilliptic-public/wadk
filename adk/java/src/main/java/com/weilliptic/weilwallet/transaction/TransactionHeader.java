package com.weilliptic.weilwallet.transaction;

import com.weilliptic.weilwallet.Utils;

/**
 * Transaction header: nonce, public key, addresses, signature, weilpod counter, creation time, salt.
 */
public class TransactionHeader {

    private final long nonce;
    private final String publicKey;
    private final String fromAddr;
    private final String toAddr;
    private String signature;
    private final int weilpodCounter;
    private long creationTime;
    // Random UUIDv4. Covered by the signature (see WeilClient.execute) and
    // mixed into the node's get_txn_id(), so two transactions never collide
    // on id even if nonce happens to match.
    private final String salt;

    /**
     * Create a transaction header. A {@code creationTime} of 0 is replaced
     * with the current time.
     */
    public TransactionHeader(long nonce, String publicKey, String fromAddr, String toAddr,
                             String signature, int weilpodCounter, long creationTime, String salt) {
        this.nonce = nonce;
        this.publicKey = publicKey;
        this.fromAddr = fromAddr;
        this.toAddr = toAddr;
        this.signature = signature;
        this.weilpodCounter = weilpodCounter;
        this.creationTime = creationTime != 0 ? creationTime : (long) Utils.currentTimeMillis();
        this.salt = salt;
    }

    public long getNonce() { return nonce; }
    public String getPublicKey() { return publicKey; }
    public String getFromAddr() { return fromAddr; }
    public String getToAddr() { return toAddr; }
    public String getSignature() { return signature; }
    public int getWeilpodCounter() { return weilpodCounter; }
    public long getCreationTime() { return creationTime; }
    public String getSalt() { return salt; }

    public void setSignature(String signature) {
        this.signature = signature;
    }

    public void setCreationTime(long creationTime) {
        this.creationTime = creationTime;
    }
}
