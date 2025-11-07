# Aurora MCP 
The Aurora MCP provides a secure interface to interact with Amazon Aurora PostgreSQL databases through natural language queries.

Amazon Aurora is a fully managed relational database service provided by AWS (Amazon Web Services).
It is part of the Amazon RDS (Relational Database Service) family, but it is designed to deliver
higher-performance and be more fault-tolerant and scalable than traditional relational databases.
Aurora is MySQL- and PostgreSQL-compatible, meaning applications written for those databases can run on Aurora with little or no modification.

## Core Tools
1. Database Query Execution (`run_query`)
2. Database Statement Execution (`execute`)

## Testing
In our testing, we populate Aurora with the [`chinook` database](./aurora_schema_hash.txt).
Using the CLI, the databse is deployed with the following command.

```
Weilliptic$$$> deploy --widl-file <path to>/aurora.widl --file-path <path to>/aurora.wasm --config-file <path to>/config.yaml -x <path to>/aurora_schema.txt -w <Pod Id to deploy> -l
```

## Prompt examples
Once deployed, you can use natural language to query the database. Below are some suggested prompts to get you started.

Level 1 Prompts (Basic single-table queries):
1. Retrieve top 20 album titles from the Album table.
2. List the first and last names of all customers.
3. Show all employees and their job titles.
4. Retrieve all tracks with their names and unit prices.
5. Display all genres available in the Genre table.

Level 2 Prompts (Joins between two tables):
1. Retrieve all albums along with the artist name who created them.
2. List customer names with their assigned support representative (employee).
3. Show invoice details along with the customer’s full name.
4. List tracks with their genre name and media type.
5. Display playlists along with the track names they contain.

Level 3 Prompts (Multi-table joins and aggregations):
1. List the top 10 customers by total invoice amount spent.
2. Find the most popular genre based on the number of tracks sold (using InvoiceLine).
3. Show total sales (sum of invoice totals) by country.
4. Retrieve all employees along with the customers they support and the total amount each customer has spent.
5. Display the top 5 albums by revenue, joining Album, Track, InvoiceLine, and Invoice.
