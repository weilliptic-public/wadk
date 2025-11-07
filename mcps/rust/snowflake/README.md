# Snowflake MCP server


## Core Tools

1. run_query
2. execute
3. list_procedures

## Testing

In the example prompts below, we assume you have deployed the [TPC dataset](https://docs.snowflake.com/en/user-guide/sample-data-tpcds) in your environment.
Also, to inform the MCP server of the dataset being used, you should pass in the context file [tpcds_sf10tcl.txt](./tpcds_sf10tcl.txt) in the following command.

```
deploy -e -l --widl-file <path to>/snowflake.widl --file-path <path to>/snowflake.wasm --config-file <path to>/config.yaml --context-file <path to>/tpcds_sf10tcl.txt 
```


### `config.yaml`
```yaml
account_identifier : identifier used in API usage of Snowflake.
pat_token : Programmatic Access Token for Snowflake access
role : The role at which the statements are executed.
```

### Example prompts

1. List all call centers with name, city, state, and GMT offset. Sort by state then city.
2. Show call centers with more than 200 employees. Include center id, name, employees, and hours.
3. Return catalog pages with department, catalog number, and page number for pages whose description contains sale.
4. Find the top 20 most expensive items by current price. Show item_id, product_name, current_price, brand.
5. List items in category Women and class Dresses, with brand and wholesale cost.
6. Show customers whose preferred customer flag is Y, including name, email, birth year.
7. Count customers by state using CA_STATE.
1. Given a date like 2001-06-15, return its D_DATE_SK, day name, and whether it was a holiday/weekend.
1. List warehouses with name, square feet, city, state, and GMT offset.
1. List active promotions that use email and catalog channels; include promo_id, name, start and end date.
1. Show all return reasons sorted by description.
1. List all ship modes with type, code, and carrier.
1. Total net sales by store for D_DATE = '2001-12-24'. Join STORE_SALES → DATE_DIM → STORE.
2. Net sales by ship mode for Q1-2002. Join WEB_SALES → DATE_DIM → SHIP_MODE.
3. Compare billed vs shipped customers for a given month.
4. Average sales price by item price band for store channel last quarter of 2001.
5. Top 10 return reasons by amount for 2001 in stores.
6. Return quantity by reason for Cyber-Monday week of 2001.
7. Items with low on-hand (≤5) in any warehouse on a given day.
8. Sales by catalog page for a month.
1. Catalog order count by call center for 2001.
1. Compute promo vs non-promo net sales and % lift for second quarter 2002.
2. Build monthly cohorts by a customer’s first store purchase month.
3. Count customers who purchased in both store and web in 2002.
4. Top 10 items most frequently appearing with item class Electronics.
5. Net sales by hour of day for the week of 2002-11-25.
6. Net sales by store state for 2001-H2.
7. Total quantity shipped via each warehouse for 2001 (web + catalog).
8. Average list vs sales price by item category, comparing Catalog vs Web channels in 2002.
9. Return quantity as % of sold quantity by item for 2002 (store channel).
