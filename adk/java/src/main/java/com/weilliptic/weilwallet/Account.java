package com.weilliptic.weilwallet;

import org.bitcoinj.core.ECKey;
import org.bitcoinj.core.Sha256Hash;

import java.math.BigInteger;
import java.util.Arrays;

/**
 * A single WeilChain account: secp256k1 keypair + sentinel-minted address.
 *
 * <p>For external accounts the address is the sentinel-minted 72-char hex string
 * supplied at construction time. Addresses cannot be derived from the private key alone.</p>
 *
 * <p>Signs with ECDSA secp256k1 over the SHA-256 digest of the input.
 * Signature is 64-byte compact (r || s), hex-encoded.</p>
 */
public final class Account {

    private static final int COMPACT_SIG_LEN = 32;

    private final ECKey secretKey;
    /** Sentinel-minted 72-char hex account address. */
    private final String accountAddress;

    /**
     * Create an Account from an ECKey and pre-minted account address.
     *
     * @param secretKey       The secp256k1 key pair for signing.
     * @param accountAddress  The sentinel-minted 72-char hex account address.
     */
    public Account(ECKey secretKey, String accountAddress) {
        this.secretKey = secretKey;
        this.accountAddress = accountAddress;
    }

    /**
     * Factory: build from a hex-encoded {@link PrivateKey} and a pre-minted sentinel address.
     *
     * @param privateKey      The secp256k1 private key.
     * @param accountAddress  The sentinel-minted 72-char hex account address.
     * @return A new Account instance.
     */
    public static Account fromPrivateKeyAndAddress(PrivateKey privateKey, String accountAddress) {
        ECKey key = ECKey.fromPrivate(privateKey.toBytes());
        return new Account(key, accountAddress);
    }

    /**
     * Return the sentinel-minted 72-char hex account address.
     */
    public String getAddress() {
        return accountAddress;
    }

    /**
     * Return the account's secp256k1 public key (uncompressed 65 bytes for wire format).
     */
    public byte[] getPublicKeyUncompressed() {
        return secretKey.getPubKeyPoint().getEncoded(false);
    }

    /**
     * Return the underlying ECKey for low-level operations.
     */
    public ECKey getSecretKey() {
        return secretKey;
    }

    /**
     * Sign buf with ECDSA secp256k1. Message is hashed with SHA-256, then signed.
     * Returns hex-encoded 64-byte compact signature (r || s).
     */
    public String sign(byte[] buf) {
        byte[] digest = Utils.hashSha256(buf);
        Sha256Hash hash = Sha256Hash.wrap(digest);
        ECKey.ECDSASignature sig = secretKey.sign(hash);
        byte[] r = bigIntegerToBytes32(sig.r);
        byte[] s = bigIntegerToBytes32(sig.s);
        byte[] compact = new byte[64];
        System.arraycopy(r, 0, compact, 0, COMPACT_SIG_LEN);
        System.arraycopy(s, 0, compact, COMPACT_SIG_LEN, COMPACT_SIG_LEN);
        return Utils.bytesToHex(compact);
    }

    private static byte[] bigIntegerToBytes32(BigInteger n) {
        byte[] bytes = n.toByteArray();
        if (bytes.length > COMPACT_SIG_LEN) {
            return Arrays.copyOfRange(bytes, bytes.length - COMPACT_SIG_LEN, bytes.length);
        }
        if (bytes.length < COMPACT_SIG_LEN) {
            byte[] padded = new byte[COMPACT_SIG_LEN];
            System.arraycopy(bytes, 0, padded, COMPACT_SIG_LEN - bytes.length, bytes.length);
            return padded;
        }
        return bytes;
    }
}
