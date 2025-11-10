# SAP HANA

The SAP HANA MCP provides a secure interface to interact with SAP HANA databases through natural language queries and SQL operations.


## Core Tools
1. Database Schema (`schema`)
   - Purpose: Retrieve the complete schema of the SAP HANA database
   - Returns: Database schema information as string

2. Query Execution (`run_query`)
   - Purpose: Execute SELECT queries and retrieve data from database
   - Returns: Query results as a vector of strings

3. Statement Execution (`execute`)
   - Purpose: Execute INSERT, UPDATE, DELETE, and DDL statements
   - Returns: Number of rows affected by the operation


## Testing 

### Deployment
```
deploy -f <path to>/sap_hana.wasm -p <path to>/sap_hana.widl -c <path to>/config.yaml
```

#### `config.yaml`
```yaml
conn_str: <Your SAP HANNA ODBC connection string>
```

**Note:** Please ensure that the sap hana database instance is in a running state.
When it has been stale for a while, it gets stopped and thus connection will not be established.

### Prompt examples (to be adapted your database)

- Get the database schema for the sap hana database
- Show me all the entries in the Users table in the sap hana database
- Get all products from the sap hana database
- Count how many users we have
- Show me the top 3 most expensive Products in the sap hana database
