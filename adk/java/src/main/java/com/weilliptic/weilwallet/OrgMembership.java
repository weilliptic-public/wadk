package com.weilliptic.weilwallet;

import com.fasterxml.jackson.annotation.JsonCreator;
import com.fasterxml.jackson.annotation.JsonProperty;

/**
 * v2 org membership entry — no purpose (resolved at runtime).
 *
 * <p>Used internally when deserializing the {@code orgs} array from wallet files.</p>
 */
final class OrgMembership {

    private final String org;
    private final String subgroup;

    @JsonCreator
    OrgMembership(
            @JsonProperty("org") String org,
            @JsonProperty("subgroup") String subgroup) {
        this.org = org;
        this.subgroup = subgroup != null ? subgroup : "";
    }

    /** Organization name. */
    String org() {
        return org;
    }

    /** Subgroup within the organization (empty string when absent). */
    String subgroup() {
        return subgroup;
    }

    /**
     * Convert to the public {@link OrgInfo} type.
     */
    OrgInfo toOrgInfo() {
        String sub = subgroup.isEmpty() ? null : subgroup;
        return new OrgInfo(org, sub, "");
    }
}
