package com.weilliptic.weilwallet.transaction;

import com.weilliptic.weilwallet.OrgInfo;
import java.util.Map;
import java.util.TreeMap;

/**
 * The organization a transaction is signed under.
 *
 * <p>Carried in the signed header and surfaced to applets as {@code Runtime::org()}, so an applet
 * can scope itself to the caller's org without taking one as a method argument.</p>
 *
 * <p><b>The wire shape is load-bearing.</b> It must serialize exactly as the node's
 * {@code OrgContext} does — keys {@code org}/{@code subgroup}, with {@code "subgroup": null} when
 * absent rather than omitted. The signature is a SHA-256 over key-sorted JSON that both sides
 * rebuild independently, so a renamed or dropped key changes the digest and the node rejects the
 * transaction. This is why {@link #toMap()} always writes {@code subgroup}, and why this type is
 * serialized through an explicit map rather than by Jackson bean introspection.</p>
 */
public final class OrgContext {

    private final String org;
    private final String subgroup;

    public OrgContext(String org, String subgroup) {
        this.org = org;
        // Normalize an empty subgroup to null: org-level membership must always
        // produce one digest, never two.
        this.subgroup = (subgroup == null || subgroup.isEmpty()) ? null : subgroup;
    }

    /**
     * Maps the wallet-file {@link OrgInfo} onto the wire shape the node expects, or {@code null}
     * when the wallet has no active org.
     *
     * <p>{@code purpose} is intentionally dropped — it is advisory metadata resolved from the
     * Identity contract at runtime, not part of the signed claim.</p>
     */
    public static OrgContext fromOrgInfo(OrgInfo info) {
        if (info == null) {
            return null;
        }
        return new OrgContext(info.name(), info.subgroup());
    }

    public String org() {
        return org;
    }

    public String subgroup() {
        return subgroup;
    }

    /**
     * Digest/wire representation. {@code subgroup} is always present, {@code null} when absent.
     *
     * <p>Deliberately a {@code TreeMap}: Jackson serializes a map in its <i>iterator</i> order and
     * does not sort nested maps, so key order here has to be guaranteed by the map type — the same
     * reason the signed payload in {@code WeilClient.execute} uses {@code TreeMap} throughout.</p>
     */
    public Map<String, Object> toMap() {
        Map<String, Object> map = new TreeMap<>();
        map.put("org", org);
        map.put("subgroup", subgroup);
        return map;
    }
}
