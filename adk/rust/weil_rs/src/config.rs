//! # Secrets/config bridge for WASM applets
//!
//! This module exposes a minimal, typed wrapper around a host-provided configuration
//! channel via a `wasm_import_module="env"` function. The host returns a pointer to
//! a length-prefixed UTF-8 JSON buffer that is deserialized into `T`.
//!
//! Use [`Secrets<T>`] to obtain strongly typed config with a safe default when the
//! host provides no value (i.e., `null`), or when the type cannot be found.
//!
//! ## Data flow
//! 1. Call into the host with [`get_config`] (FFI).
//! 2. Read the returned pointer with [`read_bytes_from_memory`] to obtain a JSON string.
//! 3. Deserialize as `Option<T>`; fall back to `T::default()` if `None`.
//!
//! ## FFI/memory safety
//! The host returns a pointer to a length-prefixed buffer. Callers must read the
//! pointer exactly once with [`read_bytes_from_memory`]. Each `unsafe` block is
//! annotated with a `// SAFETY:` comment describing assumptions.

use crate::runtime::read_bytes_from_memory;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::marker::PhantomData;

#[link(wasm_import_module = "env")]
extern "C" {
    /// Host entrypoint: fetch configuration JSON for the current applet.
    ///
    /// Returns an `i32` pointer to a length-prefixed UTF-8 JSON string representing
    /// `Option<T>` (where `T` is the expected config type). The JSON may be:
    ///
    /// - `null` → interpreted as “no config”; caller will use `T::default()`
    /// - an object/value matching `T` → deserialized into a `Some(T)`
    ///
    /// # Safety
    /// - The returned pointer must reference a valid, host-owned, length-prefixed buffer.
    /// - The caller must retrieve the bytes exactly once via [`read_bytes_from_memory`].
    fn get_config() -> i32;
}

/// A typed, zero-sized handle for retrieving host-provided configuration.
///
/// `Secrets<T>` carries the target type `T` at compile time (via [`PhantomData`]) and
/// provides a simple API to pull and deserialize configuration. If the host returns
/// `null`, [`Secrets::config`] falls back to `T::default()`.
#[derive(Serialize, Deserialize)]
pub struct Secrets<T>(PhantomData<T>);

impl<T: DeserializeOwned + Default> Secrets<T> {
    /// Construct a new, zero-sized secrets handle for type `T`.
    ///
    /// This does not perform any host calls; it only establishes the type parameter.
    pub fn new() -> Self {
        Secrets(PhantomData)
    }

    /// Retrieve and deserialize configuration from the host into `T`.
    ///
    /// The host is expected to return a JSON-encoded `Option<T>`. When the result is:
    ///
    /// - `Some(T)` → that value is returned
    /// - `None`/`null` → `T::default()` is returned
    ///
    /// # Panics
    /// - If reading host memory fails (`unwrap()` on [`read_bytes_from_memory`])
    /// - If JSON decoding into `Option<T>` fails (`unwrap()` on `serde_json::from_str`)
    ///
    /// # Safety
    /// - The pointer returned by [`get_config`] must be a valid length-prefixed buffer.
    /// - We immediately consume it via [`read_bytes_from_memory`], which copies into
    ///   guest memory and frees/handles host state according to the runtime contract.
    pub fn config(&self) -> T {
        // SAFETY: `get_config` is an FFI call that returns a pointer to a host-managed,
        // length-prefixed UTF-8 JSON buffer. The pointer is consumed once by
        // `read_bytes_from_memory`, per the runtime contract.
        let ptr = unsafe { get_config() };
        let serialized_value = read_bytes_from_memory(ptr).unwrap();
        let config = serde_json::from_str::<Option<T>>(&serialized_value).unwrap();

        if let Some(config) = config {
            config
        } else {
            T::default()
        }
    }
}
