# Sample Prompts & Expected Outputs

## 1. Full Incident Response

**Prompt:**
> Incident INC-042. Database connection pool exhausted. 1. create war room 2. get oncall 3. notify via sms and email 4. post to discord

**Expected output:**
- War room created with join/host URLs
- On-call engineer paged via PagerDuty; name/phone/email returned
- SMS sent to engineer's phone with war room link
- Email sent to engineer with incident details
- Discord notified with severity embed
- All 5 steps logged to the incident timeline

---

## 2. Status Page Update

**Prompt:**
> Update status page for INC-042 to "investigating" with message "We are aware of elevated DB errors and are actively investigating."

**Expected output:**
Status page updated: incident INC-042 set to 'investigating'

---

## 3. AI Remediation

**Prompt:**
> Run AI remediation for INC-042. Issue is database connection pool exhausted under high load. Severity: critical.

**Expected output:**
- Remediation steps suggested (e.g. increase pool size, restart connection broker, scale read replicas)
- Result logged as `AI_REMEDIATION` event on the timeline

---

## 4. Incident Timeline Query

**Prompt:**
> Show me the full timeline for INC-042.

**Expected output:**
Ordered list of logged events:
[WAR_ROOM_CREATED] War room created successfully
[ONCALL_ASSIGNED]  Oncall engineer assigned - Jane Doe
[SMS_SENT]         SMS sent to +1XXXXXXXXXX
[EMAIL_SENT]       Email sent to oncall@yourcompany.com
[DISCORD_NOTIFIED] Discord channel notified
[AI_REMEDIATION]   Remediation suggestions generated

---

## 5. List All Active Incidents

**Prompt:**
> List all incidents.

**Expected output:**
["INC-042", "INC-101", "INC-205"]