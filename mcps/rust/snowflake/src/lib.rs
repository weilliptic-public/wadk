//! # Snowflake MCP Server (WeilChain Applet)
//!
//! This module implements a **Model Context Server (MCP)** contract that lets on-chain
//! agents run SQL against **Snowflake** via the Snowflake SQL API, optionally export
//! results to the on-chain IMFS, and enumerate stored procedures.
//!
//! ## Key entry points
//! - [`Snowflake::run_query`]: run a SELECT (or other row-returning) statement and
//!   return a rich [`RunQueryResult`] summarizing outcomes (success, no rows, timeout, bad SQL).
//! - [`Snowflake::run_query_and_export`]: run a query and write its JSON rows to IMFS, returning a file descriptor.
//! - [`Snowflake::execute`]: execute statements that may mutate data (e.g., DDL/DML) and return a concise summary.
//! - [`Snowflake::list_procedures`]: list stored procedures and metadata for a schema.
//! - [`Snowflake::tools`] / [`Snowflake::prompts`]: advertise tool schemas to function-calling agents.
//!
//! ## Flow & behavior
//! * The contract reads credentials (account identifier, PAT, role) from
//!   [`Secrets<SnowflakeConfig>`].
//! * Requests are sent to `https://{account_identifier}.snowflakecomputing.com/api/v2/statements`
//!   with a unique `requestId`.
//! * If Snowflake responds **202 Accepted**, the query is considered a model-unfriendly
//!   long-running query and is surfaced as
//!   [`RunQueryResult::CorrectSqlGeneratedButTimeout`].
//! * Successful 2xx responses are parsed into [`QueryResult`]; rows are mapped
//!   into JSON strings keyed by column name, with values normalized via
//!   [`parse_snowflake_value`].
//! * Responses spanning multiple partitions are returned with a message indicating
//!   that the result is too large for the chatbot to process fully.
//!
//! ## Notes & caveats
//! * Inputs are normalized via [`cleanse_sql`] to reduce transport/escape issues;
//!   this is **not** a substitute for parameterization or least-privilege roles.
//! * The trait returns `Result<RunQueryResult, RunQueryResult>` for `run_query`,
//!   making both success and error conditions strongly typed for tool-calling LLMs.
//!
//! ## Security
//! Use a role with least privileges, short-lived PATs where possible, and
//! prefer read-only warehouses for analytical paths. Avoid exposing raw secrets
//! in logs.

use serde::{Deserialize, Serialize};
use std::fmt;
use weil_macros::{WeilType, constructor, query, smart_contract};
use weil_rs::config::Secrets;
use weil_rs::{
    http::{HttpClient, HttpMethod},
    runtime::Runtime,
};

use crate::parser::{cleanse_sql, parse_snowflake_value};
use std::collections::{BTreeMap, HashMap};
mod parser;

/// Standardized prefix for user-visible data/parse errors bubbled up from
/// Snowflake or JSON parsing stages.
const INVALID_DATA_RECEIVED: &str = "invalid data received; ";

/// Configuration for connecting to a Snowflake account.
///
/// Fields are sourced from the contract's secret store. `account_identifier` is the
/// canonical Snowflake account identifier (e.g., `xy12345.us-east-1`), `pat_token` is
/// a **Programmatic Access Token**, and `role` is the Snowflake role used for all
/// API calls made by this contract.
#[derive(Debug, Serialize, Deserialize, WeilType, Clone, Default)]
pub struct SnowflakeConfig {
    /// Full account identifier, e.g., `"youraccount.yourregion"` or `"youraccount"`.
    pub account_identifier: String,
    /// Programmatic Access Token (PAT) used for API authentication.
    pub pat_token: String,
    /// Snowflake role name under which statements will execute.
    pub role: String,
}

/// JSON body sent to the Snowflake SQL API `/api/v2/statements`.
///
/// The API requires the statement text and execution context (database/schema/warehouse/role).
#[derive(Debug, Serialize)]
struct SqlApiRequestBody<'a> {
    /// SQL statement (raw string).
    statement: &'a str,
    /// Optional timeout; `None` delegates to service defaults.
    timeout: Option<u32>,
    /// Target database.
    database: &'a str,
    /// Optional schema within the database.
    #[serde(skip_serializing_if = "Option::is_none")]
    schema: Option<&'a str>,
    /// Target warehouse for execution.
    warehouse: &'a str,
    /// Role to use for the execution.
    role: &'a str,
}

/// Partition metadata for a segmented query response.
///
/// Large result sets may be split across multiple partitions.
#[derive(Debug, Deserialize, Clone)]
struct PartitionInfo {
    #[serde(rename = "rowCount")]
    row_count: u64,
}

/// Row metadata containing column definitions and partition info.
#[derive(Debug, Deserialize, Clone)]
struct RowInfo {
    /// Column type/name metadata.
    #[serde(rename = "rowType")]
    columns: Vec<ColumnInfo>,
    /// Partitioning details for the result set.
    #[serde(rename = "partitionInfo")]
    partition_info: Vec<PartitionInfo>,
}

/// Column metadata describing each field in the result set.
#[derive(Debug, Deserialize, Clone)]
struct ColumnInfo {
    /// Column name as returned by Snowflake.
    name: String,
    /// Snowflake logical type name for the column.
    #[serde(rename = "type")]
    row_type: String,
    /// Source table name (if available).
    table: String,
    // Additional metadata such as length/precision/scale may also be present.
}

/// Successful query wrapper containing serialized rows and a status message.
///
/// Each element in `data` is a JSON object string representing one row,
/// keyed by column names.
#[derive(Debug, Serialize, Deserialize)]
pub struct RunQueryResponse {
    /// Rows serialized as JSON strings (BTreeMap<column -> value>).
    data: Vec<String>,
    /// Human-readable status or guidance message.
    message: String,
}

/// Outcome envelope for query attempts.
///
/// Designed for tool-calling LLMs: both success and error states are explicit.
#[derive(Debug, Serialize, Deserialize)]
pub enum RunQueryResult {
    /// SQL was correct, executed, but yielded no rows.
    NoRowsRetuned { message: String },
    /// SQL was generated but Snowflake (or parsing) returned an error.
    WrongSqlGenerated { error: String },
    /// SQL executed successfully and returned rows with a message.
    SuccessfulSqlRun { response: RunQueryResponse },
    /// SQL was accepted but **timed out** (>45s) and returned HTTP 202.
    CorrectSqlGeneratedButTimeout { sql: String },
}

impl fmt::Display for RunQueryResult {
    /// Formats a human-readable summary of the [`RunQueryResult`] variant.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RunQueryResult::NoRowsRetuned { message } => {
                write!(f, "No rows returned: {}", message)
            }
            RunQueryResult::WrongSqlGenerated { error } => {
                write!(f, "Wrong SQL generated: {}", error)
            }
            RunQueryResult::SuccessfulSqlRun { response } => {
                write!(f, "Query executed successfully: {}", response.message)
            }
            RunQueryResult::CorrectSqlGeneratedButTimeout { sql } => {
                write!(f, "Query timed out: {}", sql)
            }
        }
    }
}

/// Deserialized representation of Snowflake's `api/v2/statements` response.
///
/// The `data` matrix is `Vec<row><col>` with optional string cell values.
/// Column metadata and partition info are available in `row_info`.
#[derive(Debug, Deserialize)]
pub struct QueryResult {
    /// 2-D array of cell values (as optional strings).
    data: Vec<Vec<Option<String>>>,
    /// Column and partition metadata.
    #[serde(rename = "resultSetMetaData")]
    row_info: RowInfo,
    /// Optional server message (not always present).
    message: Option<String>,
    /// Optional code, e.g., `"090001"` for success with results.
    code: Option<String>,
    /// Statement handle for async polling (if applicable).
    #[serde(rename = "statementHandle")]
    statement_id: Option<String>,
}

/// Convert an `Option<&String>` into a `Result<String, String>` using a common error prefix.
fn handle_option_str(val_opt: Option<&String>) -> Result<String, String> {
    let Some(val_str) = val_opt else {
        return Err(INVALID_DATA_RECEIVED.to_string());
    };
    Ok(val_str.to_string())
}

/// Build Snowflake API request headers, including PAT authentication and token type.
///
/// Adds a descriptive `User-Agent` for observability and best practices.
fn get_header(pat_token: String) -> HashMap<String, String> {
    let mut header = HashMap::new();
    header.insert("Authorization".to_owned(), format!("Bearer {}", pat_token));
    header.insert("Content-Type".to_owned(), "application/json".to_owned());
    header.insert("Accept".to_owned(), "application/json".to_owned());
    header.insert(
        "X-Snowflake-Authorization-Token-Type".to_owned(),
        "PROGRAMMATIC_ACCESS_TOKEN".to_owned(),
    );
    header.insert("User-Agent".to_owned(), "snowflake-mcp/1.0".to_owned()); // Good practice

    header
}

/// Convert a Snowflake `"YES"`/`"NO"` string into a boolean.
///
/// Returns an error if the value is neither `"YES"` nor `"NO"`.
fn str_to_bool(val: &Option<String>) -> Result<bool, String> {
    match val.as_ref().unwrap().as_str() {
        "YES" => Ok(true),
        "NO" => Ok(false),
        _ => Err(INVALID_DATA_RECEIVED.to_string()),
    }
}

/// Foreign key / primary key relationship details for a table.
///
/// Useful for documenting inter-table constraints discovered via information schema.
#[derive(Debug, Serialize, Deserialize)]
pub struct Constraint {
    pub fk_schema_name: String,
    pub fk_column_name: String,
    pub fk_table_name: String,
    pub pk_schema_name: String,
    pub pk_column_name: String,
    pub pk_table_name: String,
}

/// Effective privileges on a table for a given grantee.
#[derive(Debug, Serialize, Deserialize)]
pub struct TablePrivilege {
    #[serde(rename = "GRANTOR")]
    grantor: String,
    #[serde(rename = "GRANTEE")]
    grantee: String,
    #[serde(rename = "TABLE_SCHEMA")]
    table_schema: String,
    #[serde(rename = "TABLE_NAME")]
    table_name: String,
    #[serde(rename = "PRIVILEGE_TYPE")]
    privilege_type: String,
    #[serde(rename = "IS_GRANTABLE")]
    is_grantable: String,
    #[serde(rename = "WITH_HIERARCHY")]
    with_herirachy: String,
}

/// Public MCP interface implemented by [`SnowflakeContractState`].
trait Snowflake {
    /// Construct a new contract state with an attached `Secrets<SnowflakeConfig>`.
    fn new() -> Result<Self, String>
    where
        Self: Sized;

    /// Execute a row-returning SQL statement.
    ///
    /// Returns a typed [`RunQueryResult`] describing success/timeout/no-rows/errors.
    async fn run_query(
        &self,
        query_str: String,
        schema_name: String,
        warehouse: String,
        database: String,
    ) -> Result<RunQueryResult, RunQueryResult>;

    /// Execute a query and write its JSON rows to IMFS, returning a file descriptor.
    ///
    /// Delegates to [`Self::run_query`] and maps outcomes into a simple `Result`.
    // This executes the query and writes it to an in-memory-file-system(imfs).
    async fn run_query_and_export(
        &self,
        query_str: String,
        schema_name: String,
        warehouse: String,
        database: String,
        filename: String,
    ) -> Result<String, String>;

    /// Execute a (potentially mutating) statement and return a concise summary string.
    ///
    /// For mutating operations, non-zero stats keys (e.g., `rowsChanged`) are surfaced.
    /// For single-row outputs (e.g., stored procedure calls), the value is returned.
    async fn execute(
        &self,
        statement: String,
        schema_name: String,
        warehouse: String,
        database: String,
    ) -> Result<String, String>;

    /// List stored procedures and metadata for a schema.
    async fn list_procedures(
        &self,
        schema_name: String,
        warehouse: String,
        database: String,
    ) -> Result<Vec<ProcedureDetail>, String>;

    /// Return MCP tool JSON schema for function-calling discovery.
    fn tools(&self) -> String;

    /// Return prompt pack(s) for agent guidance (currently empty).
    fn prompts(&self) -> String;
}

/// Minimal metadata for a stored procedure.
#[derive(Debug, Serialize, Deserialize)]
pub struct ProcedureDetail {
    arguments: String,
    min_num_arguments: u64,
    max_num_arguments: u64,
    name: String,
    description: String,
}

/// On-chain Snowflake MCP contract state holding secret config.
#[derive(Serialize, Deserialize, WeilType)]
pub struct SnowflakeContractState {
    /// Contract-scoped secret storage containing [`SnowflakeConfig`].
    secrets: Secrets<SnowflakeConfig>,
}

impl SnowflakeContractState {
    /// Send a SQL statement to the Snowflake SQL API and return the raw JSON response text.
    ///
    /// * Builds the base URL using the configured account identifier.
    /// * Adds a unique `requestId` for idempotency/observability.
    /// * Returns an error if the API responds with 202 (long-running) or a non-2xx status.
    pub fn get_response(
        &self,
        statement: &str,
        schema_name: Option<&str>,
        warehouse: &str,
        database: &str,
    ) -> Result<String, String> {
        let base_url = format!(
            "https://{}.snowflakecomputing.com",
            self.secrets.config().account_identifier
        );

        let request_id = Runtime::uuid();
        let query_url = format!("{}/api/v2/statements?requestId={}", base_url, request_id);

        let request_body = SqlApiRequestBody {
            statement,
            timeout: None,
            database,
            schema: schema_name,
            warehouse,
            role: &self.secrets.config().role,
        };

        let response = HttpClient::request(&query_url, HttpMethod::Post)
            .headers(get_header(self.secrets.config().pat_token.clone()))
            .json(&request_body)
            .send()
            .map_err(|err| INVALID_DATA_RECEIVED.to_owned() + &err.to_string())?;

        // handling query result which contains handle for Asynchronous execution
        if response.status() == 202 {
            return Err(format!(
                "The Query Execution is taking more than 45 seconds. Therefore it can not be processed via the model. SQL statement (shown by the model) {}, in schema {} of database {}, warehouse {}",
                statement,
                schema_name.unwrap_or("not needed"),
                database,
                warehouse
            ));
        }

        if !(200..300).contains(&response.status()) {
            return Err(response.text());
        }

        Ok(response.text())
    }
}

#[smart_contract]
impl Snowflake for SnowflakeContractState {
    /// Initialize the contract state with a new `Secrets<SnowflakeConfig>` handle.
    #[constructor]
    fn new() -> Result<Self, String>
    where
        Self: Sized,
    {
        Ok(Self {
            secrets: Secrets::<SnowflakeConfig>::new(),
        })
    }

    /// Execute a row-returning SQL statement and map Snowflake responses into [`RunQueryResult`].
    ///
    /// * Cleans input with [`cleanse_sql`].
    /// * Distinguishes between **timeout** (202), **no rows**, **wrong SQL**, and **successful** outcomes.
    #[query]
    async fn run_query(
        &self,
        query_str: String,
        schema_name: String,
        warehouse: String,
        database: String,
    ) -> Result<RunQueryResult, RunQueryResult> {
        let query_str = cleanse_sql(&query_str);

        // Handle the case where get_response returns an error
        let response_text =
            match self.get_response(&query_str, Some(&schema_name), &warehouse, &database) {
                Ok(text) => text,
                Err(error) => {
                    // Check if this is a 202 timeout error
                    if error.contains("Query Execution is taking more than 45 seconds") {
                        return Ok(RunQueryResult::CorrectSqlGeneratedButTimeout {
                            sql: query_str,
                        });
                    }
                    // Other errors are treated as wrong SQL generated
                    return Err(RunQueryResult::WrongSqlGenerated { error });
                }
            };

        // Parse the response
        let query_response: QueryResult = match serde_json::from_str(&response_text) {
            Ok(response) => response,
            Err(err) => {
                return Err(RunQueryResult::WrongSqlGenerated {
                    error: INVALID_DATA_RECEIVED.to_owned() + &err.to_string(),
                });
            }
        };

        let row_info = query_response.row_info;
        let rows = query_response.data;
        /*
        NOTE: 0 rows is considered an error in this case
        */

        if rows.len() == 0 {
            return Err(RunQueryResult::NoRowsRetuned {
                message: "Query executed successfully, but no rows returned".to_string(),
            });
        }

        let mut result = Vec::new();

        for row in rows {
            assert_eq!(row.len(), row_info.columns.len());
            let mut row_map = BTreeMap::new();
            for i in 0..row.len() {
                let col_info = row_info.columns.get(i).unwrap();
                // parsing null values to "null" string
                let row_parsed_val =
                    parse_snowflake_value(row.get(i).unwrap().as_ref(), col_info.row_type.as_str());
                row_map.insert(col_info.name.clone(), row_parsed_val);
            }
            result.push(match serde_json::to_string(&row_map) {
                Ok(json_str) => json_str,
                Err(err) => {
                    return Err(RunQueryResult::WrongSqlGenerated {
                        error: INVALID_DATA_RECEIVED.to_owned() + &err.to_string(),
                    });
                }
            });
        }
        let num_partitions = row_info.partition_info.len() as u32;
        let mut total_row_count = 0;
        for partition in &row_info.partition_info {
            total_row_count += partition.row_count;
        }

        let response = if num_partitions == 1 {
            // If there's only one partition, we can return the data directly
            RunQueryResponse {
                data: result,
                message: "Query executed successfully, this is the complete response".to_string(),
            }
        } else {
            RunQueryResponse {
                data: result,
                message: format!(
                    "Message to the user: The Query response has a total of {total_row_count} rows, This response is too large for the chatbot to process. "
                ),
            }
        };

        Ok(RunQueryResult::SuccessfulSqlRun { response })
    }

    /// Run a query via [`Self::run_query`] and export the JSON rows to IMFS, returning the file descriptor.
    ///
    /// Errors are serialized variants of [`RunQueryResult`] to help upstream callers present clear feedback.
    #[query]
    async fn run_query_and_export(
        &self,
        query_str: String,
        schema_name: String,
        database: String,
        warehouse: String,
        filename: String,
    ) -> Result<String, String> {
        let query_str = cleanse_sql(&query_str);

        // Use the run_query function and handle its Result<RunQueryResult, RunQueryResult> return type
        let query_result = self
            .run_query(query_str.clone(), schema_name, warehouse, database)
            .await;

        let result_vec = match query_result {
            Ok(RunQueryResult::SuccessfulSqlRun { response }) => response.data,
            Ok(RunQueryResult::WrongSqlGenerated { error }) => {
                let err_enum = RunQueryResult::WrongSqlGenerated { error };
                return Err(serde_json::to_string(&err_enum).map_err(|err| err.to_string())?);
            }
            Ok(RunQueryResult::CorrectSqlGeneratedButTimeout { sql }) => {
                let err_enum = RunQueryResult::CorrectSqlGeneratedButTimeout { sql };
                return Err(serde_json::to_string(&err_enum).map_err(|err| err.to_string())?);
            }
            Ok(RunQueryResult::NoRowsRetuned { message }) => {
                let err_enum = RunQueryResult::NoRowsRetuned { message };
                return Err(serde_json::to_string(&err_enum).map_err(|err| err.to_string())?);
            }
            Err(run_query_result) => {
                return Err(
                    serde_json::to_string(&run_query_result).map_err(|err| err.to_string())?
                );
            }
        };

        #[derive(Serialize, Deserialize)]
        struct Args {
            /// Destination IMFS path.
            filepath: String,
            /// File content (JSON array of row objects).
            content: String,
        }

        let args = Args {
            filepath: filename,
            content: serde_json::to_string(&result_vec).unwrap(),
        };

        let contract_addr = Runtime::contract_id_for_name("imfs");

        let file_descriptor = Runtime::call_contract::<String>(
            contract_addr,
            "write".to_string(),
            Some(serde_json::to_string(&args).unwrap()),
        )
        .map_err(|err| err.to_string())?;

        Ok(file_descriptor)
    }

    /// Execute a (possibly mutating) statement and return a concise, human-readable summary.
    ///
    /// * Parses `stats` to surface only **non-zero** counters (e.g., rows changed).
    /// * If a single result row is present (e.g., from a stored procedure),
    ///   it is included in the message for clarity.
    #[query]
    async fn execute(
        &self,
        statement: String,
        schema_name: String,
        warehouse: String,
        database: String,
    ) -> Result<String, String> {
        let response = self.get_response(&statement, Some(&schema_name), &warehouse, &database)?;

        #[derive(Deserialize)]
        struct ExecuteResult {
            /// Server message (e.g., statement accepted).
            message: String,
            /// Optional returned data (e.g., stored procedure result).
            data: Option<Vec<Vec<Option<String>>>>,
            /// Optional execution stats (rows changed/deleted/etc.).
            stats: Option<BTreeMap<String, i32>>,
        }
        let execute_result: ExecuteResult = serde_json::from_str(&response)
            .map_err(|err| INVALID_DATA_RECEIVED.to_owned() + &err.to_string())?;

        // return only entries that are non-zero , out of rowsChanged, rowsDeleted, ...
        let mut filtered_result = BTreeMap::new();
        for entry in execute_result.stats.unwrap_or_default().iter() {
            if *entry.1 != 0 {
                filtered_result.insert(entry.0.to_owned(), entry.1.to_owned());
            }
        }

        let val = if filtered_result.len() != 0 {
            format!(
                "stats: {}",
                serde_json::to_string(&filtered_result)
                    .map_err(|err| INVALID_DATA_RECEIVED.to_owned() + &err.to_string())?
            )
        } else {
            // only show data when single entry, which will mean that this is an output to the
            // statement which could have been a stored procedure call or similar one word answer

            match execute_result.data {
                None => String::new(),
                Some(data) => {
                    if data.len() == 1 {
                        format!("{:?}", data.get(0).unwrap())
                        // safe to unwrap, since data.len() already checked
                    } else {
                        String::new()
                    }
                }
            }
        };

        let response = format!("message: {}; {}", execute_result.message, val);
        Ok(response)
    }

    /// List stored procedures for a schema by issuing `SHOW PROCEDURES` and parsing results.
    ///
    /// Returns a vector of [`ProcedureDetail`]. Errors from different `RunQueryResult`
    /// variants are mapped to `Err(String)` with relevant context.
    #[query]
    async fn list_procedures(
        &self,
        schema_name: String,
        warehouse: String,
        database: String,
    ) -> Result<Vec<ProcedureDetail>, String> {
        let statement = "SHOW PROCEDURES".to_string();
        let query_result = self
            .run_query(statement, schema_name, warehouse, database)
            .await;

        let query_response = match query_result {
            Ok(RunQueryResult::SuccessfulSqlRun { response }) => response,
            Ok(RunQueryResult::WrongSqlGenerated { error }) => return Err(error),
            Ok(RunQueryResult::CorrectSqlGeneratedButTimeout { sql }) => {
                return Err(format!("Query timed out: {}", sql));
            }
            Ok(RunQueryResult::NoRowsRetuned { message }) => {
                return Err(message);
            }
            Err(RunQueryResult::WrongSqlGenerated { error }) => return Err(error),
            Err(RunQueryResult::CorrectSqlGeneratedButTimeout { sql }) => {
                return Err(format!("Query timed out: {}", sql));
            }
            Err(RunQueryResult::NoRowsRetuned { message }) => {
                return Err(message);
            }
            Err(RunQueryResult::SuccessfulSqlRun { response }) => response,
        };

        let mut procedure_details = Vec::new();

        // NOTE: can be optimized in case WASM becomes multi-threaded.
        for item in query_response.data {
            let procedure_detail: ProcedureDetail = serde_json::from_str(&item)
                .map_err(|err| INVALID_DATA_RECEIVED.to_owned() + &err.to_string())?;

            procedure_details.push(procedure_detail);
        }

        return Ok(procedure_details);
    }

    /// Return the tool schema (JSON) advertising callable functions and their parameters.
    ///
    /// Intended for function-calling agents to validate requests before invocation.
    #[query]
    fn tools(&self) -> String {
        r#"[
  {
    "type": "function",
    "function": {
      "name": "run_query",
      "description": "This runs a query provided in argument `query_str` on the schema database, given by the `schema_name` parameter.\n",
      "parameters": {
        "type": "object",
        "properties": {
          "query_str": {
            "type": "string",
            "description": "raw sql statement query without any backslashes or any other escape characters.\n"
          },
          "schema_name": {
            "type": "string",
            "description": "the name of schema to run the query in\n"
          },
          "warehouse": {
            "type": "string",
            "description": "the name of warehouse to run the query in\n"
          },
          "database": {
            "type": "string",
            "description": "the name of database to run the query in\n"
          }
        },
        "required": [
          "query_str",
          "schema_name",
          "warehouse",
          "database"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "execute",
      "description": "This executes the statement provided in argument `statement` potentially mutating the rows of the schema database, given by the `schema_name` parameter.\n",
      "parameters": {
        "type": "object",
        "properties": {
          "statement": {
            "type": "string",
            "description": "raw sql statement query without any backslashes or any other escape characters.\n"
          },
          "schema_name": {
            "type": "string",
            "description": "the name of schema to run the query in\n"
          },
          "warehouse": {
            "type": "string",
            "description": "the name of warehouse to run the query in\n"
          },
          "database": {
            "type": "string",
            "description": "the name of database to run the query in\n"
          }
        },
        "required": [
          "statement",
          "schema_name",
          "warehouse",
          "database"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "list_procedures",
      "description": "This gets the list of procedures and their metadata in snowflake, for the given schema\n",
      "parameters": {
        "type": "object",
        "properties": {
          "schema_name": {
            "type": "string",
            "description": "the name of schema to run the query in\n"
          },
          "warehouse": {
            "type": "string",
            "description": "the name of warehouse to run the query in\n"
          },
          "database": {
            "type": "string",
            "description": "the name of database to run the query in\n"
          }
        },
        "required": [
          "schema_name",
          "warehouse",
          "database"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "run_query_and_export",
      "description": "exports the data returned by running a query provided in `query_str` in Snowflake on the schema database, given by the `schema_name` parameter., returns a file descriptor to the uploaded file\n",
      "parameters": {
        "type": "object",
        "properties": {
          "query_str": {
            "type": "string",
            "description": "raw sql statement query without any backslashes or any other escape characters.\n"
          },
          "schema_name": {
            "type": "string",
            "description": "the name of schema to run the query in\n"
          },
          "warehouse": {
            "type": "string",
            "description": "the name of warehouse to run the query in\n"
          },
          "database": {
            "type": "string",
            "description": "the name of database to run the query in\n"
          },
          "filename": {
            "type": "string",
            "description": "the name of imfs file to write into\n"
          }
        },
        "required": [
          "query_str",
          "schema_name",
          "warehouse",
          "database",
          "filename"
        ]
      }
    }
  }
]"#.to_string()
    }

    /// Return an (extensible) prompt pack JSON for agent guidance.
    ///
    /// Currently returns an empty array; populate as needed to steer LLM behavior.
    #[query]
    fn prompts(&self) -> String {
        r#"{
  "prompts": []
}"#
        .to_string()
    }
}
