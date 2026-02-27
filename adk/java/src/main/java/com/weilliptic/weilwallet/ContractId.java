package com.weilliptic.weilwallet;

import org.apache.commons.codec.binary.Base32;

import java.nio.ByteBuffer;
import java.util.Objects;

/**
 * Contract ID (contract address) of a Weil Applet (smart contract).
 * Extracts WeilPod (shard) counter from base32-encoded value for routing.
 */
public final class ContractId {

    private static final int EXPECTED_DECODED_LEN = 36;

    private final String value;

    public ContractId(String contractId) {
        this.value = contractId != null ? contractId.trim() : "";
    }

    public static ContractId of(String contractId) {
        return new ContractId(contractId);
    }

    /**
     * Extract WeilPod (shard) counter from the contract ID for routing.
     * Decodes base32 (RFC 4648 lower, no padding), expects 36 bytes, first 4 bytes big-endian as i32.
     */
    public int podCounter() {
        Base32 base32 = new Base32();
        String padded = value;
        if (padded.length() % 8 != 0) {
            padded = padded + "========".substring(0, (8 - value.length() % 8) % 8);
        }
        byte[] decoded = base32.decode(padded.toUpperCase());
        if (decoded == null || decoded.length != EXPECTED_DECODED_LEN) {
            throw new IllegalArgumentException(
                "invalid contract-id: expected " + EXPECTED_DECODED_LEN + " bytes long, got " + (decoded == null ? 0 : decoded.length) + " bytes");
        }
        return ByteBuffer.wrap(decoded, 0, 4).getInt();
    }

    @Override
    public String toString() {
        return value;
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (o == null || getClass() != o.getClass()) return false;
        ContractId that = (ContractId) o;
        return Objects.equals(value, that.value);
    }

    @Override
    public int hashCode() {
        return Objects.hash(value);
    }
}
