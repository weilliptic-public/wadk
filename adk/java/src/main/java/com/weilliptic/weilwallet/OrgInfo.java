package com.weilliptic.weilwallet;

import com.fasterxml.jackson.annotation.JsonCreator;
import com.fasterxml.jackson.annotation.JsonInclude;
import com.fasterxml.jackson.annotation.JsonProperty;

/**
 * Organization membership linked via {@code wallet link-org}.
 *
 * <p>Used for v1 wallet files (backward compat) and as the public return type.</p>
 */
@JsonInclude(JsonInclude.Include.NON_NULL)
public final class OrgInfo {

    private final String name;
    private final String subgroup;
    private final String purpose;

    @JsonCreator
    public OrgInfo(
            @JsonProperty("name") String name,
            @JsonProperty("subgroup") String subgroup,
            @JsonProperty("purpose") String purpose) {
        this.name = name;
        this.subgroup = subgroup;
        this.purpose = purpose != null ? purpose : "";
    }

    /** Organization name. */
    @JsonProperty("name")
    public String name() {
        return name;
    }

    /** Optional subgroup within the organization (may be {@code null}). */
    @JsonProperty("subgroup")
    public String subgroup() {
        return subgroup;
    }

    /** Purpose string (empty when resolved at runtime). */
    @JsonProperty("purpose")
    public String purpose() {
        return purpose;
    }

    @Override
    public String toString() {
        StringBuilder sb = new StringBuilder("OrgInfo{name='").append(name).append('\'');
        if (subgroup != null) {
            sb.append(", subgroup='").append(subgroup).append('\'');
        }
        if (!purpose.isEmpty()) {
            sb.append(", purpose='").append(purpose).append('\'');
        }
        sb.append('}');
        return sb.toString();
    }
}
