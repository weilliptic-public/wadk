package com.weilliptic.weilwallet;

import com.fasterxml.jackson.databind.ObjectMapper;
import org.bitcoinj.core.ECKey;

import java.io.ByteArrayOutputStream;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.Map;
import java.util.zip.GZIPOutputStream;

/**
 * Cryptographic and utility helpers: SHA-256, address from public key, timestamp.
 */
public final class Utils {

    private Utils() {}

    public static byte[] hashSha256(byte[] buf) {
        try {
            MessageDigest md = MessageDigest.getInstance("SHA-256");
            return md.digest(buf);
        } catch (NoSuchAlgorithmException e) {
            throw new RuntimeException("SHA-256 not available", e);
        }
    }

    /**
     * Derive address from secp256k1 public key.
     * Address is hex-encoded SHA-256 of the uncompressed (65-byte) public key.
     */
    public static String getAddressFromPublicKey(ECKey publicKey) {
        byte[] full = publicKey.getPubKeyPoint().getEncoded(false);
        byte[] addr = hashSha256(full);
        return bytesToHex(addr);
    }

    public static long currentTimeMillis() {
        return System.currentTimeMillis();
    }

    public static String bytesToHex(byte[] bytes) {
        StringBuilder sb = new StringBuilder(bytes.length * 2);
        for (byte b : bytes) {
            sb.append(String.format("%02x", b & 0xff));
        }
        return sb.toString();
    }

    public static byte[] hexToBytes(String hex) {
        String s = hex.replaceAll("^0x", "").trim();
        if (s.length() % 2 != 0) {
            throw new IllegalArgumentException("Hex string must have even length");
        }
        byte[] out = new byte[s.length() / 2];
        for (int i = 0; i < out.length; i++) {
            int idx = i * 2;
            out[i] = (byte) Integer.parseInt(s.substring(idx, idx + 2), 16);
        }
        return out;
    }

    private static final ObjectMapper JSON = new ObjectMapper();

    /** Serialize map to JSON and GZIP-compress the bytes. */
    public static byte[] compress(Map<String, Object> value) {
        try {
            byte[] jsonBytes = JSON.writeValueAsString(value).getBytes(java.nio.charset.StandardCharsets.UTF_8);
            ByteArrayOutputStream baos = new ByteArrayOutputStream();
            try (GZIPOutputStream gzip = new GZIPOutputStream(baos)) {
                gzip.write(jsonBytes);
            }
            return baos.toByteArray();
        } catch (Exception e) {
            throw new RuntimeException("compress failed", e);
        }
    }
}
