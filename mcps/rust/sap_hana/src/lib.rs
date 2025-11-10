//! # SAP HANA MCP Server
//!
//! A Model Context Protocol (MCP) smart contract that integrates with an
//! **SAP HANA** backend via `weil_rs::db::HanaDB` to introspect schema,
//! run ad-hoc `SELECT` queries, and execute mutating statements.
//!
//! ## Overview
//! - **Config**: Connection string provided via `Secrets<HanaConfig>`.
//! - **Transport/Driver**: Uses `weil_rs::db::HanaDB` (aliased as `HanaSDK`).
//! - **MCP Surface**: `schema`, `run_query`, `execute` exposed via `tools()`;
//!   `prompts()` reserved for future prompt templates.
//! - **I/O Model**: All public ops are `#[query]` (non-mutating to contract state).
//!
//! ## Supported Operations
//! - `schema()` — Return a textual schema description from the HANA instance.
//! - `run_query(query_str)` — Execute a read query (e.g., `SELECT ...`) and
//!   return rows as `Vec<String>` (driver-formatted).
//! - `execute(statement)` — Run a DML/DDL statement and return affected row count.
//!
//! ## Notes
//! - This update adds **documentation only**; there are **no functional changes**.
//! - Ensure `HanaConfig.conn_str` is provisioned via `Secrets<HanaConfig>` before use.

use serde::{Deserialize, Serialize};
use weil_macros::{WeilType, constructor, query, smart_contract};
use weil_rs::config::Secrets;
use weil_rs::db::HanaDB as HanaSDK;

/// Connection settings for the SAP HANA backend.
///
/// The `conn_str` should contain all information required by the underlying
/// `HanaSDK` to establish a connection (host, port, credentials, DB name, etc.).
#[derive(Debug, Serialize, Deserialize, WeilType, Default)]
pub struct HanaConfig {
    /// SAP HANA connection string (driver-specific format).
    conn_str: String,
}

/// Public MCP trait surface for interacting with SAP HANA.
///
/// All methods are asynchronous and return `Result<…, String>` with
/// error messages propagated from the underlying driver (`HanaSDK`).
trait HanaDB {
    /// Construct a new contract state with an empty `Secrets<HanaConfig>` container.
    fn new() -> Result<Self, String>
    where
        Self: Sized;

    /// Retrieve a serialized description of the database schema from HANA.
    ///
    /// The exact format is determined by `HanaSDK::schema`.
    async fn schema(&self) -> Result<String, String>;

    /// Run a read-only SQL query (e.g., `SELECT …`) against HANA.
    ///
    /// * `query_str` — The SQL text to execute.
    /// * Returns a vector of driver-formatted row strings.
    async fn run_query(&self, query_str: String) -> Result<Vec<String>, String>;

    /// Execute a mutating SQL statement (DDL/DML) against HANA.
    ///
    /// * `statement` — The SQL text to execute.
    /// * Returns the count of affected rows (as reported by the driver).
    async fn execute(&self, statement: String) -> Result<u64, String>;

    /// JSON description of exposed MCP tools (for agent orchestration).
    fn tools(&self) -> String;

    /// Placeholder for prompt templates used by agentic flows.
    fn prompts(&self) -> String;
}

/// Contract state holding HANA connection secrets.
///
/// `secrets` must contain a valid `HanaConfig` at runtime for all operations.
#[derive(Serialize, Deserialize, WeilType)]
pub struct HanaDBContractState {
    // define your contract state here!
    /// Secure wrapper around the HANA connection configuration.
    secrets: Secrets<HanaConfig>,
}

#[smart_contract]
impl HanaDB for HanaDBContractState {
    /// Initialize an empty contract state with a new `Secrets<HanaConfig>` container.
    #[constructor]
    fn new() -> Result<Self, String>
    where
        Self: Sized,
    {
        Ok(HanaDBContractState {
            secrets: Secrets::<HanaConfig>::new(),
        })
    }

    /// Return a textual schema snapshot from the configured HANA instance.
    ///
    /// Delegates to `HanaSDK::schema(conn_str)`.
    #[query]
    async fn schema(&self) -> Result<String, String> {
        let credentials = self.secrets.config();

        let schema = HanaSDK::schema(&credentials.conn_str).map_err(|err| err.to_string())?;
        Ok(schema)
    }

    /// Execute a read-only SQL query and return driver-formatted rows.
    ///
    /// * `query_str` — Full SQL statement (e.g., `SELECT …`).
    /// Delegates to `HanaSDK::query(conn_str, query_str)`.
    #[query]
    async fn run_query(&self, query_str: String) -> Result<Vec<String>, String> {
        let credentials = self.secrets.config();

        let rows =
            HanaSDK::query(&credentials.conn_str, query_str).map_err(|err| err.to_string())?;
        Ok(rows)
    }

    /// Execute a mutating SQL statement (DDL/DML) and return affected row count.
    ///
    /// * `statement` — e.g., `INSERT …`, `UPDATE …`, `DELETE …`, `CREATE TABLE …`
    /// Delegates to `HanaSDK::execute(conn_str, statement)`.
    #[query]
    async fn execute(&self, statement: String) -> Result<u64, String> {
        let credentials = self.secrets.config();

        let number_of_rows_affected =
            HanaSDK::execute(&credentials.conn_str, statement).map_err(|err| err.to_string())?;
        Ok(number_of_rows_affected)
    }

    /// Machine-readable MCP tool specifications for `schema`, `run_query`, and `execute`.
    #[query]
    fn tools(&self) -> String {
        r#"[
  {
    "type": "function",
    "function": {
      "name": "schema",
      "description": "This returns the schema of the SAP HANA database\n",
      "parameters": {
        "type": "object",
        "properties": {},
        "required": []
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "run_query",
      "description": "This runs a query provided in argument `query_str` on the SAP HANA database.\n",
      "parameters": {
        "type": "object",
        "properties": {
          "query_str": {
            "type": "string",
            "description": ""
          }
        },
        "required": [
          "query_str"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "execute",
      "description": "This executes the statement provided in argument `statement` potentially mutating the rows of the SAP HANA database\n",
      "parameters": {
        "type": "object",
        "properties": {
          "statement": {
            "type": "string",
            "description": ""
          }
        },
        "required": [
          "statement"
        ]
      }
    }
  }
]"#.to_string()
    }

    /// Placeholder for prompt templates. Currently returns an empty `prompts` set.
    #[query]
    fn prompts(&self) -> String {
        r#"{
  "prompts": []
}"#
        .to_string()
    }
}
