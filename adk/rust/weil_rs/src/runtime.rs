//! # Weil WASM Runtime bridge and collection memory utilities
//!
//! This module defines the typed, ergonomic façade over the host-provided
//! (`wasm_import_module = "env"`) runtime for Weil applets. It includes:
//!
//! - **FFI bindings** for collection access, contract calls, logging, time/ID, etc.
//! - **Length-prefixed memory helpers** to pass data across the WASM boundary
//! - **`Memory`** helpers: type-safe collection read/write/delete and prefix scans
//! - **`Runtime`** helpers: state/args retrieval, cross-contract calls, logging, task spawn, etc.
//! - **`WeilValue`**: a small result wrapper for returning `(optional state, ok value)`
//!
//! ## Shared memory layout
//! Host return values follow this layout (little-endian):
//!
//! ```text
//! | ERROR (1 byte) | LEN (u32 LE) | BYTES (LEN bytes of UTF-8 JSON) |
//! ```
//!
//! The helpers here (e.g., [`read_bytes_from_memory`]) enforce that contract and surface
//! errors are represented as serialized `WeilError` when `ERROR == 1`.
//!
//! ## Safety
//! All `extern "C"` calls are `unsafe` and must be paired with a single call to
//! [`read_bytes_from_memory`] to copy/interpret the returned buffer in guest memory.
//! Each unsafe call is annotated with a `// SAFETY:` comment summarizing assumptions.

use crate::{
    collections::trie::map::WeilTriePrefixMap, traits::WeilType, utils::ParsedTimeInterval,
};
use crate::{
    errors::WeilError,
    utils::{AppletDetails, StateArgsValue, StateResultValue},
};
use anyhow::Result;
use async_executor::LocalExecutor;
use futures_lite::future::block_on;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{future::Future, mem::ManuallyDrop, ptr};

// Raw FFI surface from the host runtime.
//
// > **Do not** use these directly in applets. Prefer the safe wrappers in
// > [`Memory`] and [`Runtime`].
#[link(wasm_import_module = "env")]
extern "C" {
    fn write_collection(key: i32, val: i32);
    fn delete_collection(key: i32) -> i32;
    fn read_collection(key: i32) -> i32;
    fn read_bulk_collection(prefix: i32) -> i32;
    fn get_state_and_args() -> i32;
    fn get_sender() -> i32;
    fn get_block_height() -> i32;
    fn get_block_timestamp() -> i32;
    fn get_contract_id() -> i32;
    fn set_state_and_result(ptr: i32);
    fn call_contract(ptr: i32) -> i32;
    fn call_xpod_contract(ptr: i32) -> i32;
    fn debug_log(log: i32);
    fn uuid() -> i32;
    fn applet_addr_for_name(name: i32) -> i32;
    fn get_applet_details(applet_id: i32) -> i32;
    fn parse_human_time(s: i32) -> i32;
}

/// Wrapper for returning an optional state and a success value from a contract call.
///
/// Use [`WeilValue::raw`] to convert into the wire format [`StateResultValue`].
#[derive(Serialize, Deserialize)]
pub struct WeilValue<T, U> {
    /// Optional state to be persisted alongside the result.
    pub state: Option<T>,
    /// The successful return value.
    pub ok_val: U,
}

impl<T, U> WeilValue<T, U> {
    /// Construct a `WeilValue` containing only an OK value (no state update).
    pub fn new_with_ok_value(val: U) -> Self {
        WeilValue {
            state: None,
            ok_val: val,
        }
    }

    /// Construct a `WeilValue` containing both state and OK value.
    pub fn new_with_state_and_ok_value(state: T, val: U) -> Self {
        WeilValue {
            state: Some(state),
            ok_val: val,
        }
    }

    /// Returns `true` if this value includes a state payload.
    pub fn has_state(&self) -> bool {
        self.state.is_some()
    }
}

impl<T: Serialize, U: Serialize> WeilValue<T, U> {
    /// Convert to the host contract result envelope [`StateResultValue`].
    ///
    /// Serializes both `state` (if present) and `ok_val` into JSON strings as required
    /// by the host runtime.
    pub fn raw(&self) -> StateResultValue {
        StateResultValue::new(
            self.state
                .as_ref()
                .map(|state| serde_json::to_string(state).unwrap()),
            serde_json::to_string(&self.ok_val).unwrap(),
        )
    }
}

// Memory Layout for WASM memory shared between mmodule and host
// |ERROR ||LENGTH||VALID UTF-8 ENCODED STRING BYTES|
// |1 BYTE||4 BYTE||LENGTH BYTES ...................|

/// Read a host-returned, length-prefixed UTF-8 buffer and map to `Result<String, WeilError>`.
///
/// Interprets error sentinel values (`-1`, `-2`, `-3`) and the first byte flag:
/// - If `ERROR == 0`, returns the JSON string.
/// - If `ERROR == 1`, deserializes and returns a [`WeilError`].
///
/// # Errors
/// Returns a `WeilError` if the host surface indicates an error or if the buffer fails to parse.
///
/// # Safety
/// - `ptr` must be a valid address returned by a host FFI function for the current call.
/// - The buffer is read **once** and copied into guest memory.
///
/// (The function itself encapsulates the `unsafe` memory reads.)
pub(crate) fn read_bytes_from_memory(ptr: i32) -> Result<String, WeilError> {
    match ptr {
        -1 => {
            return Err(WeilError::InvalidWasmModuleError(
                "WASM size limit reached".to_owned(),
            ))
        }
        -2 => {
            return Err(WeilError::InvalidWasmModuleError(
                "invalid __new function export in module".to_owned(),
            ))
        }
        -3 => {
            return Err(WeilError::InvalidWasmModuleError(
                "invalid __free function export in module".to_owned(),
            ))
        }
        _ => {}
    };

    let ptr = ptr as *mut u8;
    let is_error = unsafe { *ptr };
    let mut len_buffer = [0u8; 4];

    // SAFETY: `ptr` is a valid host-provided buffer with at least 1 + 4 + LEN bytes.
    unsafe {
        ptr::copy_nonoverlapping(ptr.add(1), len_buffer.as_mut_ptr(), 4);
    }

    let len = u32::from_le_bytes(len_buffer) as usize;

    // let mut buffer: Vec<u8> = Vec::with_capacity(len as usize);
    let mut buffer: Vec<u8> = vec![0; len];

    // SAFETY: Source points to `ptr + 5` for `len` bytes; destination is sized to `len`.
    unsafe {
        ptr::copy_nonoverlapping(ptr.add(1 + 4), buffer.as_mut_ptr(), len);
    }

    let serialized_str = String::from_utf8(buffer).unwrap();

    if is_error == 0 {
        Ok(serialized_str)
    } else {
        let err: WeilError = serde_json::from_str(&serialized_str).unwrap();
        Err(err)
    }
}

/// Serialize a `Result<T, WeilError>` into the length-prefixed host wire format.
///
/// When `Ok`, encodes the value as JSON with error flag `0`.
/// When `Err`, encodes the [`WeilError`] as JSON with error flag `1`.
pub(crate) fn get_length_prefixed_bytes_from_result<T: Serialize>(
    payload: Result<T, WeilError>,
) -> Vec<u8> {
    let (serialized_payload, is_error) = match payload {
        Ok(payload) => (serde_json::to_string(&payload).unwrap(), 0),
        Err(err) => (serde_json::to_string(&err).unwrap(), 1),
    };

    get_length_prefixed_bytes_from_string(&serialized_payload, is_error)
}

/// Serialize a `&str` into the length-prefixed host wire format with explicit error flag.
///
/// Layout: `[is_error: u8] [len: u32 LE] [payload bytes...]`.
pub(crate) fn get_length_prefixed_bytes_from_string(payload: &str, is_error: u8) -> Vec<u8> {
    let payload_bytes = payload.as_bytes();
    let len = payload_bytes.len() as u32;
    let mut buffer: Vec<u8> = Vec::with_capacity(1 + 4 + len as usize);

    buffer.push(is_error);
    buffer.extend_from_slice(&len.to_le_bytes());
    buffer.extend_from_slice(payload_bytes);

    buffer
}

/// `Memory` is used to provide type-safe APIs for implementing `Weil` Collections.
///
/// It wraps host FFI for collection operations and performs JSON (de)serialization.
pub(crate) struct Memory;

impl Memory {
    /// Insert or overwrite a collection entry at `key` with serialized `val`.
    pub fn write_collection<V: Serialize>(key: String, val: V) {
        let raw_key = get_length_prefixed_bytes_from_string(&key, 0);
        let raw_val = get_length_prefixed_bytes_from_result(Ok(val));

        // SAFETY: Both buffers are valid length-prefixed byte slices in WASM memory.
        unsafe { write_collection(raw_key.as_ptr() as _, raw_val.as_ptr() as _) };
    }

    /// Delete a collection entry and (optionally) return its previous value.
    ///
    /// Returns `None` when no value was present (as indicated by the host error variant).
    pub fn delete_collection<V: DeserializeOwned>(key: String) -> Option<V> {
        let raw_key = get_length_prefixed_bytes_from_string(&key, 0);
        // SAFETY: `raw_key` is a valid length-prefixed buffer; host returns a status/result pointer.
        let ptr = unsafe { delete_collection(raw_key.as_ptr() as _) };

        match read_bytes_from_memory(ptr) {
            Ok(buffer) => Some(serde_json::from_str::<V>(&buffer).unwrap()),
            Err(err) => {
                let WeilError::NoValueReturnedFromDeletingCollectionItem(_) = err else {
                    // any other error other than `KeyNotFoundInCollection` is the state of panic and
                    // program execution should just stops as probably there is nothing the developer
                    // can do by handling other variants if we would have returned it instead of panic
                    panic!(
                        "panic occured while deletion of collection key `{}` => {}",
                        key, err
                    )
                };

                None
            }
        }
    }

    /// Read a collection entry by `key`, returning `None` if the key does not exist.
    pub fn read_collection<V: DeserializeOwned>(key: String) -> Option<V> {
        let raw_key = get_length_prefixed_bytes_from_string(&key, 0);
        // SAFETY: `raw_key` is a valid length-prefixed buffer; host returns a status/result pointer.
        let ptr = unsafe { read_collection(raw_key.as_ptr() as _) };

        match read_bytes_from_memory(ptr) {
            Ok(buffer) => Some(serde_json::from_str::<V>(&buffer).unwrap()),
            Err(err) => {
                let WeilError::KeyNotFoundInCollection(_) = err else {
                    // any other error other than `KeyNotFoundInCollection` is the state of panic and
                    // program execution should just stops as probably there is nothing the developer
                    // can do by handling other variants if we would have returned it instead of panic
                    panic!(
                        "panic occured while reading collection key `{}` => {}",
                        key, err
                    )
                };

                None
            }
        }
    }

    /// Read all entries whose keys start with `prefix` as raw JSON string.
    fn read_bulk_collection(prefix: &str) -> Result<String, WeilError> {
        let raw_prefix = get_length_prefixed_bytes_from_string(&prefix, 0);
        // SAFETY: `raw_prefix` is a valid length-prefixed buffer; host returns a status/result pointer.
        let ptr = unsafe { read_bulk_collection(raw_prefix.as_ptr() as _) };
        let value = read_bytes_from_memory(ptr)?;

        Ok(value)
    }

    /// Read a prefix map for a trie from the collection, deserializing to [`WeilTriePrefixMap<T>`].
    ///
    /// Returns `None` if no entries match the prefix.
    pub fn read_prefix_for_trie<T: DeserializeOwned>(
        prefix: String,
    ) -> Option<WeilTriePrefixMap<T>> {
        match Memory::read_bulk_collection(&prefix) {
            Ok(buffer) => Some(serde_json::from_str::<WeilTriePrefixMap<T>>(&buffer).unwrap()),
            Err(err) => {
                let WeilError::EntriesNotFoundInCollectionForKeysWithPrefix(_) = err else {
                    panic!(
                        "panic occured while reading prefix `{}` for trie => {}",
                        prefix, err
                    )
                };

                None
            }
        }
    }
}

/// Arguments envelope for cross-contract calls over FFI.
#[derive(Serialize)]
struct CrossContractCallArgs {
    id: String,
    method_name: String,
    method_args: String,
}

/// A manually-managed memory segment used for host allocations.
///
/// Backed by `Vec<u8>` inside a [`ManuallyDrop`] to control ownership explicitly.
pub(crate) struct MemorySegment(ManuallyDrop<Vec<u8>>);

impl MemorySegment {
    /// Create a new uninitialized segment with capacity `len`.
    fn new(len: usize) -> Self {
        MemorySegment(ManuallyDrop::new(Vec::with_capacity(len)))
    }
}

/// High-level runtime façade for Weil applets.
///
/// Provides safe wrappers for contract state/args access, cross-contract calls,
/// logging, scheduling, and result emission.
pub struct Runtime;

impl Runtime {
    /// Allocate a raw buffer of size `len` in guest memory and return its pointer.
    ///
    /// Intended for host to write into during FFI transfers.
    pub fn allocate(len: usize) -> *mut u8 {
        let mut v = MemorySegment::new(len);
        let v_ref = v.0.as_mut_ptr();

        v_ref
    }

    /// Deallocate a buffer previously allocated via [`Runtime::allocate`].
    ///
    /// # Safety
    /// `ptr` and `len` must match the allocation; this reconstructs a `Vec<u8>` and drops it.
    pub fn deallocate(ptr: usize, len: usize) {
        let ptr = ptr as *mut u8;
        let len = len as usize;
        // SAFETY: Recreates the original allocation so Rust can drop it and free memory.
        _ = unsafe { Vec::from_raw_parts(ptr, len, len) }; // this takes the ownership of the underlying buffer and then drops it
    }

    /// Retrieve the **current contract state** deserialized as `T`.
    ///
    /// Panics if host access fails or deserialization fails.
    pub fn state<T: WeilType>() -> T {
        // SAFETY: `get_state_and_args` returns a valid pointer to the shared state+args buffer.
        let ptr = unsafe { get_state_and_args() };

        let raw_state_and_args = match read_bytes_from_memory(ptr) {
            Ok(val) => val,
            Err(err) => panic!("panic occured while fetching contract state => {}", err),
        };

        let state_args = serde_json::from_str::<StateArgsValue>(&raw_state_and_args).unwrap();
        let state = serde_json::from_str::<T>(&state_args.state).unwrap();

        state
    }

    /// Retrieve **call arguments** as `T`.
    ///
    /// Panics if host access to state/args fails; returns `Err` only for argument deserialization.
    pub fn args<T: DeserializeOwned>() -> Result<T, serde_json::Error> {
        // SAFETY: `get_state_and_args` returns a valid pointer to the shared state+args buffer.
        let ptr = unsafe { get_state_and_args() };

        let raw_state_and_args = match read_bytes_from_memory(ptr) {
            Ok(val) => val,
            Err(err) => panic!("panic occured while fetching contract state => {}", err),
        };

        let state_args = serde_json::from_str::<StateArgsValue>(&raw_state_and_args).unwrap();

        serde_json::from_str(&state_args.args)
    }

    /// Retrieve both **state** and **args** together.
    ///
    /// Panics if host access fails; returns `Err` only for the argument deserialization half.
    pub fn state_and_args<T: WeilType, U: DeserializeOwned>() -> (T, Result<U, serde_json::Error>) {
        // SAFETY: `get_state_and_args` returns a valid pointer to the shared state+args buffer.
        let ptr = unsafe { get_state_and_args() };

        let raw_state_and_args = match read_bytes_from_memory(ptr) {
            Ok(val) => val,
            Err(err) => panic!("panic occured while fetching contract state => {}", err),
        };

        let state_args = serde_json::from_str::<StateArgsValue>(&raw_state_and_args).unwrap();

        let state = serde_json::from_str::<T>(&state_args.state).unwrap();
        let args = serde_json::from_str::<U>(&state_args.args);

        (state, args)
    }

    /// Returns contract identifier of the executing WeilApplet.
    pub fn contract_id() -> String {
        // SAFETY: `get_contract_id` returns a valid pointer to a UTF-8 JSON string.
        let ptr = unsafe { get_contract_id() };
        let contract_id = read_bytes_from_memory(ptr).unwrap();

        contract_id
    }

    /// Returns the address/identifier of the **caller** who invoked this method.
    ///
    /// - End-user contexts (wallet/dapp): returns the caller's address.
    /// - Cross-contract calls: returns the caller contract ID.
    pub fn sender() -> String {
        // SAFETY: `get_sender` returns a valid pointer to a UTF-8 JSON string.
        let ptr = unsafe { get_sender() };
        let addr = read_bytes_from_memory(ptr).unwrap();

        addr
    }

    /// Resolve an applet name to its contract identifier.
    pub fn contract_id_for_name(name: &str) -> String {
        let raw_name = get_length_prefixed_bytes_from_string(name, 0);
        // SAFETY: `raw_name` is a valid length-prefixed buffer; host returns a pointer to the ID string.
        let ptr = unsafe { applet_addr_for_name(raw_name.as_ptr() as _) };
        let applet_id = read_bytes_from_memory(ptr).unwrap();

        applet_id
    }

    /// Returns meta details for the provided applet id.
    ///
    /// Deserializes the host return into [`AppletDetails`], mapping decode failures
    /// into an appropriate [`WeilError`].
    pub fn get_applet_details(applet_id: &str) -> Result<AppletDetails, WeilError> {
        let raw_applet_id = get_length_prefixed_bytes_from_string(applet_id, 0);
        // SAFETY: `raw_applet_id` is a valid length-prefixed buffer; host returns a JSON pointer.
        let ptr = unsafe { get_applet_details(raw_applet_id.as_ptr() as _) };
        let applet_details_serialized = read_bytes_from_memory(ptr)?;

        let applet_details = serde_json::from_str::<AppletDetails>(&applet_details_serialized)
            .map_err(|err| {
                WeilError::InvalidWasmModuleError(format!(
                    "Failed to deserialize applet details for applet_id '{}': {}",
                    applet_id, err
                ))
            })?;
        Ok(applet_details)
    }

    /// Returns `Ledger` contract identifier.
    pub(crate) fn ledger_contract_id() -> String {
        Runtime::contract_id_for_name("Ledger")
    }

    /// Returns the current **block height** (deterministic across nodes in a pod).
    pub fn block_height() -> u64 {
        // SAFETY: `get_block_height` returns a pointer to a UTF-8 stringified integer.
        let ptr = unsafe { get_block_height() };
        let value = read_bytes_from_memory(ptr).unwrap();

        value.parse().unwrap()
    }

    /// Returns the current **block timestamp** (deterministic across nodes in a pod).
    pub fn block_timestamp() -> String {
        // SAFETY: `get_block_timestamp` returns a pointer to a UTF-8 JSON string timestamp.
        let ptr = unsafe { get_block_timestamp() };
        let block_timestamp = read_bytes_from_memory(ptr).unwrap();

        block_timestamp
    }

    /// Call a method of another **WeilApplet on the same pod** and deserialize its result to `R`.
    ///
    /// This is the *intra-pod* (synchronous) cross-contract call.
    ///
    /// # Errors
    /// - Returns `anyhow::Error` wrapping a [`WeilError`] if the host signals an error.
    /// - Returns deserialization errors as a cross-contract result decoding error.
    pub fn call_contract<R: DeserializeOwned>(
        contract_id: String,
        method_name: String,
        method_args: Option<String>,
    ) -> anyhow::Result<R> {
        let args = CrossContractCallArgs {
            id: contract_id.clone(),
            method_name: method_name.clone(),
            method_args: match method_args {
                Some(args) => args,
                None => "{}".to_string(),
            },
        };

        let args_buf = get_length_prefixed_bytes_from_result(Ok(args));
        // SAFETY: `args_buf` is a valid length-prefixed buffer; host returns a JSON result pointer.
        let result_ptr = unsafe { call_contract(args_buf.as_ptr() as _) };
        let serialized_result = read_bytes_from_memory(result_ptr)?;

        match serde_json::from_str::<R>(&serialized_result) {
            Ok(result) => Ok(result),
            Err(err) => Err(
                WeilError::new_cross_contract_call_result_deserialization_error(
                    contract_id,
                    method_name,
                    err,
                )
                .into(),
            ),
        }
    }

    /// Start an **xpod** (cross-pod) contract call and return a unique identifier for the invocation.
    ///
    /// This is a two-phase mechanism (`main` + `callback`). Call this from the `main` phase.
    /// The returned ID is also provided to the `callback` to correlate results.
    pub fn call_xpod_contract(
        contract_id: String,
        method_name: String,
        method_args: Option<String>,
    ) -> anyhow::Result<String> {
        let args = CrossContractCallArgs {
            id: contract_id.clone(),
            method_name: method_name.clone(),
            method_args: match method_args {
                Some(args) => args,
                None => "{}".to_string(),
            },
        };

        let args_buf = get_length_prefixed_bytes_from_result(Ok(args));
        // SAFETY: `args_buf` is a valid length-prefixed buffer; host returns a string ID pointer.
        let result_ptr = unsafe { call_xpod_contract(args_buf.as_ptr() as _) };
        let xpod_id = read_bytes_from_memory(result_ptr)?;

        Ok(xpod_id)
    }

    /// Write a debug log line into the platform’s pod nodes.
    pub fn debug_log(log: &str) {
        let raw_log = get_length_prefixed_bytes_from_result(Ok(log));
        // SAFETY: `raw_log` is a valid length-prefixed buffer; host consumes it synchronously.
        let _ = unsafe { debug_log(raw_log.as_ptr() as _) };
    }

    /// Run a future to completion on a local single-threaded executor and return its output.
    pub fn spawn_task<T>(task: impl Future<Output = T>) -> T {
        let ex = LocalExecutor::new();
        let task = ex.spawn(async { task.await });
        block_on(ex.run(task))
    }

    /// Helper to set only a result value (no state) from a contract method.
    ///
    /// Converts `Ok(val)` into `Ok(WeilValue::<(), T>)` and passes to the host.
    pub fn set_result<T: Serialize>(result: Result<T, WeilError>) {
        let result = match result {
            Ok(val) => Ok(WeilValue::<(), T>::new_with_ok_value(val)),
            Err(err) => Err(err),
        };

        Runtime::set_state_and_result(result);
    }

    /// Set both (optional) state and result for a contract method.
    ///
    /// Serializes to [`StateResultValue`] and sends to the host runtime.
    pub fn set_state_and_result<T: Serialize, U: Serialize>(
        result: Result<WeilValue<T, U>, WeilError>,
    ) {
        let result = match result {
            Ok(val) => Ok(val.raw()),
            Err(err) => Err(err),
        };

        let raw_result = get_length_prefixed_bytes_from_result(result);
        // SAFETY: `raw_result` is a valid length-prefixed buffer; host consumes it synchronously.
        unsafe { set_state_and_result(raw_result.as_ptr() as _) };
    }

    /// Returns a randomly generated UUID v4 string.
    pub fn uuid() -> String {
        // SAFETY: `uuid` returns a pointer to a UTF-8 string; host does not signal errors here.
        let ptr = unsafe { uuid() };
        let uuid_str = read_bytes_from_memory(ptr).unwrap(); // safe to unwrap since the
                                                             // host-native function does not have any error propagation, but just Ok(...)

        uuid_str
    }

    /// Parse a human-readable time span into a concrete interval.
    ///
    /// Examples of accepted inputs depend on host capabilities (e.g., `"last 24 hours"`,
    /// `"yesterday"`, `"next week"`). The result contains RFC3339 (or host-defined) `from`/`to`.
    ///
    /// # Errors
    /// - Returns an error if host memory cannot be read or the JSON cannot be deserialized.
    pub fn parse_human_time(s: &str) -> Result<ParsedTimeInterval> {
        let serialized_payload = get_length_prefixed_bytes_from_string(s, 0);
        // SAFETY: `serialized_payload` is valid; host returns a JSON for `ParsedTimeInterval`.
        let result_ptr = unsafe { parse_human_time(serialized_payload.as_ptr() as _) };
        let response = read_bytes_from_memory(result_ptr)?;
        let interval: ParsedTimeInterval = serde_json::from_str(&response).unwrap();

        Ok(interval)
    }
}
