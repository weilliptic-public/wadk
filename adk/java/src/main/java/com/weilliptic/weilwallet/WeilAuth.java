package com.weilliptic.weilwallet;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.bitcoinj.core.ECKey;
import org.bitcoinj.core.Sha256Hash;

import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.util.Iterator;
import java.util.Map;
import java.util.TreeMap;

/**
 * Weil HTTP authentication helpers compatible with the Python SDK.
 */
public final class WeilAuth {

    public static final int MAX_TIMESTAMP_AGE_SECONDS = 300;
    private static final ObjectMapper JSON = new ObjectMapper();

    private WeilAuth() {}

    public static Map<String, String> buildHeaders(Wallet wallet) {
        return buildHeaders(wallet, null);
    }

    /**
     * Build the signed auth headers, optionally binding them to a request body.
     *
     * <p>Two addresses travel, because they answer different questions. The
     * 64-hex {@code sha256(public key)} goes in {@code X-Wallet-Address}: it is
     * the only form a server can re-derive from the signature, so it is what
     * proves key ownership. The canonical, sentinel-minted address carries a
     * pod-counter prefix that cannot be derived from a key at all, yet it is
     * what identity applets are keyed on — so it travels <em>inside the signed
     * message</em>, where {@link #canonicalAddress} can trust it.</p>
     *
     * @param wallet signing wallet
     * @param body   request body to bind, or null for no binding. When given,
     *               its SHA-256 is signed and a nonce is added, so a captured
     *               header set cannot be replayed against different content.
     */
    public static Map<String, String> buildHeaders(Wallet wallet, Object body) {
        try {
            String timestamp = String.valueOf(System.currentTimeMillis() / 1000L);
            Map<String, String> message = new TreeMap<>();
            message.put("timestamp", timestamp);
            message.put("address", wallet.getAddress());
            String bodyHash = null;
            String nonce = null;
            if (body != null) {
                bodyHash = bodySha256(body);
                nonce = randomNonce();
                message.put("body_sha256", bodyHash);
                message.put("nonce", nonce);
            }
            String json = JSON.writeValueAsString(message);
            String signature = wallet.sign(json.getBytes(StandardCharsets.UTF_8));

            Map<String, String> headers = new TreeMap<>();
            // The verifiable form: sha256(public key), re-derivable from the signature.
            headers.put("X-Wallet-Address",
                Utils.bytesToHex(Utils.hashSha256(wallet.getPublicKeyUncompressed())));
            headers.put("X-Signature", signature);
            headers.put("X-Message", json);
            headers.put("X-Timestamp", timestamp);
            if (bodyHash != null) {
                headers.put("X-Body-SHA256", bodyHash);
                headers.put("X-Nonce", nonce);
            }
            return headers;
        } catch (Exception e) {
            throw new RuntimeException("failed to build Weil auth headers", e);
        }
    }

    /**
     * The signed canonical (72-hex) address, when it is trustworthy.
     *
     * <p>Only the SHA-256 suffix of that address is re-derivable from the
     * signature, so the pod-counter prefix is accepted only when the remainder
     * matches the already-verified {@code X-Wallet-Address}. That binds the
     * claimed pod counter to the key whose ownership the signature proved.</p>
     *
     * <p>Returns null for header sets that predate this field, or that fail the
     * binding check; callers should then fall back to {@code X-Wallet-Address}.
     * Call only on headers that already passed {@link #verifyHeaders}.</p>
     */
    public static String canonicalAddress(String walletAddress, String message) {
        if (walletAddress == null || message == null) {
            return null;
        }
        try {
            String full = JSON.readTree(message).path("address").asText(null);
            if (full == null || full.isEmpty()) {
                return null;
            }
            return full.toLowerCase().endsWith(walletAddress.toLowerCase()) ? full : null;
        } catch (Exception e) {
            return null;
        }
    }

    /**
     * Verify that a request body matches the hash signed into its headers.
     *
     * @return true when the headers carry no body binding, or the binding holds
     */
    public static boolean verifyBody(Map<String, String> headers, Object body) {
        try {
            String message = header(headers, "X-Message");
            String signed = JSON.readTree(message).path("body_sha256").asText(null);
            if (signed == null || signed.isEmpty()) {
                return body == null;
            }
            return signed.equals(header(headers, "X-Body-SHA256"))
                && signed.equals(bodySha256(body));
        } catch (Exception e) {
            return false;
        }
    }

    /**
     * Case-insensitive header lookup.
     *
     * <p>HTTP header names are case-insensitive, and servers normalise them
     * differently: Java's {@code HttpExchange} turns {@code X-Body-SHA256} into
     * {@code X-Body-Sha256}, while an ASGI server hands them over lowercased. An
     * exact-match lookup silently misses, which reads as a failed signature.</p>
     */
    public static String header(Map<String, String> headers, String name) {
        if (headers == null || name == null) {
            return null;
        }
        String exact = headers.get(name);
        if (exact != null) {
            return exact;
        }
        for (Map.Entry<String, String> e : headers.entrySet()) {
            if (name.equalsIgnoreCase(e.getKey())) {
                return e.getValue();
            }
        }
        return null;
    }

    /**
     * SHA-256 of the canonical encoding of {@code body}: a string or byte array
     * as its UTF-8 bytes, anything else as {@link #canonicalJson}.
     */
    public static String bodySha256(Object body) {
        try {
            byte[] bytes;
            if (body instanceof byte[] raw) {
                bytes = raw;
            } else if (body instanceof String text) {
                bytes = text.getBytes(StandardCharsets.UTF_8);
            } else {
                bytes = canonicalJson(body).getBytes(StandardCharsets.UTF_8);
            }
            return Utils.bytesToHex(Utils.hashSha256(bytes));
        } catch (Exception e) {
            throw new RuntimeException("failed to hash request body", e);
        }
    }

    /**
     * {@code value} as the Python SDK encodes a body before hashing it:
     * {@code json.dumps(value, separators=(",", ":"), sort_keys=True)}.
     *
     * <p>Keys sorted at every depth, no whitespace, and every non-ASCII
     * character escaped as a four-digit {@code \\u} escape. A signature made on one side is
     * checked by re-hashing on the other, so the two encodings must agree byte
     * for byte, or a Java agent and a Python agent reject each other.</p>
     */
    public static String canonicalJson(Object value) {
        return pythonJson(value, true, ",", ":");
    }

    /**
     * {@code value} as Python's {@code json.dumps(value)} writes it with its
     * defaults: keys in their own order, {@code ", "} and {@code ": "} between
     * items, non-ASCII escaped. The form the Python SDK publishes payloads in.
     */
    public static String pythonJson(Object value) {
        return pythonJson(value, false, ", ", ": ");
    }

    /**
     * {@code value} as Python's {@code json.dumps(value, sort_keys=True)}
     * writes it: keys sorted, {@code ", "} and {@code ": "} between items,
     * non-ASCII escaped.
     */
    public static String sortedPythonJson(Object value) {
        return pythonJson(value, true, ", ", ": ");
    }

    private static String pythonJson(Object value, boolean sortKeys, String itemSep, String keySep) {
        JsonNode node = value instanceof JsonNode n ? n : JSON.valueToTree(value);
        StringBuilder out = new StringBuilder();
        writeCanonical(node, out, sortKeys, itemSep, keySep);
        return out.toString();
    }

    private static void writeCanonical(
        JsonNode node, StringBuilder out, boolean sortKeys, String itemSep, String keySep) {
        if (node == null || node.isNull() || node.isMissingNode()) {
            out.append("null");
        } else if (node.isObject()) {
            Map<String, JsonNode> fields = sortKeys ? new TreeMap<>() : new java.util.LinkedHashMap<>();
            for (Iterator<Map.Entry<String, JsonNode>> it = node.fields(); it.hasNext(); ) {
                Map.Entry<String, JsonNode> e = it.next();
                fields.put(e.getKey(), e.getValue());
            }
            out.append('{');
            boolean first = true;
            for (Map.Entry<String, JsonNode> e : fields.entrySet()) {
                if (!first) {
                    out.append(itemSep);
                }
                first = false;
                writeString(e.getKey(), out);
                out.append(keySep);
                writeCanonical(e.getValue(), out, sortKeys, itemSep, keySep);
            }
            out.append('}');
        } else if (node.isArray()) {
            out.append('[');
            for (int i = 0; i < node.size(); i++) {
                if (i > 0) {
                    out.append(itemSep);
                }
                writeCanonical(node.get(i), out, sortKeys, itemSep, keySep);
            }
            out.append(']');
        } else if (node.isTextual()) {
            writeString(node.asText(), out);
        } else if (node.isBoolean()) {
            out.append(node.booleanValue() ? "true" : "false");
        } else if (node.isIntegralNumber()) {
            out.append(node.bigIntegerValue().toString());
        } else if (node.isNumber()) {
            out.append(pythonFloat(node.doubleValue()));
        } else {
            writeString(node.asText(), out);
        }
    }

    /** A string as Python's {@code json.dumps} writes it with ensure_ascii. */
    private static void writeString(String s, StringBuilder out) {
        out.append('"');
        for (int i = 0; i < s.length(); i++) {
            char c = s.charAt(i);
            switch (c) {
                case '"' -> out.append("\\\"");
                case '\\' -> out.append("\\\\");
                case '\n' -> out.append("\\n");
                case '\r' -> out.append("\\r");
                case '\t' -> out.append("\\t");
                case '\b' -> out.append("\\b");
                case '\f' -> out.append("\\f");
                default -> {
                    // Non-ASCII goes out as UTF-16 escapes, surrogate pairs
                    // included, which is what Python emits for astral chars.
                    if (c < 0x20 || c > 0x7e) {
                        out.append(String.format("\\u%04x", (int) c));
                    } else {
                        out.append(c);
                    }
                }
            }
        }
        out.append('"');
    }

    /** A double as Python's {@code repr}: {@code 1.0}, {@code 1e-05}, {@code 1e+20}. */
    private static String pythonFloat(double d) {
        if (Double.isNaN(d)) {
            return "NaN";
        }
        if (Double.isInfinite(d)) {
            return d > 0 ? "Infinity" : "-Infinity";
        }
        double abs = Math.abs(d);
        if (abs != 0 && (abs < 1e-4 || abs >= 1e16)) {
            String sci = new java.math.BigDecimal(Double.toString(d)).stripTrailingZeros()
                .toString().replace("E", "e");
            int e = sci.indexOf('e');
            String mantissa = sci.substring(0, e);
            String exp = sci.substring(e + 1);
            if (!exp.startsWith("-") && !exp.startsWith("+")) {
                exp = "+" + exp;
            }
            String sign = exp.substring(0, 1);
            String digits = exp.substring(1);
            if (digits.length() < 2) {
                digits = "0" + digits;
            }
            return mantissa + "e" + sign + digits;
        }
        String plain = new java.math.BigDecimal(Double.toString(d)).stripTrailingZeros()
            .toPlainString();
        return plain.contains(".") ? plain : plain + ".0";
    }

    /**
     * The part of an A2A JSON-RPC body that a Weil signature covers.
     *
     * <p>The A2A message itself, {@code params.message}, rather than the
     * envelope: its {@code id} is assigned by the client transport after the
     * caller has signed. Mirrors the Python SDK's
     * {@code _signed_payload_from_http_body}: {@code params.message}, then
     * {@code params.request}, then {@code params}, then a top-level
     * {@code message} or {@code request}, then the whole body.</p>
     */
    public static JsonNode a2aSignedPayload(JsonNode envelope) {
        if (envelope == null || !envelope.isObject()) {
            return envelope;
        }
        JsonNode params = envelope.get("params");
        if (params != null && params.isObject()) {
            for (String key : new String[] {"message", "request"}) {
                JsonNode value = params.get(key);
                if (value != null && value.isObject()) {
                    return value;
                }
            }
            return params;
        }
        for (String key : new String[] {"message", "request"}) {
            JsonNode value = envelope.get(key);
            if (value != null && value.isObject()) {
                return value;
            }
        }
        return envelope;
    }

    private static String randomNonce() {
        byte[] bytes = new byte[16];
        new java.security.SecureRandom().nextBytes(bytes);
        return Utils.bytesToHex(bytes);
    }

    public static boolean verifyHeaders(
        String walletAddress,
        String signatureHex,
        String message,
        String timestamp
    ) {
        try {
            long ts = Long.parseLong(timestamp);
            if (Math.abs((System.currentTimeMillis() / 1000L) - ts) > MAX_TIMESTAMP_AGE_SECONDS) {
                return false;
            }

            byte[] sigBytes = Utils.hexToBytes(signatureHex);
            if (sigBytes.length != 64) {
                return false;
            }

            BigInteger r = new BigInteger(1, slice(sigBytes, 0, 32));
            BigInteger s = new BigInteger(1, slice(sigBytes, 32, 64));
            ECKey.ECDSASignature signature = new ECKey.ECDSASignature(r, s);
            byte[] digest = Utils.hashSha256(message.getBytes(StandardCharsets.UTF_8));
            Sha256Hash hash = Sha256Hash.wrap(digest);

            for (int recoveryId = 0; recoveryId <= 1; recoveryId++) {
                ECKey key = ECKey.recoverFromSignature(recoveryId, signature, hash, false);
                if (key == null) {
                    continue;
                }
                String derived = Utils.bytesToHex(Utils.hashSha256(key.getPubKeyPoint().getEncoded(false)));
                // Only the trailing 64 hex chars are the key hash; a caller
                // may send its full address there, as the Python SDK allows.
                String claimed = walletAddress.toLowerCase();
                if (claimed.length() > 64) {
                    claimed = claimed.substring(claimed.length() - 64);
                }
                if (derived.equalsIgnoreCase(claimed)) {
                    return true;
                }
            }
            return false;
        } catch (Exception e) {
            return false;
        }
    }

    private static byte[] slice(byte[] value, int start, int end) {
        byte[] out = new byte[end - start];
        System.arraycopy(value, start, out, 0, out.length);
        return out;
    }
}
