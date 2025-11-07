# BigQuery MCP Server
The BigQuery MCP provides a secure interface to interact with Google BigQuery for dataset management and SQL query execution.

## Core Tools
1. Create Dataset (`create_dataset`)
2. Get Dataset (`get_dataset`)
3. Update Dataset (`update_dataset`)
4. Delete Dataset (`delete_dataset`)
5. List Tables (`list_tables`)
6. Execute Query (`execute_query`)


## Testing
```
deploy -f /root/code/weilliptic/mcp_vault/rust/bigquery/target/wasm32-unknown-unknown/release/bigquery.wasm -p /root/code/weilliptic/mcp_vault/rust/bigquery/bigquery.widl -c /root/code/weilliptic/platform-scripts/contracts/configs/bigquery.yaml
```



### `config.yaml`
```yaml
project_id: 
access_token: 
refresh_token: 
client_id: 
client_secret: 
```



### Prompt examples
The following examples assume that a dataset named `test_dataset`, with a `Meetings` table,  with `title` and `date` fields, and `users` table, with `id`, `name`, and `score`, have been created.

- Can u show me all entries in the Users table of my dataset named test_dataset in bigquery
- Show me all tables in test_dataset
- Can u show me the Name in the Users table of my test_dataset who has the maximum score?
- Show entries in Users table of test_dataset with score greater than 80
- Get top 5 users by score
- Show average value of score in the Users table of test_dataset in my bigquery
- Add a new user named Alissa with score 95
- Update the score of Alissa in test_dataset's Users table to 90

