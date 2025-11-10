
# Datadog

Datadog is a metrics database in which metrics metrics may be pushed using its `/api/v2/series` API.
All the datadog metrics are timeseries data expressed as sequence of two dimensional data points with `x` coordinate being the timestamp.

The Datadog MCP server exposes listing and querying metrics through natural language.
Datadog MCP server uses [`plottable`](https://github.com/weilliptic-inc/platform/wiki/Flow-for-Plottable) datatype to return data in a form which is automatically plotted on the Icarus chatbot, for example:

!(https://private-user-images.githubusercontent.com/174828730/494419435-b01b77da-f7b5-458a-be5a-1a5ae09fd7f3.png?jwt=eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJpc3MiOiJnaXRodWIuY29tIiwiYXVkIjoicmF3LmdpdGh1YnVzZXJjb250ZW50LmNvbSIsImtleSI6ImtleTUiLCJleHAiOjE3NjI3ODYxMjEsIm5iZiI6MTc2Mjc4NTgyMSwicGF0aCI6Ii8xNzQ4Mjg3MzAvNDk0NDE5NDM1LWIwMWI3N2RhLWY3YjUtNDU4YS1iZTVhLTFhNWFlMDlmZDdmMy5wbmc_WC1BbXotQWxnb3JpdGhtPUFXUzQtSE1BQy1TSEEyNTYmWC1BbXotQ3JlZGVudGlhbD1BS0lBVkNPRFlMU0E1M1BRSzRaQSUyRjIwMjUxMTEwJTJGdXMtZWFzdC0xJTJGczMlMkZhd3M0X3JlcXVlc3QmWC1BbXotRGF0ZT0yMDI1MTExMFQxNDQzNDFaJlgtQW16LUV4cGlyZXM9MzAwJlgtQW16LVNpZ25hdHVyZT02ZjgxYzk2M2U5OTFmNTViY2U0MDc4YWZhZGMyMWVkMTMzYjg0Zjc0MWI2ODdmM2NkOTE5MTc5YmFlNmQwNWQ3JlgtQW16LVNpZ25lZEhlYWRlcnM9aG9zdCJ9.Kt9UUXK1MuibudfmWfehj0_vhXUScHt8vsEDG0lIH80)


## Core Tools

The MCP provides tools to List and Query metrics.
Detailed documentation of the MCP interface is provided in the accompanying `.widl` file.

## Testing 

### Deployment
```
deploy -f <path to>/datadog.wasm -p <path to>/datadog.widl -c <path to>/config.yaml
```


#### `config.yaml`
```yaml
site: "ap1.datadoghq.com"
api_key: "<YOUR API KEY>"
app_key: "<YOUR APP KEY"
```

### Prompt examples (must be adapted to the metrics you uploaded to Datadog)

- list all the datadog metrics
- average load of system 1 in last 5 months
