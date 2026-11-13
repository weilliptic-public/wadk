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

    public static PrivateKey fromFile(String path) throws IOException {
        return fromFile(Paths.get(path));
    }

    public static PrivateKey fromFile(Path path) throws IOException {
        String content = new String(Files.readAllBytes(path)).trim().replaceAll("\\s+", "");
        if (content.isEmpty()) {
            throw new IllegalArgumentException("private key file is empty");
        }
        return new PrivateKey(content);
    }

    public static PrivateKey fromHex(String hexStr) {
        return new PrivateKey(hexStr);
    }

    public static PrivateKey fromBytes(byte[] keyBytes) {
        return new PrivateKey(Utils.bytesToHex(keyBytes));
    }

    public String getHex() {
        return hex;
    }

    public byte[] toBytes() {
        return Utils.hexToBytes(hex);
    }
}
