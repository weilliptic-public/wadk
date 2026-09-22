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
