package com.weilliptic.weilwallet;

import org.bitcoinj.core.ECKey;

/**
 * A single WeilChain account: secp256k1 keypair + sentinel-minted address.
 */
public final class Account {
    private final ECKey ecKey;
    private final String address;

    public Account(ECKey ecKey, String address) {
        this.ecKey = ecKey;
        this.address = address;
    }

    public ECKey getEcKey() {
        return ecKey;
    }

    public String getAddress() {
        return address;
    }
}

