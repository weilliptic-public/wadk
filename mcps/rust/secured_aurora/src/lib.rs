//! # Aurora MCP Server (WeilChain Applet)
//!
//! This module implements a Model Context Protocol (MCP) server that
//! exposes read/write access to an Amazon Aurora (PostgreSQL-compatible)
//! database from a WeilChain smart contract. It provides three query-facing
//! entry points:
//!
//! - [`Aurora::run_query`] — run a SELECT (or other row-returning) SQL and
//!   return the rows as `Vec<String>`
//! - [`Aurora::execute`] — run a DDL/DML statement (INSERT/UPDATE/DELETE/DDL)
//!   and return the number of affected rows
//! - [`Aurora::run_query_and_export`] — run a row-returning query and export
//!   its JSON-serialized results to the on-chain IMFS, returning a file
//!   descriptor
//!
//! The contract reads credentials from [`Secrets<AuroraDBConfig>`] and builds a
//! Postgres connection URL per request. Inputs are sanitized via
//! [`cleanse_input_string`] to remove escape characters that could break SQL
//! parsing when transmitted across layers.
//!
//! ## Security note
//! This implementation forwards raw SQL strings. Prefer parameterized queries
//! whenever possible to mitigate injection risks. `cleanse_input_string` helps
//! normalize inputs but is **not** a substitute for full SQL parameterization.
//!
//! ## Tooling
//! The [`Aurora::tools`] method returns a JSON schema describing the available
//! tool functions for MCP-style function calling.

use serde::{Deserialize, Serialize};
use weil_macros::{WeilType, constructor, query, secured, smart_contract};
use weil_rs::{config::Secrets, db::DB, runtime::Runtime, utils::cleanse_input_string};

/// Configuration required to connect to an Aurora (PostgreSQL-compatible) database.
///
/// Values are retrieved at runtime from the contract's secret store.
#[derive(Debug, Serialize, Deserialize, WeilType, Default)]
pub struct AuroraDBConfig {
    /// Database username.
    username: String,
    /// Database password.
    password: String,
    /// Aurora endpoint in the form `host:port` (e.g., `my-cluster.cluster-xxxx.us-east-1.rds.amazonaws.com:5432`).
    endpoint: String,
}

/// Public MCP-facing API for the Aurora contract.
///
/// Implemented by [`AuroraContractState`] under the `#[smart_contract]` macro.
trait SecuredAurora {
    /// Construct a new contract state with an empty/loaded secret store.
    ///
    /// # Errors
    /// Returns `Err(String)` if initialization fails (e.g., secret store issues).
    fn new() -> Result<Self, String>
    where
        Self: Sized;

    // async fn schema(&self, db_name: String) -> String;

    /// Run a row-returning SQL query against the specified database.
    ///
    /// Inputs are cleansed with [`cleanse_input_string`].
    ///
    /// # Parameters
    /// - `query_str`: SQL string expected to return rows (e.g., `SELECT ...`).
    /// - `db_name`: Logical database name within the Aurora cluster.
    ///
    /// # Returns
    /// - `Ok(Vec<String>)` — Each element is a row serialized by the underlying `DB::query`.
    /// - `Err(String)` — On connection/query errors, or when zero rows are returned.
    ///
    /// # Errors
    /// - Connection/driver errors are stringified and returned.
    /// - Returns `"No rows found for this query"` when the result set is empty.
    async fn run_query(&self, query_str: String, db_name: String) -> Result<Vec<String>, String>;

    /// Execute a non-row-returning SQL statement (INSERT/UPDATE/DELETE/DDL).
    ///
    /// Inputs are cleansed with [`cleanse_input_string`].
    ///
    /// # Parameters
    /// - `db_name`: Logical database name within the Aurora cluster.
    /// - `statement`: SQL to execute.
    ///
    /// # Returns
    /// - `Ok(u64)` — Number of rows affected (as reported by the driver).
    /// - `Err(String)` — If execution fails.
    async fn execute(&self, db_name: String, statement: String) -> Result<u64, String>;

    /// Run a row-returning query and export the result to IMFS as JSON.
    ///
    /// This calls [`Self::run_query`], serializes the returned vector to JSON,
    /// then performs a cross-contract call to `imfs.write`, returning the
    /// resulting file descriptor.
    ///
    /// # Parameters
    /// - `query_str`: SQL expected to return rows.
    /// - `db_name`: Target database name.
    /// - `filename`: IMFS path to write the JSON content to.
    ///
    /// # Returns
    /// - `Ok(String)` — File descriptor string returned by `imfs.write`.
    /// - `Err(String)` — Propagated from querying or file write failures.
    async fn run_query_and_export(
        &self,
        query_str: String,
        db_name: String,
        filename: String,
    ) -> Result<String, String>;

    /// Return the MCP tools JSON schema describing callable functions and parameters.
    ///
    /// This is used by tool-calling LLMs/agents to discover available functions.
    fn tools(&self) -> String;
}

/// On-chain contract state and entry points for the Aurora MCP server.
#[derive(Serialize, Deserialize, WeilType)]
pub struct SecuredAuroraContractState {
    /// Contract-scoped secret storage containing [`AuroraDBConfig`].
    ///
    /// Use `self.secrets.config()` to access the current configuration.
    secrets: Secrets<AuroraDBConfig>,
}

#[smart_contract]
impl SecuredAurora for SecuredAuroraContractState {
    /// Create an empty state with a `Secrets<AuroraDBConfig>` handle.
    ///
    /// Secrets are expected to be managed outside of code (deployed/updated via platform tools).
    #[constructor]
    fn new() -> Result<Self, String>
    where
        Self: Sized,
    {
        Ok(Self {
            secrets: Secrets::<AuroraDBConfig>::new(),
        })
    }

    /// See [`Aurora::run_query`].
    #[query]
    #[secured("Engg::weil")]
    async fn run_query(&self, query_str: String, db_name: String) -> Result<Vec<String>, String> {
        let config = self.secrets.config();

        // Normalize potentially escaped inputs to avoid breaking SQL transport.
        let query_str = cleanse_input_string(&query_str);
        let db_name = cleanse_input_string(&db_name);

        // Build a postgres URL:
        // postgres://{username}:{password}@{endpoint}/{db_name}
        let mut url = format!(
            "postgres://{}:{}@{}/",
            config.username, config.password, config.endpoint
        );
        url += &db_name;

        let rows = DB::query(&url, query_str).map_err(|err| err.to_string())?;

        if rows.len() == 0 {
            return Err("No rows found for this query".to_string());
        }

        Ok(rows)
    }

    /// See [`Aurora::execute`].
    #[query]
    #[secured("Engg::weil")]
    async fn execute(&self, db_name: String, statement: String) -> Result<u64, String> {
        let config = self.secrets.config();

        // Clean inputs so transport/parsing isn't broken by escapes.
        let statement = cleanse_input_string(&statement);
        let db_name = cleanse_input_string(&db_name);

        // postgres://{username}:{password}@{endpoint}/{db_name}
        let mut url = format!(
            "postgres://{}:{}@{}/",
            config.username, config.password, config.endpoint
        );
        url += &db_name;

        let number_of_rows_affected =
            DB::execute(&url, statement).map_err(|err| err.to_string())?;

        Ok(number_of_rows_affected)
    }

    /// See [`Aurora::run_query_and_export`].
    #[query]
    #[secured("Engg::weil")]
    async fn run_query_and_export(
        &self,
        query_str: String,
        db_name: String,
        filename: String,
    ) -> Result<String, String> {
        let result = self.run_query(query_str, db_name).await;

        match result {
            Ok(result_vector) => {
                /// Arguments for the IMFS `write` cross-contract call.
                #[derive(Serialize, Deserialize)]
                struct Args {
                    /// Destination path within IMFS.
                    filepath: String,
                    /// File content (JSON string of the query result).
                    content: String,
                };

                let content = serde_json::to_string(&result_vector).unwrap();

                let args = Args {
                    filepath: filename.clone(),
                    content: content,
                };

                // Resolve the IMFS contract address by name and call `write`.
                let contract_addr = Runtime::contract_id_for_name("imfs");

                let file_descriptor = Runtime::call_contract::<String>(
                    contract_addr,                               // address of imfs
                    "write".to_string(),                         // function to call
                    Some(serde_json::to_string(&args).unwrap()), // serialized args
                )
                .map_err(|err| err.to_string())?;

                Ok(file_descriptor)
            }
            Err(error) => Err(error),
        }
    }

    /// Return MCP tool schema for `execute`, `run_query`, and `run_query_and_export`.
    ///
    /// This JSON is intended for LLM/agent function-calling discovery and validation.
    #[query]
    fn tools(&self) -> String {
        r#"[
  {
    "type": "function",
    "function": {
      "name": "execute",
      "description": "This executes the statement in aurora provided in argument `statement` potentially mutating the rows of the database with name given by argument `db_name`.\n",
      "parameters": {
        "type": "object",
        "properties": {
          "db_name": {
            "type": "string",
            "description": "the name of the database you want to run the query in\n"
          },
          "statement": {
            "type": "string",
            "description": "the statement you want to execute\n"
          }
        },
        "required": [
          "db_name",
          "statement"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "run_query",
      "description": "This runs a query provided in `query_str` in aurora on the database with name given by argument `db_name`.\n",
      "parameters": {
        "type": "object",
        "properties": {
          "query_str": {
            "type": "string",
            "description": "the query string that you want to run\n"
          },
          "db_name": {
            "type": "string",
            "description": "the name of the database you want to run the query in\n"
          }
        },
        "required": [
          "query_str",
          "db_name"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "run_query_and_export",
      "description": "exports the data returned by running a query provided in `query_str` in aurora on the database with name given by argument `db_name`, returns a file descriptor to the uploaded file\n",
      "parameters": {
        "type": "object",
        "properties": {
          "query_str": {
            "type": "string",
            "description": "the query string that you want to run\n"
          },
          "db_name": {
            "type": "string",
            "description": "the name of the database you want to run the query in\n"
          },
          "filename": {
            "type": "string",
            "description": "the filename to save to\n"
          }
        },
        "required": [
          "query_str",
          "db_name",
          "filename"
        ]
      }
    }
  }
]"#.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Smoke-test demonstrating input cleansing of escape characters.
    ///
    /// This does not hit a real database; it simply exercises `cleanse_input_string`.
    #[test]
    fn test_string() {
        let input = "SELECT Title FROM ALBUM LIMIT 10;\\";
        let output = cleanse_input_string(input);
        println!("{}", output);
    }
}
