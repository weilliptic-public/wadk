//! # WASM DB bridge (generic + SAP HANA)
//!
//! This module provides thin, typed wrappers around host-provided database
//! functions exposed via `wasm_import_module="env"`. It supports two targets:
//!
//! - **Generic DB** via URL-based FFI: [`DB::schema`], [`DB::query`], [`DB::execute`]
//! - **SAP HANA** via structured params: [`HanaDB::schema`], [`HanaDB::query`], [`HanaDB::execute`]
//!
//! Data crosses the WASM boundary as **length-prefixed UTF-8** byte buffers. Helpers
//! [`get_length_prefixed_bytes_from_string`] and [`read_bytes_from_memory`] are used to
//! safely serialize inputs and read host responses.
//!
//! ## FFI/Memory Safety
//! Each host function returns an `i32` pointer to a length-prefixed buffer that must be
//! read exactly once via [`read_bytes_from_memory`]. All `unsafe` blocks are annotated
//! with `// SAFETY:` comments describing the assumptions.

use crate::runtime::{get_length_prefixed_bytes_from_string, read_bytes_from_memory};
use serde::{Deserialize, Serialize};

#[link(wasm_import_module = "env")]
extern "C" {
    /// Host: return schema description for a database reachable at `url`.
    ///
    /// `url` is a pointer to a length-prefixed UTF-8 string.
    /// Returns a pointer to a length-prefixed UTF-8 JSON string containing the schema.
    ///
    /// # Safety
    /// - `url` must reference a valid, host-readable, length-prefixed buffer.
    /// - The returned pointer must be consumed via [`read_bytes_from_memory`].
    fn schema(url: i32) -> i32;

    /// Host: execute a read/query against the database at `url` using `query`.
    ///
    /// `url` and `query` are pointers to length-prefixed UTF-8 strings.
    /// Returns a pointer to a length-prefixed UTF-8 JSON `Vec<String>` (rows as JSON).
    ///
    /// # Safety
    /// - Both pointers must reference valid, length-prefixed buffers.
    /// - The returned pointer must be consumed via [`read_bytes_from_memory`].
    fn query(url: i32, query: i32) -> i32;

    /// Host: execute a write/DDL statement against the database at `url`.
    ///
    /// `url` and `statement` are pointers to length-prefixed UTF-8 strings.
    /// Returns a pointer to a length-prefixed UTF-8 JSON `u64` (rows affected).
    ///
    /// # Safety
    /// - Both pointers must reference valid, length-prefixed buffers.
    /// - The returned pointer must be consumed via [`read_bytes_from_memory`].
    fn execute(url: i32, statement: i32) -> i32;

    /// Host: return schema description for a SAP HANA connection string.
    ///
    /// `conn_str` is a pointer to a length-prefixed UTF-8 string.
    /// Returns a pointer to a length-prefixed UTF-8 JSON string containing the schema.
    ///
    /// # Safety
    /// - `conn_str` must reference a valid, length-prefixed buffer.
    /// - The returned pointer must be consumed via [`read_bytes_from_memory`].
    fn hana_schema(conn_str: i32) -> i32;

    /// Host: execute a SAP HANA query with serialized parameters.
    ///
    /// `params` is a pointer to a length-prefixed UTF-8 JSON object:
    /// `{ "connection_string": String, "query": String }`
    /// Returns a pointer to a length-prefixed UTF-8 JSON `Vec<String>` (rows as JSON).
    ///
    /// # Safety
    /// - `params` must reference a valid, length-prefixed buffer.
    /// - The returned pointer must be consumed via [`read_bytes_from_memory`].
    fn hana_query(params: i32) -> i32;

    /// Host: execute a SAP HANA statement with serialized parameters.
    ///
    /// `params` is a pointer to a length-prefixed UTF-8 JSON object:
    /// `{ "connection_string": String, "statement": String }`
    /// Returns a pointer to a length-prefixed UTF-8 JSON `u64` (rows affected).
    ///
    /// # Safety
    /// - `params` must reference a valid, length-prefixed buffer.
    /// - The returned pointer must be consumed via [`read_bytes_from_memory`].
    fn hana_execute(params: i32) -> i32;
}

/// Generic database façade using URL-based FFI endpoints.
///
/// This type has no fields and acts purely as a namespace for static helpers.
pub struct DB;

impl DB {
    /// Retrieve the schema for a database reachable at `url`.
    ///
    /// # Arguments
    /// * `url` — connection URL understood by the host (driver-specific)
    ///
    /// # Returns
    /// A JSON string describing the schema (format defined by the host).
    ///
    /// # Errors
    /// Propagates failures to read host memory or deserialize response text.
    pub fn schema(url: &str) -> Result<String, anyhow::Error> {
        let raw_url = get_length_prefixed_bytes_from_string(&url, 0);
        // SAFETY: `raw_url` is a valid length-prefixed buffer; host returns a pointer to a
        // length-prefixed UTF-8 JSON string which we immediately read.
        let ptr = unsafe { schema(raw_url.as_ptr() as _) };
        let schema = read_bytes_from_memory(ptr)?;

        Ok(schema)
    }

    /// Execute a read query and return rows as JSON strings.
    ///
    /// Each `String` in the result typically represents a row serialized as JSON (host-defined).
    ///
    /// # Arguments
    /// * `url` — connection URL
    /// * `query_str` — SQL text
    ///
    /// # Returns
    /// `Vec<String>` — rows encoded as JSON strings
    ///
    /// # Errors
    /// Propagates host memory read errors or JSON parse errors.
    pub fn query(url: &str, query_str: String) -> Result<Vec<String>, anyhow::Error> {
        let raw_url = get_length_prefixed_bytes_from_string(url, 0);
        let raw_query = get_length_prefixed_bytes_from_string(&query_str, 0);

        // SAFETY: Both buffers are valid length-prefixed sequences; host returns pointer
        // to a JSON Vec<String> which we read and parse.
        let ptr = unsafe { query(raw_url.as_ptr() as _, raw_query.as_ptr() as _) };

        let result = read_bytes_from_memory(ptr)?;
        let parsed_result = serde_json::from_str::<Vec<String>>(&result).unwrap();

        Ok(parsed_result)
    }

    /// Execute a write/DDL statement and return the affected row count.
    ///
    /// # Arguments
    /// * `url` — connection URL
    /// * `statement` — SQL statement (INSERT/UPDATE/DELETE/DDL)
    ///
    /// # Returns
    /// `u64` — number of affected rows (host-reported)
    ///
    /// # Errors
    /// Propagates host memory read errors or JSON parse errors.
    pub fn execute(url: &str, statement: String) -> Result<u64, anyhow::Error> {
        let raw_url = get_length_prefixed_bytes_from_string(url, 0);
        let raw_query = get_length_prefixed_bytes_from_string(&statement, 0);

        // SAFETY: Both buffers are valid; host returns pointer to a JSON u64 which we read.
        let ptr = unsafe { execute(raw_url.as_ptr() as _, raw_query.as_ptr() as _) };

        let result = read_bytes_from_memory(ptr)?;
        let parsed_result = serde_json::from_str::<u64>(&result).unwrap();

        Ok(parsed_result)
    }
}

/// SAP HANA-specific façade using structured JSON parameters.
///
/// This type has no fields and acts purely as a namespace for HANA helpers.
pub struct HanaDB;

/// Parameters for a HANA **query** request passed over FFI.
///
/// Serialized as JSON for the host.
#[derive(Debug, Serialize, Deserialize)]
pub struct HanaQueryParams {
    /// Full SAP HANA connection string.
    pub connection_string: String,
    /// SQL query text to execute.
    pub query: String,
}

/// Parameters for a HANA **execute** request passed over FFI.
///
/// Serialized as JSON for the host.
#[derive(Debug, Serialize, Deserialize)]
pub struct HanaExecuteParams {
    /// Full SAP HANA connection string.
    pub connection_string: String,
    /// SQL statement to execute (e.g., INSERT/UPDATE/DELETE/DDL).
    pub statement: String,
}

impl HanaDB {
    /// Retrieves schema information from a SAP HANA database
    ///
    /// # Arguments
    ///
    /// * `conn_str` - A connection string to the SAP HANA database
    ///
    /// # Returns
    ///
    /// A formatted string containing the schema information
    pub fn schema(conn_str: &str) -> Result<String, anyhow::Error> {
        // hana_schema keeps the original interface for backward compatibility
        let raw_conn_str = get_length_prefixed_bytes_from_string(conn_str, 0);
        // SAFETY: `raw_conn_str` is a valid length-prefixed buffer; host returns a pointer
        // to a length-prefixed UTF-8 JSON string with schema content.
        let ptr = unsafe { hana_schema(raw_conn_str.as_ptr() as _) };
        let schema = read_bytes_from_memory(ptr)?;

        Ok(schema)
    }

    /// Executes a query on a SAP HANA database and returns the results
    ///
    /// # Arguments
    ///
    /// * `conn_str` - A connection string to the SAP HANA database
    /// * `query_str` - The SQL query to execute
    ///
    /// # Returns
    ///
    /// A vector of strings, each representing a row as a JSON object
    pub fn query(conn_str: &str, query_str: String) -> Result<Vec<String>, anyhow::Error> {
        let params = HanaQueryParams {
            connection_string: conn_str.to_string(),
            query: query_str,
        };

        let params_json = serde_json::to_string(&params)?;
        let raw_params = get_length_prefixed_bytes_from_string(&params_json, 0);

        // SAFETY: `raw_params` is a valid length-prefixed JSON buffer matching `HanaQueryParams`.
        // Host returns a pointer to a length-prefixed UTF-8 JSON Vec<String>.
        let ptr = unsafe { hana_query(raw_params.as_ptr() as _) };

        let result = read_bytes_from_memory(ptr)?;
        let parsed_result = serde_json::from_str::<Vec<String>>(&result)?;

        Ok(parsed_result)
    }

    /// Executes a SQL statement on a SAP HANA database
    ///
    /// # Arguments
    ///
    /// * `conn_str` - A connection string to the SAP HANA database
    /// * `statement` - The SQL statement to execute
    ///
    /// # Returns
    ///
    /// The number of rows affected by the statement
    pub fn execute(conn_str: &str, statement: String) -> Result<u64, anyhow::Error> {
        let params = HanaExecuteParams {
            connection_string: conn_str.to_string(),
            statement: statement,
        };

        let params_json = serde_json::to_string(&params)?;
        let raw_params = get_length_prefixed_bytes_from_string(&params_json, 0);

        // SAFETY: `raw_params` is a valid length-prefixed JSON buffer matching `HanaExecuteParams`.
        // Host returns a pointer to a length-prefixed UTF-8 JSON u64 (rows affected).
        let ptr = unsafe { hana_execute(raw_params.as_ptr() as _) };

        let result = read_bytes_from_memory(ptr)?;
        let parsed_result = serde_json::from_str::<u64>(&result)?;

        Ok(parsed_result)
    }

    /// Example of how to create a connection string for SAP HANA
    ///
    /// # Arguments
    ///
    /// * `server` - The server address
    /// * `port` - The port number
    /// * `username` - The username
    /// * `password` - The password
    ///
    /// # Returns
    ///
    /// A formatted connection string
    pub fn create_connection_string(
        server: &str,
        port: u16,
        username: &str,
        password: &str,
    ) -> String {
        format!(
            "Driver=HDBODBC;ServerNode={}:{};UID={};PWD={};Encrypt=True",
            server, port, username, password
        )
    }
}
