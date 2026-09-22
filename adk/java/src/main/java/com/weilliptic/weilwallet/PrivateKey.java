package com.weilliptic.weilwallet;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;

/**
 * Represents the private key associated with your account.
 */
public final class PrivateKey {

    private final String hex;

    /**
     * Create a private key from a hex string.
     * Whitespace is stripped; the key must be non-empty, even-length hex.
     *
     * @param hexStr the hex-encoded secp256k1 private key.
     * @throws IllegalArgumentException if the input is empty or not valid hex.
     */
    public PrivateKey(String hexStr) {
        String hexTrimmed = hexStr != null ? hexStr.trim().replaceAll("\\s+", "") : "";
        if (hexTrimmed.isEmpty()) {
            throw new IllegalArgumentException("private key is empty");
        }
        if (hexTrimmed.length() % 2 != 0 || !hexTrimmed.matches("^[0-9a-fA-F]+$")) {
            throw new IllegalArgumentException("private key is not a valid hexadecimal string");
        }
        this.hex = hexTrimmed;
    }

    /** Load a hex private key from a file path. */
    public static PrivateKey fromFile(String path) throws IOException {
        return fromFile(Paths.get(path));
    }

    /**
     * Load a hex private key from a file, stripping whitespace.
     *
     * @throws IllegalArgumentException if the file is empty.
     * @throws IOException              if the file cannot be read.
     */
    public static PrivateKey fromFile(Path path) throws IOException {
        String content = new String(Files.readAllBytes(path)).trim().replaceAll("\\s+", "");
        if (content.isEmpty()) {
            throw new IllegalArgumentException("private key file is empty");
        }
        return new PrivateKey(content);
    }

    /** Create a private key from a hex string. */
    public static PrivateKey fromHex(String hexStr) {
        return new PrivateKey(hexStr);
    }

    /** Create a private key from raw key bytes. */
    public static PrivateKey fromBytes(byte[] keyBytes) {
        return new PrivateKey(Utils.bytesToHex(keyBytes));
    }

    /** Return the hex-encoded private key. */
    public String getHex() {
        return hex;
    }

    /** Return the private key as raw bytes. */
    public byte[] toBytes() {
        return Utils.hexToBytes(hex);
    }
}
