package com.weilliptic.weilwallet.derived;

import org.bouncycastle.jce.provider.BouncyCastleProvider;

import java.security.MessageDigest;
import java.security.Security;
import java.util.Arrays;

/**
 * Keccak-256 for derived-account address (last 20 bytes, 0x-prefixed).
 */
public final class Keccak {

    static {
        if (Security.getProvider(BouncyCastleProvider.PROVIDER_NAME) == null) {
            Security.addProvider(new BouncyCastleProvider());
        }
    }

    private Keccak() {}

    public static byte[] keccak256(byte[] data) {
        try {
            MessageDigest digest = MessageDigest.getInstance("KECCAK-256", BouncyCastleProvider.PROVIDER_NAME);
            return digest.digest(data);
        } catch (Exception e) {
            throw new RuntimeException("Keccak-256 not available", e);
        }
    }

    /**
     * Derived-account address from uncompressed public key: Keccak256(pubkey minus 0x04), last 20 bytes, 0x-prefixed.
     */
    public static String pubkeyToDerivedAddress(byte[] pubkeyBytes) {
        if (pubkeyBytes.length == 65 && pubkeyBytes[0] == 0x04) {
            pubkeyBytes = Arrays.copyOfRange(pubkeyBytes, 1, 65);
        }
        byte[] hash = keccak256(pubkeyBytes);
        byte[] last20 = Arrays.copyOfRange(hash, hash.length - 20, hash.length);
        StringBuilder sb = new StringBuilder("0x");
        for (byte b : last20) {
            sb.append(String.format("%02x", b & 0xff));
        }
        return sb.toString();
    }
}
