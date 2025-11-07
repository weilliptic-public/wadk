//! # Collection & Memory error types for Weil collections, WeilChain applets and runtime.
//!
//! This module defines strongly-typed error variants used by Weil collection
//! types (e.g., `WeilVec`) and memory/chunking utilities. Errors derive
//! [`thiserror::Error`] for ergonomic `Display` and `std::error::Error` support.
//!
//! This module exposes a single error enum, `WeilError`, with structured
//! context types (`MethodError`, `ContractCallError`). It is compatible with
//! `anyhow`, `thiserror`, and `serde` for transport across process boundaries.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Error returned when indexing beyond the bounds of a Weil collection
/// (e.g., accessing an element at `index` when `index >= len`).
#[derive(Debug, Error)]
#[error("index out of bound error: index {} accessed for length {}", self.index, self.len)]
pub struct IndexOutOfBoundsError {
    /// The index that was requested.
    pub index: usize,
    /// The collection length at the time of access.
    pub len: usize,
}

/// Error returned when a chunk's size does not match the expected size.
///
/// Typical for streaming or segmented I/O where each chunk must be of a fixed size.
#[derive(Debug, Error)]
#[error("chunk with invalid size received: expected with size {}, got {}", self.expected_size, self.received_size)]
pub struct InvalidChunkSizeError {
    /// Size (in bytes) that was expected for the chunk.
    pub expected_size: u32,
    /// Size (in bytes) actually received.
    pub received_size: u32,
}

/// Error returned when a single memory chunk exceeds the allowed WASM page limit.
///
/// In this runtime, a `WeilMemory` chunk must be ≤ 64 KiB (1 WASM page).
#[derive(Debug, Error)]
#[error("`WeilMemory` chunk size should be less than equal to 64 KiB (1 WASM Page), got {}", self.received_size)]
pub struct ChunkSizeError {
    /// Size (in bytes) actually received for the chunk.
    pub received_size: u32,
}

/// Context for method-level errors (name + message).
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MethodError {
    /// Name of the method where the error occurred.
    method_name: String,
    /// Human-readable error message.
    err_msg: String,
}

impl MethodError {
    /// Construct a new `MethodError`.
    pub fn new(method_name: String, err_msg: String) -> Self {
        MethodError {
            method_name,
            err_msg,
        }
    }
    /// Method name accessor.
    pub fn method_name(&self) -> &str {
        &self.method_name
    }
    /// Error message accessor.
    pub fn message(&self) -> &str {
        &self.err_msg
    }
}

/// Context for contract call errors (contract id + method + message).
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ContractCallError {
    /// Contract identifier (address / id string).
    contract_id: String,
    /// Method invoked on the contract.
    method_name: String,
    /// Human-readable error message.
    err_msg: String,
}

impl ContractCallError {
    /// Contract id accessor.
    pub fn contract_id(&self) -> &str {
        &self.contract_id
    }
    /// Method name accessor.
    pub fn method_name(&self) -> &str {
        &self.method_name
    }
    /// Error message accessor.
    pub fn message(&self) -> &str {
        &self.err_msg
    }
}

/// Top-level error for the Weil platform.
///
/// Variants are designed to preserve enough context for logging and for
/// serialization over the wire. Messages are human-readable; if you need
/// programmatic matching, prefer matching on the enum variants.
///
/// # Example
/// ```
/// use serde_json::json;
/// use weil_errors::{WeilError, MethodError};
///
/// // Construct from a serde_json error:
/// let bad = serde_json::from_str::<serde_json::Value>("not-json").unwrap_err();
/// let err = WeilError::new_method_argument_deserialization_error("init".into(), bad);
/// assert!(matches!(err, WeilError::MethodArgumentDeserializationError(_)));
///
/// // Display is human-friendly:
/// let s = err.to_string();
/// assert!(s.contains("arguments for method `init` cannot be deserialized"));
/// ```
#[derive(Debug, Error, Serialize, Deserialize, Clone)]
#[non_exhaustive]
pub enum WeilError {
    #[error("arguments for method `{}` cannot be deserialized: {}", ._0.method_name, ._0.err_msg)]
    MethodArgumentDeserializationError(MethodError),

    #[error("method `{}` returned with an error: {}", ._0.method_name, ._0.err_msg)]
    FunctionReturnedWithError(MethodError),

    #[error("a trap occurred while executing method `{}`: {}", ._0.method_name, ._0.err_msg)]
    TrapOccurredWhileWasmModuleExecution(MethodError),

    #[error("key `{0}` not found in the collection state")]
    KeyNotFoundInCollection(String),

    #[error("no value returned from deleting collection item with key `{0}`")]
    NoValueReturnedFromDeletingCollectionItem(String),

    #[error("no entry found in collections for keys with prefix `{0}`")]
    EntriesNotFoundInCollectionForKeysWithPrefix(String),

    #[error("error occurred while executing contract method `{}` with id `{}`: {}", ._0.method_name, ._0.contract_id, ._0.err_msg)]
    ContractMethodExecutionError(ContractCallError),

    #[error("invalid cross contract call with id `{}` to method `{}`: {}", ._0.contract_id, ._0.method_name, ._0.err_msg)]
    InvalidCrossContractCallError(ContractCallError),

    #[error("result from cross contract call with id `{}` and method `{}` cannot be deserialized: {}", ._0.contract_id, ._0.method_name, ._0.err_msg)]
    CrossContractCallResultDeserializationError(ContractCallError),

    #[error("LLM cluster error occurred: {0}")]
    LLMClusterError(String),

    #[error("streaming response cannot be deserialized: {0}")]
    StreamingResponseDeserializationError(String),

    #[error("invalid data received: {0}")]
    InvalidDataReceivedError(String),

    #[error("error making an outcall: {0}")]
    OutcallError(String),

    #[error("invalid wasm module: {0}")]
    InvalidWasmModuleError(String),

    #[error("platform error: {0}")]
    PlatformError(String),

    #[error("stream error: {0}")]
    ByteStreamError(String),
}

impl WeilError {
    /// Helper: wrap a `serde_json::Error` as a method argument deserialization error.
    pub fn new_method_argument_deserialization_error(
        method_name: String,
        serde_error: serde_json::Error,
    ) -> Self {
        Self::MethodArgumentDeserializationError(MethodError::new(
            method_name,
            serde_error.to_string(),
        ))
    }

    pub fn new_byte_stream_error(msg: String) -> Self {
        Self::ByteStreamError(msg)
    }
    pub fn new_platform_error(msg: String) -> Self {
        Self::PlatformError(msg)
    }
    pub fn new_outcall_error(msg: String) -> Self {
        Self::OutcallError(msg)
    }
    pub fn new_streaming_response_deserialization_error(msg: String) -> Self {
        Self::StreamingResponseDeserializationError(msg)
    }
    pub fn new_llm_cluster_error(msg: String) -> Self {
        Self::LLMClusterError(msg)
    }
    pub fn new_no_value_returned_from_deleting_collection_item_error(key: String) -> Self {
        Self::NoValueReturnedFromDeletingCollectionItem(key)
    }
    pub fn new_entries_not_found_in_collection_for_keys_with_prefix_error(prefix: String) -> Self {
        Self::EntriesNotFoundInCollectionForKeysWithPrefix(prefix)
    }
    pub fn new_function_returned_with_error<T: ToString>(method_name: String, err: T) -> Self {
        Self::FunctionReturnedWithError(MethodError::new(method_name, err.to_string()))
    }
    pub fn new_trap_occurred_while_module_execution_error(
        method_name: String,
        err: anyhow::Error,
    ) -> Self {
        Self::TrapOccurredWhileWasmModuleExecution(MethodError::new(method_name, err.to_string()))
    }
    pub fn new_key_not_found_in_collection_error(key: String) -> Self {
        Self::KeyNotFoundInCollection(key)
    }
    pub fn new_contract_method_execution_error<T: ToString>(
        contract_id: String,
        method_name: String,
        err: T,
    ) -> Self {
        Self::ContractMethodExecutionError(ContractCallError {
            contract_id,
            method_name,
            err_msg: err.to_string(),
        })
    }
    pub fn new_invalid_cross_contract_call_error(
        contract_id: String,
        method_name: String,
        err: String,
    ) -> Self {
        Self::InvalidCrossContractCallError(ContractCallError {
            contract_id,
            method_name,
            err_msg: err,
        })
    }
    pub fn new_cross_contract_call_result_deserialization_error(
        contract_id: String,
        method_name: String,
        serde_error: serde_json::Error,
    ) -> Self {
        Self::CrossContractCallResultDeserializationError(ContractCallError {
            contract_id,
            method_name,
            err_msg: serde_error.to_string(),
        })
    }
}
