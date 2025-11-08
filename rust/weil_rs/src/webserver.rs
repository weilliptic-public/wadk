//! # In-memory chunked file server for Weil applets
//!
//! This module implements a simple, chunk-based file staging and serving layer
//! backed by a [`WeilMap`] and [`WeilMemory`]. It supports a three-step upload
//! flow per path (key):
//!
//! 1. [`WebServer::start_file_upload`] — allocate chunked memory for a path
//! 2. [`WebServer::add_path_content`] — push individual chunks by index
//! 3. [`WebServer::finish_upload`] — mark the file as ready and record total size
//!
//! Once a file is `Ready`, it can be sliced and served via
//! [`WebServer::http_content`] with `GET`/`HEAD`, returning appropriate status,
//! headers (content type inferred via [`mime_guess`]), and body bytes.
//!
//! Paths are normalized to map keys by replacing `/` with `~` (see
//! [`WebServer::path_key`]) to ensure compatibility with map key constraints.

use crate::{
    collections::{
        map::WeilMap,
        memory::{ChunkIndex, WeilMemory},
        WeilId,
    },
    runtime::Runtime,
    traits::WeilType,
};
use mime_guess::MimeGuess;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::Path};

/// Upload/serving state for a file's chunked memory.
#[derive(Serialize, Deserialize)]
enum State {
    /// Chunks are still being written into the underlying [`WeilMemory`].
    UploadInProcess(WeilMemory),
    /// All chunks uploaded; content can be served.
    Ready(WeilMemory),
}

impl WeilType for State {}

/// Metadata and state for a single file path.
#[derive(Serialize, Deserialize)]
struct FileEntry {
    /// Expected number of chunks for this file.
    total_chunks: u32,
    /// Current upload/ready state, carrying the chunked memory.
    state: State,
    /// Original file size in bytes (recorded at `finish_upload`).
    size_bytes: u32, // original size in Bytes
}

impl WeilType for FileEntry {}

/// Minimal HTTP-like file server over chunked storage.
///
/// Uses a [`WeilMap`] keyed by a normalized path (see [`WebServer::path_key`]) to
/// store [`FileEntry`] items. Each entry points to a [`WeilMemory`] holding fixed-size
/// chunks of the file. The server can respond to `GET` and `HEAD` with per-chunk bodies.
#[derive(Serialize, Deserialize)]
pub struct WebServer {
    /// Maps file paths to their entries
    map_obj: WeilMap<String, FileEntry>,
    /// Fixed chunk size in bytes used to allocate [`WeilMemory`].
    chunk_size: u32,
}

impl WeilType for WebServer {}

impl WebServer {
    /// Create a new [`WebServer`] with a backing map identified by `id`.
    ///
    /// If `chunk_size` is `None`, defaults to `16 * 1024` (16 KiB).
    pub fn new(id: WeilId, chunk_size: Option<u32>) -> Self
    where
        Self: Sized,
    {
        WebServer {
            map_obj: WeilMap::new(id),
            chunk_size: if let Some(x) = chunk_size {
                x
            } else {
                16 * 1024
            },
        }
    }

    /// Normalize a user-facing path into a map key by replacing `/` with `~`.
    ///
    /// This avoids conflicts with path separators in key storage.
    fn path_key(path: String) -> String {
        path.as_str().replace("/", "~")
    }

    /// Infer a `Content-Type` header value from a file path using its extension.
    ///
    /// Falls back to `application/octet-stream` when the type cannot be guessed.
    fn get_content_header(file_path: &String) -> String {
        let path = Path::new(file_path);

        let file_extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(String::from)
            .unwrap();

        let mime_type = MimeGuess::from_ext(&file_extension);

        mime_type.first_or_octet_stream().to_string()
    }

    /// Start a new chunked upload for `path`.
    ///
    /// Allocates a [`WeilMemory`] with `total_chunks` and this server’s `chunk_size`,
    /// and stores an entry in the map in `UploadInProcess` state.
    ///
    /// # Errors
    /// Returns a stringified error if [`WeilMemory::with_num_chunks`] fails.
    pub fn start_file_upload(
        &mut self,
        id: WeilId,
        path: String,
        total_chunks: u32,
    ) -> Result<(), String> {
        Runtime::debug_log(&format!("[start_file_upload] path: {}", path));
        let path_key = WebServer::path_key(path);
        Runtime::debug_log(&format!("[start_file_upload] path_key: {}", path_key));

        let memory = WeilMemory::with_num_chunks(id, total_chunks, self.chunk_size as u32)
            .map_err(|err| err.to_string())?;

        self.map_obj.insert(
            path_key,
            FileEntry {
                total_chunks,
                state: State::UploadInProcess(memory),
                size_bytes: 0,
            },
        );

        Ok(())
    }

    /// Get the expected total number of chunks for a file at `path`.
    ///
    /// # Errors
    /// Returns `"path not found"` if no entry exists for the normalized key.
    pub fn total_chunks(&self, path: String) -> Result<u32, String> {
        let path_key = WebServer::path_key(path);

        let Some(entry) = self.map_obj.get(&path_key) else {
            return Err("path not found".to_string());
        };

        Ok(entry.total_chunks)
    }

    /// Upload a single `chunk` at `index` for the file at `path`.
    ///
    /// The file must have been initialized with [`start_file_upload`]. While
    /// uploading, the entry remains in `UploadInProcess` until finalized by
    /// [`finish_upload`].
    ///
    /// # Errors
    /// - Returns an error if the path is unknown (no prior `start_file_upload`).
    /// - Returns an error if the file is already `Ready`.
    /// - Propagates chunk-set errors from [`WeilMemory::set_chunk`] as strings.
    pub fn add_path_content(
        &mut self,
        path: String,
        chunk: Vec<u8>,
        index: u32,
    ) -> Result<(), String> {
        // read the file into a vector of u8
        let path_key = WebServer::path_key(path);

        let Some(entry) = self.map_obj.get(&path_key) else {
            return Err("chunk cannot be uploaded without calling `start_file_upload`".to_string());
        };

        match entry.state {
            State::UploadInProcess(memory) => {
                memory
                    .set_chunk(ChunkIndex(index), chunk)
                    .map_err(|err| err.to_string())?;

                self.map_obj.insert(
                    path_key,
                    FileEntry {
                        total_chunks: entry.total_chunks,
                        state: State::UploadInProcess(memory),
                        size_bytes: 0,
                    },
                );
            }
            State::Ready(_) => return Err("file is already uploaded completely!".to_string()),
        }

        Ok(())
    }

    /// Finalize the upload for `path`, marking it `Ready` and recording `size_bytes`.
    ///
    /// After this call, chunks can be served via [`http_content`].
    ///
    /// # Errors
    /// - Returns an error if the path is unknown.
    /// - Returns an error if the file is already marked `Ready`.
    pub fn finish_upload(&mut self, path: String, size_bytes: u32) -> Result<(), String> {
        let path_key = WebServer::path_key(path);

        let Some(entry) = self.map_obj.get(&path_key) else {
            return Err("chunk cannot be uploaded without calling `start_file_upload`".to_string());
        };

        match entry.state {
            State::UploadInProcess(memory) => {
                self.map_obj.insert(
                    path_key,
                    FileEntry {
                        total_chunks: entry.total_chunks,
                        state: State::Ready(memory),
                        size_bytes,
                    },
                );
            }
            State::Ready(_) => return Err("file is already uploaded completely!".to_string()),
        }

        Ok(())
    }

    /// Serve a chunked HTTP-like response for `path` and `index`, honoring `method`.
    ///
    /// Supported methods:
    /// - `GET`: returns `(status, headers, body)` where `body` is the chunk bytes
    /// - `HEAD`: returns headers only (empty body) if the file exists
    ///
    /// # Status codes
    /// - `200 OK` — on success (`GET` returns the chunk bytes; `HEAD` returns empty body)
    /// - `400 Bad Request` — file still uploading or chunk index out of bounds
    /// - `404 Not Found` — unknown path
    /// - `405 Method Not Allowed` — methods other than `GET`/`HEAD`
    pub fn http_content(
        &self,
        path: String,
        index: u32,
        method: String,
    ) -> (u16, std::collections::HashMap<String, String>, Vec<u8>) {
        match method.as_str() {
            "GET" | "HEAD" => {}
            _ => return (405, HashMap::new(), b"Method Not Allowed".to_vec()),
        }

        let mut headers: HashMap<String, String> = HashMap::new();
        headers.insert("Content-Type".to_string(), Self::get_content_header(&path));

        let path_replaced = WebServer::path_key(path);
        let body = self.map_obj.get(&path_replaced);

        if method == "HEAD" {
            if body.is_none() {
                return (404, HashMap::new(), b"Not Found".to_vec());
            }
            return (200, headers, vec![]);
        }

        if let Some(entry) = body {
            match entry.state {
                State::UploadInProcess(_) => {
                    return (400, HashMap::new(), b"File upload in process".to_vec());
                }
                State::Ready(memory) => {
                    let Some(chunk) = memory.chunk(ChunkIndex(index)) else {
                        return (400, HashMap::new(), b"index out of bounds".to_vec());
                    };

                    return (200, headers, chunk);
                }
            }
        } else {
            return (404, HashMap::new(), b"Not Found".to_vec());
        }
    }

    /// Return the original file size in bytes recorded at `finish_upload`.
    ///
    /// # Errors
    /// Returns `"Not Found"` if `path` does not exist.
    pub fn size_bytes(&self, path: String) -> Result<u32, String> {
        let path_replaced = Self::path_key(path);
        let body = self.map_obj.get(&path_replaced);

        if let Some(entry) = body {
            return Ok(entry.size_bytes);
        } else {
            return Err("Not Found".to_string());
        }
    }

    /// Get the configured chunk size (in bytes) used for new uploads.
    pub fn get_chunk_size(&self) -> u32 {
        self.chunk_size
    }
}
