# Databricks MCP server


## Core Tools

1. run_query
2. execute
3. list_procedures

## Testing

In the example prompts below, we assume you have deployed some of the example databaset provided by DataBricks, for example
the [COVID-19 Datasets](https://www.databricks.com/blog/2020/04/14/covid-19-datasets-now-available-on-databricks.html), in your environment.

Also, to inform the MCP server of the datasets being used, you should pass in the [context file](context.txt) in the following command.

```
deploy -e -l --widl-file <path to>/databricks.widl --file-path <path to>/databricks.wasm --config-file <path to>/config.yaml --context-file <path to>/context.txt 
```


### `config.yaml`
```yaml
pat_token: 
workspace_url: <e.g., https://dbc-fcf8b0b8-596e.cloud.databricks.com/>
```

### Example prompts
- list all users
- list all clusters
- List all files in databricks
- create a directory : /Users/somya@weilliptic.com/mydir
- show all workspace directories in the path /Users/somya@weilliptic.com
- list metastores in databricks
- Which counties had the highest death rate in Florida?
- get me the top 10 counties in California with the highest COVID-19 cases
- Which states had the lowest number of deaths?
- show me first 5 trip details
- list all SQL warehouses
- create an alert named "alert2" on the sql query with id "9b0f80c1-a104-42fd-8e05-7df28b51ac1b" when the count field is greater than 100
- list catalogs in databricks
- list all schemas in mycatalog
- get all jobs
- run the job with id 443808742059485
- show me all serving endpoints
- show me details of the serving endpoint "databricks-gemma-3-12b"
- get the function body in mycatalog's default schema named "get_total_rows_in_students_table"
- create a function with name as "count_rows_in_cats_table" in the catalog mycatalog and schema default and body as corresponding sql for this
- create a pipeline called "print data" in "mycatalog" catalog and "default" schema and notebook "/Workspace/Users/somya@weilliptic.com/print_1_to_10"
- list all pipelines
- execute the pipeline with id f9f701a7-22b5-4104-9508-73a88809055f
- get events of the pipeline with id f9f701a7-22b5-4104-9508-73a88809055f
