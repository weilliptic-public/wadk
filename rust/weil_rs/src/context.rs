//! # Context → VectorDB bridge (WASM FFI)
//!
//! This module exposes helpers to:
//! - Pull chunked context (texts + embeddings) prepared by the host runtime
//! - Upload applet-context chunks directly into a VectorDB
//! - Upload arbitrary text to a VectorDB
//! - Ask the host to generate embeddings for provided chunks and persist them
//!
//! All host bindings live behind `wasm_import_module="env"` and communicate using
//! **length-prefixed UTF-8** byte buffers. Callers must read the returned pointers
//! exactly once via [`read_bytes_from_memory`] to obtain the serialized JSON.
//!
//! ## Data contracts
//! - [`get_chunks`] returns a JSON object with keys:
//!   - `"embeddings"`: `number[][]` (each inner array is one embedding vector)
//!   - `"texts"`: `string[]` (aligned by index with `embeddings`)
//! - Upload/execute functions return a status buffer; the body is read solely to
//!   surface any host-side error as `anyhow::Error`.
//!
//! ## Safety
//! All FFI calls are wrapped in `unsafe` blocks and immediately followed by
//! [`read_bytes_from_memory`]. The host guarantees that returned pointers reference
//! a valid, length-prefixed buffer readable once.

use crate::runtime::{get_length_prefixed_bytes_from_string, read_bytes_from_memory};
use serde_json::Value;
use std::collections::BTreeMap;

#[link(wasm_import_module = "env")]
extern "C" {
    /// Host: return prepared context chunks (texts + embeddings).
    ///
    /// Returns a pointer to a length-prefixed UTF-8 JSON object with fields:
    /// `{ "embeddings": number[][], "texts": string[] }`.
    ///
    /// # Safety
    /// - Returned pointer must reference a valid length-prefixed buffer.
    /// - Caller must consume the buffer exactly once via [`read_bytes_from_memory`].
    fn get_chunks() -> i32;

    /// Host: upload context chunks already associated with `applet_id` into VectorDB `vdb_id`.
    ///
    /// - `vdb_id`: length-prefixed UTF-8 string
    /// - `applet_id`: length-prefixed UTF-8 string
    ///
    /// Returns a pointer to a status buffer (read to detect errors).
    ///
    /// # Safety
    /// - Both pointers must reference valid length-prefixed buffers.
    /// - Return pointer must be consumed via [`read_bytes_from_memory`].
    fn upload_chunks_from_context(vdb_id: i32, applet_id: i32) -> i32;

    /// Host: upload a raw `text` document to VectorDB `vdb_id`.
    ///
    /// - `vdb_id`: length-prefixed UTF-8 string
    /// - `text`: length-prefixed UTF-8 string
    ///
    /// Returns a pointer to a status buffer (read to detect errors).
    ///
    /// # Safety
    /// - Both pointers must reference valid length-prefixed buffers.
    /// - Return pointer must be consumed via [`read_bytes_from_memory`].
    fn upload_text_to_vdb(vdb_id: i32, text: i32) -> i32;

    /// Host: generate embeddings for `chunks` and persist them in VectorDB `vdb_id`.
    ///
    /// - `vdb_id`: length-prefixed UTF-8 string
    /// - `chunks`: length-prefixed UTF-8 JSON `string[]`
    ///
    /// Returns a pointer to a status buffer (read to detect errors).
    ///
    /// # Safety
    /// - Both pointers must reference valid length-prefixed buffers.
    /// - Return pointer must be consumed via [`read_bytes_from_memory`].
    fn generate_embeddings_and_save(vdb_id: i32, chunks: i32) -> i32;
}

/// Context façade for interacting with host-provided chunking/VectorDB services.
///
/// This type is a zero-sized namespace; all methods are static.
pub struct Context;

impl Context {
    /// Retrieve precomputed context chunks from the host.
    ///
    /// # Returns
    /// A tuple `(embeddings, texts)` where:
    /// - `embeddings: Vec<Vec<f32>>` — list of embedding vectors
    /// - `texts: Vec<String>` — matching textual chunks (aligned by index)
    ///
    /// Any missing or malformed fields are treated as empty via `unwrap_or_default()`.
    ///
    /// # Panics
    /// - If reading host memory fails (`unwrap()` on [`read_bytes_from_memory`])
    /// - If JSON decoding fails (`unwrap()`/`from_str`)
    pub fn get_chunks() -> (Vec<Vec<f32>>, Vec<String>) {
        // SAFETY: `get_chunks` returns a pointer to a length-prefixed JSON object.
        // We immediately copy it into guest memory and parse.
        let ptr = unsafe { get_chunks() };

        let serialized_value = read_bytes_from_memory(ptr).unwrap();
        let chunk_data: BTreeMap<String, Value> = serde_json::from_str(&serialized_value).unwrap();

        let embeddings = chunk_data
            .get("embeddings")
            .and_then(|v| v.as_array())
            .and_then(|arr| {
                arr.iter()
                    .map(|embedding_value| {
                        embedding_value.as_array().and_then(|embedding_arr| {
                            embedding_arr
                                .iter()
                                .map(|num| num.as_f64().map(|f| f as f32))
                                .collect::<Option<Vec<f32>>>()
                        })
                    })
                    .collect::<Option<Vec<Vec<f32>>>>()
            })
            .unwrap_or_default();

        let texts = chunk_data
            .get("texts")
            .and_then(|v| serde_json::from_value::<Vec<String>>(v.clone()).ok())
            .unwrap_or_default();

        (embeddings, texts)
    }

    /// Upload host-prepared context chunks (for `applet_id`) into VectorDB `vdb_id`.
    ///
    /// This uses the host’s internal association of `(applet_id → chunks)` and stores
    /// them under the provided VectorDB identifier.
    ///
    /// # Errors
    /// Propagates any host-side error surfaced via the returned status buffer.
    pub fn upload_chunks_from_context(
        vdb_id: String,
        applet_id: String,
    ) -> Result<(), anyhow::Error> {
        let vdb_id_bytes = get_length_prefixed_bytes_from_string(&vdb_id, 0);
        let serialized_applet_id = get_length_prefixed_bytes_from_string(&applet_id, 0);

        // SAFETY: Both buffers are valid length-prefixed UTF-8 strings.
        let ptr = unsafe {
            upload_chunks_from_context(
                vdb_id_bytes.as_ptr() as _,
                serialized_applet_id.as_ptr() as _,
            )
        };

        // Read the result to check for any errors
        read_bytes_from_memory(ptr)?;

        Ok(())
    }

    /// Upload a single raw `text` document to VectorDB `vdb_id`.
    ///
    /// # Errors
    /// Propagates any host-side error surfaced via the returned status buffer.
    pub fn upload_text_to_vdb(vdb_id: String, text: String) -> Result<(), anyhow::Error> {
        let vdb_id_bytes = get_length_prefixed_bytes_from_string(&vdb_id, 0);
        let text_bytes = get_length_prefixed_bytes_from_string(&text, 0);

        // SAFETY: Both buffers are valid length-prefixed UTF-8 strings.
        let ptr =
            unsafe { upload_text_to_vdb(vdb_id_bytes.as_ptr() as _, text_bytes.as_ptr() as _) };

        // Read the result to check for any errors
        read_bytes_from_memory(ptr)?;

        Ok(())
    }

    /// Ask the host to embed `chunks` and persist them in VectorDB `vdb_id`.
    ///
    /// The embedding model and persistence details are host-defined. This function only
    /// performs serialization and error surface.
    ///
    /// # Errors
    /// Propagates any host-side error surfaced via the returned status buffer, or JSON
    /// serialization errors for `chunks`.
    pub fn generate_embeddings_and_save(
        vdb_id: String,
        chunks: Vec<String>,
    ) -> Result<(), anyhow::Error> {
        let vdb_id_bytes = get_length_prefixed_bytes_from_string(&vdb_id, 0);
        let chunks_bytes =
            get_length_prefixed_bytes_from_string(&serde_json::to_string(&chunks)?, 0);

        // SAFETY: Both buffers are valid length-prefixed strings (`chunks_bytes` is JSON stringified).
        let ptr = unsafe {
            generate_embeddings_and_save(vdb_id_bytes.as_ptr() as _, chunks_bytes.as_ptr() as _)
        };

        // Read the result to check for any errors
        read_bytes_from_memory(ptr)?;

        Ok(())
    }
}
