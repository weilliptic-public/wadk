//! # JSON result utilities
//!
//! Helpers for converting a `Result<String, WeilError>`—typically a JSON-encoded
//! payload returned by a lower-level API—into a strongly typed value using Serde.
//!
//! The main entrypoint is [`try_into_result`], which:
//! 1. Maps a `Result<String, WeilError>` into `Result<String, String>` by stringifying the error,
//! 2. Deserializes the JSON string into a caller-provided type `T: DeserializeOwned`,
//! 3. Returns `Result<T, String>` with human-readable error messages on failure.
//!
//! ## When to use
//! - You have a function that returns `Result<String, WeilError>` where the `Ok`
//!   side is JSON text,
//! - You want a typed value and prefer to propagate errors as `String` (e.g. for
//!   WIDL/FFI boundaries or lightweight error surfaces).
//!
//! ## Example
//! ```rust
//! use serde::Deserialize;
//! use wadk_utils::errors::WeilError;
//! // use your_crate::utils::try_into_result;
//!
//! #[derive(Deserialize)]
//! struct Foo { x: i32 }
//!
//! let raw: Result<String, WeilError> = Ok(r#"{"x": 42}"#.to_string());
//! let foo: Foo = try_into_result(raw)?;
//! assert_eq!(foo.x, 42);
//! # Ok::<(), String>(())
//! ```

use crate::errors::WeilError;
use anyhow::Result;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::{SystemTime, UNIX_EPOCH};

/// Convert a `Result<String, WeilError>` containing JSON into a typed value `T`.
///
/// This helper expects the `Ok` variant to be a JSON string and attempts to
/// deserialize it into `T` using Serde. Any error—either from the original
/// `WeilError` or from JSON decoding—is converted into a `String`.
///
/// # Type Parameters
/// * `T`: The target deserializable type (must implement [`DeserializeOwned`]).
///
/// # Arguments
/// * `result` — A result whose `Ok` contains JSON text and whose `Err` is a [`WeilError`].
///
/// # Returns
/// * `Ok(T)` if deserialization succeeds,
/// * `Err(String)` with a human-readable message if either:
///   - the input was `Err(WeilError)`; or
///   - the JSON could not be parsed into `T`.
///
/// # Errors
/// - Returns `Err(String)` with the stringified `WeilError` if `result` is `Err`.
/// - Returns `Err(String)` with the Serde error message if JSON parsing fails.
pub fn try_into_result<T: DeserializeOwned>(
    result: Result<String, WeilError>,
) -> Result<T, String> {
    let val = result.map_err(|err| err.to_string())?;
    let ok_val: T = serde_json::from_str(&val).map_err(|err| err.to_string())?;

    Ok(ok_val)
}

/// Clean a query string before handing it to an LLM (e.g., for Aurora/Snowflake prompts).
///
/// This helper normalizes *stringified* SQL by removing escape noise and
/// collapsing whitespace so the model sees a single, readable line.
/// It is intended for **the SQL snippet itself**, not for entire JSON envelopes.
///
/// ## What it does
/// 1. Replaces the **literal** two-character sequence `\n` with a space
///    (i.e., turns `SELECT\\nFROM` into `SELECT FROM`).  
///    *Note:* This does **not** touch real newline bytes; only escaped `\\n`.
/// 2. Collapses all consecutive whitespace (spaces, tabs, real newlines) into a
///    single ASCII space using [`str::split_whitespace`].
/// 3. Removes all remaining backslashes (`\`), which typically come from JSON
///    string escaping (e.g., `\"` → `"`).
/// 4. Trims leading/trailing whitespace.
///
/// ## Typical usage
/// Use this on the **SQL substring** extracted from a larger payload, e.g.,
/// the value mapped by `"query_str"` in your request. Do *not* run it over the
/// entire JSON blob, because step (3) will remove escape characters needed to
/// keep JSON valid.
///
/// ## Example (cleaning a JSON-escaped SQL string)
/// ```
/// let raw_sql = r#"SELECT \n    COUNT(DISTINCT CS_BILL_CUSTOMER_SK) AS Billed_Customers,\n    
/// COUNT(DISTINCT CS_SHIP_CUSTOMER_SK) AS Shipped_Customers\nFROM \n    
/// SNOWFLAKE_SAMPLE_DATA.TPCDS_SF10TCL.CATALOG_SALES\nWHERE \n    
/// CS_SOLD_DATE_SK BETWEEN (SELECT D_DATE_SK FROM SNOWFLAKE_SAMPLE_DATA.TPCDS_SF10TCL.DATE_DIM WHERE D_DATE = '2002-01-01') \n    
/// AND (SELECT D_DATE_SK FROM SNOWFLAKE_SAMPLE_DATA.TPCDS_SF10TCL.DATE_DIM WHERE D_DATE = '2002-01-31')\n    
/// AND CS_BILL_CUSTOMER_SK IS NOT NULL\n    AND CS_SHIP_CUSTOMER_SK IS NOT NULL;\n"#;
///
/// let cleaned = cleanse_input_string(raw_sql);
///
/// // Becomes a single line, JSON escapes removed, extra spaces collapsed:
/// assert_eq!(cleaned, "SELECT COUNT(DISTINCT CS_BILL_CUSTOMER_SK) AS Billed_Customers, COUNT(DISTINCT CS_SHIP_CUSTOMER_SK)
/// AS Shipped_Customers FROM SNOWFLAKE_SAMPLE_DATA.TPCDS_SF10TCL.CATALOG_SALES WHERE CS_SOLD_DATE_SK
/// BETWEEN (SELECT D_DATE_SK FROM SNOWFLAKE_SAMPLE_DATA.TPCDS_SF10TCL.DATE_DIM WHERE D_DATE = '2002-01-01')
/// AND (SELECT D_DATE_SK FROM SNOWFLAKE_SAMPLE_DATA.TPCDS_SF10TCL.DATE_DIM WHERE D_DATE = '2002-01-31')
/// AND CS_BILL_CUSTOMER_SK IS NOT NULL AND CS_SHIP_CUSTOMER_SK IS NOT NULL;");
/// ```
///
/// ## Caution
/// - **Scope:** Only run on the SQL snippet (e.g., `"query_str"`), not on the whole
///   request JSON string. Removing backslashes from the whole JSON will corrupt it.
/// - **Backslashes:** All `\` are stripped. If your query *must* include a literal
///   backslash (rare for SQL), this function will remove it.
/// - **Semantics:** The transformation is cosmetic (whitespace + escapes).
///   It should not change SQL semantics, but always test queries you auto-format
///   before execution in production.
pub fn cleanse_input_string(input: &str) -> String {
    input
        // Replace escaped newlines like "\n" with a space
        .replace("\\n", " ")
        // Replace multiple spaces with a single space
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        // Remove unnecessary backslashes
        .replace("\\", "")
        .trim()
        .to_string()
}

/// Parsed absolute time interval derived from a human-readable phrase.
#[derive(Serialize, Deserialize)]
pub struct ParsedTimeInterval {
    /// Start of the interval (typically RFC3339, as defined by the host).
    pub from: String,
    /// End of the interval (typically RFC3339, as defined by the host).
    pub to: String,
}

/// Fixed width (digits) used when formatting day-epochs (see [`get_per_day_epoch`]).
///
/// Ensures lexicographic order matches numeric order when stored as strings.
const EPOCH_WIDTH: usize = 10;

/// Number of seconds in a day (24 * 60 * 60).
const SECS_IN_DAY: u64 = 86400;

/// Left-pad a numeric value with zeros to `pad` digits so that
/// **lexicographic sort equals numeric sort**.
///
/// This is useful when storing counters/timestamps as strings and you want
/// consistent ordering in key-value stores or filenames.
///
/// # Examples
/// ```
/// assert_eq!(pad_to(7, 3), "007");
/// assert_eq!(pad_to(123, 3), "123");
/// ```
fn pad_to(value: usize, pad: usize) -> String {
    format!("{:0>width$}", value.to_string(), width = pad)
}

/// Return the **per-day epoch** as a zero-padded string of width [`EPOCH_WIDTH`].
///
/// The value is computed as:
/// `floor((now_unix_seconds) / SECS_IN_DAY)`
///
/// This is handy for building daily partitions, keys, or metrics buckets where
/// string ordering should match time ordering.
///
/// # Errors
/// Returns `Err(String)` only if system time is before the Unix epoch.
///
/// # Examples
/// ```
/// let day = get_per_day_epoch().unwrap();
/// assert!(day.len() >= 1);
/// ```
pub fn get_per_day_epoch() -> Result<String, String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| err.to_string())?;
    let now = now.as_secs() / SECS_IN_DAY;
    Ok(pad_to(now as usize, EPOCH_WIDTH))
}

/// Arguments + state bundle passed into a stateful operation.
///
/// - `state`: opaque state payload (caller-defined).
/// - `args`: serialized arguments (e.g., JSON string) to the operation.
#[derive(Debug, Serialize, Deserialize)]
pub struct StateArgsValue {
    /// Caller-provided state (opaque string).
    pub state: String,
    /// Serialized arguments (typically JSON).
    pub args: String,
}

impl StateArgsValue {
    /// Construct a new [`StateArgsValue`].
    pub fn new(state: String, args: String) -> Self {
        StateArgsValue { state, args }
    }
}

/// Result + (optional) next state returned by a stateful operation.
///
/// - `state`: state to persist for the next call (if any).
/// - `value`: serialized result payload (e.g., JSON string).
#[derive(Debug, Serialize, Deserialize)]
pub struct StateResultValue {
    /// Optional next-state to persist.
    pub state: Option<String>,
    /// Serialized result (typically JSON).
    pub value: String,
}

impl StateResultValue {
    /// Construct a new [`StateResultValue`].
    pub fn new(state: Option<String>, value: String) -> Self {
        StateResultValue { state, value }
    }
}

/// JSON-RPC 2.0 params representation (untagged).
///
/// Accepts any of:
/// - `null` → [`RpcParams::None`]
/// - array → [`RpcParams::Array`]
/// - object → [`RpcParams::Map`]
///
/// Being *untagged* makes it match incoming JSON directly without extra wrappers.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RpcParams {
    /// No params (`null`).
    None,
    /// Positional params (`[...]`).
    Array(Vec<Value>),
    /// Named params (`{...}`).
    Map(serde_json::Map<String, Value>),
}

/// JSON-RPC 2.0 request envelope.
///
/// See <https://www.jsonrpc.org/specification>
/// - `jsonrpc`: must be `"2.0"`.
/// - `method`: the method name to invoke.
/// - `params`: optional positional or named params.
/// - `id`: request identifier (string/number/null).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    /// Protocol version (`"2.0"`).
    pub jsonrpc: String,
    /// Method name.
    pub method: String,
    /// Optional params (positional or named).
    pub params: Option<RpcParams>,
    /// Request id (string/number/null).
    pub id: Option<Value>, // can be String, Number, or Null
}

/// Optional metadata describing an applet (for discovery/UX).
///
/// All fields are optional to allow partial descriptors.
///
/// Typical usage:
/// - `author`: display name or handle.
/// - `description`: short human-readable summary.
/// - `organization`: publisher or owning org.
/// - `logo`: URL or resource identifier of a logo image.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppletDetails {
    /// Author name/handle.
    pub author: Option<String>,
    /// One-paragraph description.
    pub description: Option<String>,
    /// Publishing organization.
    pub organization: Option<String>,
    /// Logo URL or asset key.
    pub logo: Option<String>,
}

impl AppletDetails {
    /// Construct a new [`AppletDetails`].
    pub fn new(
        author: Option<String>,
        description: Option<String>,
        organization: Option<String>,
        logo: Option<String>,
    ) -> Self {
        AppletDetails {
            author,
            description,
            organization,
            logo,
        }
    }
}

/// JSON-RPC error codes (subset of the standard).
///
/// Maps to negative integer codes as defined by the JSON-RPC 2.0 spec:
/// - `ParseError`       → -32700
/// - `InvalidRequest`   → -32600
/// - `MethodNotFound`   → -32601
/// - `InvalidParams`    → -32602
/// - `InternalError`    → -32603
pub enum ErrorCode {
    /// Invalid JSON was received by the server.
    ParseError,
    /// The JSON sent is not a valid Request object.
    InvalidRequest,
    /// The method does not exist / is unavailable.
    MethodNotFound,
    /// Invalid method parameter(s).
    InvalidParams,
    /// Internal JSON-RPC error.
    InternalError,
}

impl ErrorCode {
    /// Convert to the numeric JSON-RPC code.
    fn to_code(&self) -> i64 {
        match self {
            ErrorCode::ParseError => -32700,
            ErrorCode::InvalidRequest => -32600,
            ErrorCode::MethodNotFound => -32601,
            ErrorCode::InvalidParams => -32602,
            ErrorCode::InternalError => -32603,
        }
    }
}

/// JSON-RPC error envelope.
///
/// - `code`: a numeric code (usually negative; see [`ErrorCode`]).
/// - `message`: short description for humans.
/// - `data`: optional extra info (structured JSON).
#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcError {
    /// Numeric error code.
    pub code: i64,
    /// Human-readable message.
    pub message: String,
    /// Optional structured data for clients.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// JSON-RPC 2.0 response envelope.
///
/// Exactly one of `result` or `error` **must** be present. The `id` must
/// match the originating request.
///
/// Use the constructors:
/// - [`JsonRpcResponse::ok`] for success responses
/// - [`JsonRpcResponse::err`] for error responses
#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    /// Protocol version (`"2.0"`).
    pub jsonrpc: String,
    /// Successful result payload.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    /// Error payload.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
    /// Echoed request id.
    pub id: Value,
}

impl JsonRpcResponse {
    /// Construct a successful response with `result`.
    ///
    /// # Examples
    /// ```
    /// let resp = JsonRpcResponse::ok(serde_json::json!(1), serde_json::json!({"ok":true}));
    /// assert!(resp.error.is_none());
    /// assert!(resp.result.is_some());
    /// ```
    pub fn ok<T: Serialize>(id: Value, result: T) -> Self {
        JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(json!(&result)),
            error: None,
        }
    }

    /// Construct an error response with a standard [`ErrorCode`].
    ///
    /// `data` allows attaching structured context for clients (e.g., validation
    /// details). It is omitted from serialization when `None`.
    ///
    /// # Examples
    /// ```
    /// let resp = JsonRpcResponse::err(
    ///     serde_json::json!("req-1"),
    ///     ErrorCode::InvalidParams,
    ///     "missing field `name`".to_string(),
    ///     Option::<serde_json::Value>::None
    /// );
    /// assert!(resp.result.is_none());
    /// assert!(resp.error.is_some());
    /// ```
    pub fn err<T: Serialize>(id: Value, code: ErrorCode, message: String, data: Option<T>) -> Self {
        JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code: code.to_code(),
                message,
                data: data.map(|d| json!(&d)),
            }),
        }
    }
}
