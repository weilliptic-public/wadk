package com.weilliptic.weilwallet;

import org.bitcoinj.core.ECKey;

/**
 * A single WeilChain account: secp256k1 keypair + sentinel-minted address.
 *
 * <p>Derived accounts carry keys re-derived from the wallet's xprv at a
 * specific BIP32 index. External accounts carry their own independent key pairs
 * loaded from an account export file.</p>
 */
public final class Account {
    private final ECKey ecKey;
    private final String address;

    /**
     * Construct an account from an already-parsed EC key and its on-chain address.
     *
     * @param ecKey   secp256k1 key pair (private + public).
     * @param address sentinel-minted account address (72-character hex string).
     */
    public Account(ECKey ecKey, String address) {
        this.ecKey = ecKey;
        this.address = address;
    }

    /**
     * Return the secp256k1 EC key pair for this account.
     * Use {@link Wallet#getPublicKeyUncompressed()} for the wire-format public key.
     */
    public ECKey getEcKey() {
        return ecKey;
    }

    /**
     * Return the sentinel-minted on-chain address for this account.
     */
    public String getAddress() {
        return address;
    }
}

