package com.weilliptic.weilwallet.api;

/**
 * Verifier type for transaction submission.
 */
public class Verifier {
    private String type = "DefaultVerifier";

    public Verifier() {}

    public String getType() { return type; }
    public void setType(String type) { this.type = type; }
}
