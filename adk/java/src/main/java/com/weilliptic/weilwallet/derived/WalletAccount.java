package com.weilliptic.weilwallet.derived;

import com.weilliptic.weilwallet.PrivateKey;
import com.weilliptic.weilwallet.Wallet;

/**
 * A single derived account: key material and derived address (0x-prefixed).
 */
public class WalletAccount {

    private final byte[] privateKey;
    private final byte[] publicKey;
    private final String address;

    public WalletAccount(byte[] privateKey, byte[] publicKey, String address) {
        this.privateKey = privateKey != null ? privateKey.clone() : new byte[0];
        this.publicKey = publicKey != null ? publicKey.clone() : new byte[0];
        this.address = address != null ? address : "";
    }

    public byte[] getPrivateKey() {
        return privateKey.clone();
    }

    public byte[] getPublicKey() {
        return publicKey.clone();
    }

    public String getAddress() {
        return address;
    }

    /** Return a Weil SDK Wallet that can sign and be used with WeilClient. */
    public Wallet toWeilWallet() {
        return new Wallet(PrivateKey.fromBytes(privateKey));
    }
}
