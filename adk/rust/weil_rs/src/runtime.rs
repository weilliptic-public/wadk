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
    basicutils::{is_valid_key, INVALID_KEY_ERROR},
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
    fn write_collection_with_suffix(key: i32, val: i32, row_suffix: i32);
    fn delete_collection_with_suffix(key: i32, row_suffix: i32) -> i32;
    fn read_collection_with_suffix(key: i32, row_suffix: i32) -> i32;
    fn read_bulk_collection_with_suffix(prefix: i32, row_suffix: i32) -> i32;
    fn read_items_with_suffix(
        items_json: i32,
        row_suffix: i32,
    ) -> i32;
    fn read_items(items_json: i32) -> i32;
    fn read_bulk_collection_with_range_and_suffix(
        start_key: i32,
        end_key: i32,
        row_suffix: i32,
    ) -> i32;
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
    fn attest(org: i32, wallet_addr: i32, txn_id: i32, claim_data: i32, webhook: i32) -> i32;
    fn get_txn_instantiator_addr() -> i32;
    fn get_txn_id() -> i32;
    fn get_txn_from_addr(txn_id: i32) -> i32;
    fn get_pod_id_from_address(wallet_addr: i32) -> i32;
    fn list_contract_transactions(ptr: i32) -> i32;
    fn aggregate_contract_transactions(ptr: i32) -> i32;
    fn audit(audit_params: i32) -> i32;
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

/// Validate that a collection key does not contain
/// invalid characters.
fn validate_collection_key(key: &str) -> Result<(), String> {
    if is_valid_key(key) {
        Ok(())
    } else {
        Err(INVALID_KEY_ERROR.to_string())
    }
}

impl Memory {
    /// Insert or overwrite a collection entry at `key` with serialized `val`.
    pub fn write_collection<V: Serialize>(key: String, val: V) -> Result<(), String> {
        validate_collection_key(&key)?;

        let raw_key = get_length_prefixed_bytes_from_string(&key, 0);
        let raw_val = get_length_prefixed_bytes_from_result(Ok(val));

        // SAFETY: Both buffers are valid length-prefixed byte slices in WASM memory.
        unsafe { write_collection(raw_key.as_ptr() as _, raw_val.as_ptr() as _) };
        
        Ok(())
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

/// Insert or overwrite a collection entry at `key` with serialized `val`
    /// targeting the partitioned row `{contract_id}_{row_suffix}`.
    pub fn write_collection_with_suffix<V: Serialize>(
        key: String,
        val: V,
        row_suffix: &str,
    ) -> Result<(), String> {
        validate_collection_key(&key)?;

        let raw_key = get_length_prefixed_bytes_from_string(&key, 0);
        let raw_val = get_length_prefixed_bytes_from_result(Ok(val));
        let raw_suffix = get_length_prefixed_bytes_from_string(row_suffix, 0);

        // SAFETY: All three buffers are valid length-prefixed byte slices in WASM memory.
        unsafe {
            write_collection_with_suffix(
                raw_key.as_ptr() as _,
                raw_val.as_ptr() as _,
                raw_suffix.as_ptr() as _,
            )
        };

        Ok(())
    }

    /// Delete a collection entry from the partitioned row
    /// `{contract_id}_{row_suffix}` and (optionally) return its previous value.
    pub fn delete_collection_with_suffix<V: DeserializeOwned>(
        key: String,
        row_suffix: &str,
    ) -> Option<V> {
        let raw_key = get_length_prefixed_bytes_from_string(&key, 0);
        let raw_suffix = get_length_prefixed_bytes_from_string(row_suffix, 0);
        // SAFETY: Both buffers are valid length-prefixed buffers; host returns a status/result pointer.
        let ptr =
            unsafe { delete_collection_with_suffix(raw_key.as_ptr() as _, raw_suffix.as_ptr() as _) };

        match read_bytes_from_memory(ptr) {
            Ok(buffer) => Some(serde_json::from_str::<V>(&buffer).unwrap()),
            Err(err) => {
                let WeilError::NoValueReturnedFromDeletingCollectionItem(_) = err else {
                    panic!(
                        "panic occured while deletion of collection key `{}` => {}",
                        key, err
                    )
                };

                None
            }
        }
    }

    /// Read a collection entry by `key` from the partitioned row
    /// `{contract_id}_{row_suffix}`, returning `None` if the key does not exist.
    pub fn read_collection_with_suffix<V: DeserializeOwned>(
        key: String,
        row_suffix: &str,
    ) -> Option<V> {
        let raw_key = get_length_prefixed_bytes_from_string(&key, 0);
        let raw_suffix = get_length_prefixed_bytes_from_string(row_suffix, 0);
        // SAFETY: Both buffers are valid length-prefixed buffers; host returns a status/result pointer.
        let ptr =
            unsafe { read_collection_with_suffix(raw_key.as_ptr() as _, raw_suffix.as_ptr() as _) };

        match read_bytes_from_memory(ptr) {
            Ok(buffer) => Some(serde_json::from_str::<V>(&buffer).unwrap()),
            Err(err) => {
                let WeilError::KeyNotFoundInCollection(_) = err else {
                    panic!(
                        "panic occured while reading collection key `{}` => {}",
                        key, err
                    )
                };

                None
            }
        }
    }

    /// Read all entries whose keys start with `prefix` from the partitioned row
    /// `{contract_id}_{row_suffix}` as raw JSON string.
    fn read_bulk_collection_with_suffix(
        prefix: &str,
        row_suffix: &str,
    ) -> Result<String, WeilError> {
        let raw_prefix = get_length_prefixed_bytes_from_string(prefix, 0);
        let raw_suffix = get_length_prefixed_bytes_from_string(row_suffix, 0);
        // SAFETY: Both buffers are valid length-prefixed buffers; host returns a status/result pointer.
        let ptr = unsafe {
            read_bulk_collection_with_suffix(raw_prefix.as_ptr() as _, raw_suffix.as_ptr() as _)
        };
        let value = read_bytes_from_memory(ptr)?;

        Ok(value)
    }

    /// Read a prefix map for a trie from the partitioned row
    /// `{contract_id}_{row_suffix}`, deserializing to [`WeilTriePrefixMap<T>`].
    pub fn read_prefix_for_trie_with_suffix<T: DeserializeOwned>(
        prefix: String,
        row_suffix: &str,
    ) -> Option<WeilTriePrefixMap<T>> {
        match Memory::read_bulk_collection_with_suffix(&prefix, row_suffix) {
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

    /// Exact-key multi-get against a partitioned row. Each entry in
    /// `items` is the FULL column key (not a prefix); the host returns
    /// every matching (key, value) on the row in one SQL query.
    fn read_items_with_suffix(
        items: &[String],
        row_suffix: &str,
    ) -> Result<String, WeilError> {
        let items_json = serde_json::to_string(items).unwrap();
        let raw_items = get_length_prefixed_bytes_from_string(&items_json, 0);
        let raw_suffix = get_length_prefixed_bytes_from_string(row_suffix, 0);
        // SAFETY: Both buffers are valid length-prefixed buffers; host returns a status/result pointer.
        let ptr = unsafe {
            read_items_with_suffix(
                raw_items.as_ptr() as _,
                raw_suffix.as_ptr() as _,
            )
        };
        let value = read_bytes_from_memory(ptr)?;

        Ok(value)
    }

    /// No-suffix variant of `read_items_with_suffix`. Reads the default
    /// contract row by exact-key multi-get. Same wire shape as the
    /// with-suffix variant; just no row_suffix parameter.
    fn read_items(items: &[String]) -> Result<String, WeilError> {
        let items_json = serde_json::to_string(items).unwrap();
        let raw_items = get_length_prefixed_bytes_from_string(&items_json, 0);
        // SAFETY: `raw_items` is a valid length-prefixed buffer; host returns a status/result pointer.
        let ptr = unsafe { read_items(raw_items.as_ptr() as _) };
        let value = read_bytes_from_memory(ptr)?;
        Ok(value)
    }

    /// Read a typed prefix map for a trie from the partitioned row by
    /// an exact-key item list. Each entry in `items` is a full column
    /// key; the host returns the matching set as one
    /// [`WeilTriePrefixMap<T>`] in a single call.
    ///
    /// Used by aggregation queries that need a known set of records
    /// from a row in one shot — collapses N FFI crossings + N SQL
    /// queries into 1 + 1.
    pub fn read_items_for_trie_with_suffix<T: DeserializeOwned>(
        items: Vec<String>,
        row_suffix: &str,
    ) -> Option<WeilTriePrefixMap<T>> {
        match Memory::read_items_with_suffix(&items, row_suffix) {
            Ok(buffer) => Some(serde_json::from_str::<WeilTriePrefixMap<T>>(&buffer).unwrap()),
            Err(err) => {
                let WeilError::EntriesNotFoundInCollectionForKeysWithPrefix(_) = err else {
                    panic!(
                        "panic occured while reading items `{:?}` for trie => {}",
                        items, err
                    )
                };

                None
            }
        }
    }

    /// Typed wrapper for `WeilMap` callers — returns a flat
    /// `std::collections::HashMap<storage_key, V>` from the partitioned
    /// row's exact-key multi-get. Reuses the existing
    /// `read_items_with_suffix` host fn; per-pair deserialization is
    /// handled by [`parse_items_as_map`].
    ///
    /// Companion to [`read_items_for_trie_with_suffix`] — same wire
    /// path, different return shape suited to `HashMap` callers.
    pub fn read_items_for_map_with_suffix<V: DeserializeOwned>(
        items: &[String],
        row_suffix: &str,
    ) -> Option<std::collections::HashMap<String, V>> {
        match Memory::read_items_with_suffix(items, row_suffix) {
            Ok(buffer) => parse_items_as_map(&buffer),
            Err(err) => {
                let WeilError::EntriesNotFoundInCollectionForKeysWithPrefix(_) = err else {
                    panic!(
                        "panic occured while reading items `{:?}` for map (with suffix) => {}",
                        items, err
                    )
                };

                None
            }
        }
    }

    /// No-suffix variant of [`read_items_for_map_with_suffix`]. Reads
    /// the default contract row. Pairs with `WeilMap::getN(None, ...)`.
    pub fn read_items_for_map<V: DeserializeOwned>(
        items: &[String],
    ) -> Option<std::collections::HashMap<String, V>> {
        match Memory::read_items(items) {
            Ok(buffer) => parse_items_as_map(&buffer),
            Err(err) => {
                let WeilError::EntriesNotFoundInCollectionForKeysWithPrefix(_) = err else {
                    panic!(
                        "panic occured while reading items `{:?}` for map => {}",
                        items, err
                    )
                };

                None
            }
        }
    }

    /// Range variant of [`read_bulk_collection_with_suffix`]. Scans
    /// the partition row's columns over the closed byte-lex interval
    /// `[start_key, end_key]` in a single underlying SQL call.
    ///
    /// Constant-size FFI payload regardless of window width — three
    /// length-prefixed string buffers — versus the per-key item list
    /// that [`read_items_with_suffix`] requires.
    /// Use this when the column keys are lex-sortable (e.g. `pad_me`-
    /// padded numeric keys) and span a contiguous band.
    fn read_bulk_collection_with_range_and_suffix(
        start_key: &str,
        end_key: &str,
        row_suffix: &str,
    ) -> Result<String, WeilError> {
        let raw_start = get_length_prefixed_bytes_from_string(start_key, 0);
        let raw_end = get_length_prefixed_bytes_from_string(end_key, 0);
        let raw_suffix = get_length_prefixed_bytes_from_string(row_suffix, 0);
        // SAFETY: All three buffers are valid length-prefixed buffers;
        // host returns a status/result pointer through the standard
        // write_memory/read_memory plumbing.
        let ptr = unsafe {
            read_bulk_collection_with_range_and_suffix(
                raw_start.as_ptr() as _,
                raw_end.as_ptr() as _,
                raw_suffix.as_ptr() as _,
            )
        };
        let value = read_bytes_from_memory(ptr)?;
        Ok(value)
    }

    /// Read a prefix map for a trie from the partitioned row covering
    /// every column whose key lies in the closed byte-lex interval
    /// `[start_key, end_key]`. ONE host call, ONE SQL query, constant-
    /// size args.
    ///
    /// Companion to [`read_items_for_trie_with_suffix`] — pick the
    /// range form when the window is a contiguous lex band (the typical
    /// case for `pad_me`-padded day keys spanning a date window) and
    /// the prefix form when the window is a non-contiguous set of
    /// prefixes.
    pub fn read_range_for_trie_with_suffix<T: DeserializeOwned>(
        start_key: String,
        end_key: String,
        row_suffix: &str,
    ) -> Option<WeilTriePrefixMap<T>> {
        match Memory::read_bulk_collection_with_range_and_suffix(
            &start_key,
            &end_key,
            row_suffix,
        ) {
            Ok(buffer) => Some(serde_json::from_str::<WeilTriePrefixMap<T>>(&buffer).unwrap()),
            Err(err) => {
                let WeilError::EntriesNotFoundInCollectionForKeysWithPrefix(_) = err else {
                    panic!(
                        "panic occured while reading range `{}~{}` for trie => {}",
                        start_key, end_key, err
                    )
                };

                None
            }
        }
    }
}

/// Reshape the host's exact-key multi-get response into a
/// `HashMap<storage_key, V>` for `WeilMap` callers.
///
/// Wire format from the host (`read_items_*_from_db`) is
/// `Vec<(storage_key, value_json)>` — same shape used by the trie's
/// typed wrapper. We parse the outer Vec, then deserialize each
/// `value_json` into V before populating the map. Pairs with malformed
/// JSON values are silently skipped (matches the trie's
/// `serde_json::from_str(...).unwrap()` pattern but doesn't panic on
/// best-effort batch reads).
fn parse_items_as_map<V: DeserializeOwned>(
    buffer: &str,
) -> Option<std::collections::HashMap<String, V>> {
    let pairs: Vec<(String, String)> = serde_json::from_str(buffer).ok()?;
    let mut out = std::collections::HashMap::with_capacity(pairs.len());
    for (sk, v_json) in pairs {
        if let Ok(v) = serde_json::from_str::<V>(&v_json) {
            out.insert(sk, v);
        }
    }
    Some(out)
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

    pub fn origin() -> String {
        let ptr = unsafe { get_txn_instantiator_addr() };
        let addr = read_bytes_from_memory(ptr).unwrap();

        addr
    }

    pub fn get_txn_id() -> String {
        let ptr = unsafe { get_txn_id() };
        let tnx_id = read_bytes_from_memory(ptr).unwrap();

        tnx_id
    }

    /// Resolve an applet name to its contract identifier.
    pub fn contract_id_for_name(name: &str) -> Result<String, String> {
        let raw_name = get_length_prefixed_bytes_from_string(name, 0);
        // SAFETY: `raw_name` is a valid length-prefixed buffer; host returns a pointer to the ID string.
        let ptr = unsafe { applet_addr_for_name(raw_name.as_ptr() as _) };
        let Ok(applet_id) = read_bytes_from_memory(ptr) else {
            return Err(format!("Failed to resolve applet name '{}'", name));
        };

        Ok(applet_id)
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
        // SAFETY: `Ledger` is a systemic applet
        Runtime::contract_id_for_name("Ledger").unwrap()
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

    pub fn get_txn_from_addr(txn_id: String) -> Result<String, WeilError> {
        let raw_txn_id = get_length_prefixed_bytes_from_string(&txn_id, 0);
        let ptr = unsafe { get_txn_from_addr(raw_txn_id.as_ptr() as _) };
        match read_bytes_from_memory(ptr) {
            Ok(addr) => Ok(addr),
            Err(err) => Err(WeilError::InvalidWasmModuleError(format!(
                "Failed to get txn from address for txn_id '{}': {}",
                txn_id, err
            ))),
        }
    }

    pub fn get_pod_id_from_addr(wallet_addr: String) -> Result<String, WeilError> {
        let raw_wallet_addr = get_length_prefixed_bytes_from_string(&wallet_addr, 0);
        let ptr = unsafe { get_pod_id_from_address(raw_wallet_addr.as_ptr() as _) };
        match read_bytes_from_memory(ptr) {
            Ok(addr) => Ok(addr),
            Err(err) => Err(WeilError::InvalidWasmModuleError(format!(
                "Failed to get pod id from address for wallet_addr '{}': {}",
                wallet_addr, err
            ))),
        }
    }

    /// Query transaction history for the current contract from the block indexer.
    /// Accepts filters (HashMap<String, String>), limit, and offset.
    /// Returns a JSON string with `{ transactions: [...], total_count: N }`.
    pub fn list_contract_transactions(
        filters: std::collections::HashMap<String, String>,
        limit: usize,
        offset: usize,
    ) -> Result<String, WeilError> {
        #[derive(Serialize)]
        struct Args {
            filters: std::collections::HashMap<String, String>,
            limit: usize,
            offset: usize,
        }
        let args = Args {
            filters,
            limit,
            offset,
        };
        let json_args = serde_json::to_string(&args).unwrap();
        let raw_args = get_length_prefixed_bytes_from_string(&json_args, 0);
        let ptr = unsafe { list_contract_transactions(raw_args.as_ptr() as _) };
        read_bytes_from_memory(ptr)
    }

    /// Aggregate transaction data for the current contract from the block indexer.
    /// The `range_days` parameter determines both the time window and the grouping bucket:
    ///   - <= 14 days → group by day
    ///   - <= 90 days → group by week
    ///   - > 90 days  → group by month
    /// Returns a JSON string with aggregated data points.
    pub fn aggregate_contract_transactions(
        filters: std::collections::HashMap<String, String>,
        range_days: u32,
        value_field: String,
    ) -> Result<String, WeilError> {
        #[derive(Serialize)]
        struct Args {
            filters: std::collections::HashMap<String, String>,
            range_days: u32,
            value_field: String,
        }
        let args = Args {
            filters,
            range_days,
            value_field,
        };
        let json_args = serde_json::to_string(&args).unwrap();
        let raw_args = get_length_prefixed_bytes_from_string(&json_args, 0);
        let ptr = unsafe { aggregate_contract_transactions(raw_args.as_ptr() as _) };
        read_bytes_from_memory(ptr)
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

    pub fn attest(
        org: String,
        wallet_addr: String,
        txn_id: String,
        claim_data: String,
        webhook: String,
    ) -> Result<(), anyhow::Error> {
        let serialized_org = get_length_prefixed_bytes_from_string(&org, 0);
        let serialized_wallet_addr = get_length_prefixed_bytes_from_string(&wallet_addr, 0);
        let serialized_txn_id = get_length_prefixed_bytes_from_string(&txn_id, 0);
        let serialized_claim_data = get_length_prefixed_bytes_from_string(&claim_data, 0);
        let serialized_webhook = get_length_prefixed_bytes_from_string(&webhook, 0);

        let result_ptr = unsafe {
            attest(
                serialized_org.as_ptr() as _,
                serialized_wallet_addr.as_ptr() as _,
                serialized_txn_id.as_ptr() as _,
                serialized_claim_data.as_ptr() as _,
                serialized_webhook.as_ptr() as _,
            )
        };

        let _ = read_bytes_from_memory(result_ptr)?;

        Ok(())
    }

    // Audit logs the entry in block-explorer
    // Parameters:
    // - `mcp_server_name`: Name of the MCP server to which the audit log
    //   will be sent
    // - `task_id`: Unique identifier for the task being audited
    // - `applet_address`: Address of the applet being audited
    // - `applet_method`: Method of the applet being audited
    // - `request`: JSON string representing the log being audited
    // - `wallet_addr`: Wallet address associated with the audit log entry
    pub fn audit(
        mcp_server_name: String,
        task_id: String,
        applet_address: String,
        applet_method: String,
        request: String,
        wallet_addr: String,
    ) -> Result<(), anyhow::Error> {
        #[derive(Serialize)]
        struct AuditParams {
            mcp_server_name: String,
            task_id: String,
            applet_address: String,
            applet_method: String,
            request: String,
            wallet_addr: String,
        }

        let audit_params = AuditParams {
            mcp_server_name,
            task_id,
            applet_address,
            applet_method,
            request,
            wallet_addr,
        };

        let result_ptr =
            unsafe { audit(get_length_prefixed_bytes_from_result(Ok(audit_params)).as_ptr() as _) };

        let _ = read_bytes_from_memory(result_ptr)?;

        Ok(())
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
