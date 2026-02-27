package com.weilliptic.weilwallet;

import org.bitcoinj.core.ECKey;
import org.bitcoinj.core.Sha256Hash;

import java.math.BigInteger;
import java.util.Arrays;

/**
 * secp256k1-backed wallet for the WeilChain platform.
 * Signs with ECDSA secp256k1 over the SHA-256 digest of the input.
 * Signature is 64-byte compact (r || s), hex-encoded.
 */
public final class Wallet {

    private static final int COMPACT_SIG_LEN = 32;

    private final ECKey secretKey;

    public Wallet(PrivateKey privateKey) {
        this.secretKey = ECKey.fromPrivate(privateKey.toBytes());
    }

    /**
     * Return the account's secp256k1 public key (uncompressed 65 bytes for wire format).
     */
    public byte[] getPublicKeyUncompressed() {
        return secretKey.getPubKeyPoint().getEncoded(false);
    }

    public ECKey getECKey() {
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
