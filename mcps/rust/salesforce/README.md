# Salesforce MCP Servcer

The Salesforce MCP provides a secure interface to interact with Salesforce CRM through natural language queries and CRUD operations.


## Core Tools
1. Create Record (`create`)
2. Read Record (`read`)
3. Update Record (`update`)
4. Delete Record (`delete`)
5. SOQL Query (`execute_soql_query`)
6. Object Metadata (`get_object_metadata`)

## Testing

```
deploy -f /root/code/weilliptic/mcp_vault/rust/salesforce/target/wasm32-unknown-unknown/release/salesforce.wasm -p /root/code/weilliptic/mcp_vault/rust/salesforce/salesforce.widl -c /root/code/weilliptic/platform-scripts/contracts/configs/salesforce.yaml
```

### `config.yaml`
```yaml
client_id: 
client_secret: 
username: 
password: 
security_token: 
```

### Example Prompts

- Show me the Name of all entries in Account in my salesforce
- Find all the entries in the Opportunity section present in my salesforce account
- Show me the schema for the Lead object
- Create a new Account named Jake Jyllenhall with phone 555-0123 and website jake.com
- Add a new Contact named Harry Smith with email harry@example.com and phone 555-0456
- Can u run all tests in my Salesforce account and show me their job Ids?
- Can u tell me the status of the test with job id as got before?
- can u show me the most recent event in my salesforce

