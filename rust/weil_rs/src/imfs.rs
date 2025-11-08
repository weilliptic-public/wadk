use crate::runtime::{get_length_prefixed_bytes_from_string, read_bytes_from_memory};
use base32::{decode, encode, Alphabet};
use serde::{Deserialize, Serialize};
use wadk_utils::errors::WeilError;

// Platform functions for IMFS operations
#[link(wasm_import_module = "env")]
extern "C" {
    // Store a file in the in-memory filesystem
    // Returns a pointer to a JSON result string
    fn store_file(filepath: i32, content: i32, mime_type: i32) -> i32;

    // Retrieve a file from the in-memory filesystem
    // Returns a pointer to a JSON string containing file content and metadata
    fn get_file(filepath: i32) -> i32;
}

/// Metadata for files stored in the IMFS
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    /// MIME type of the file
    pub mime_type: Option<String>,
    /// Size of the file in bytes
    pub size: usize,
    /// Timestamp when the file was created (Unix timestamp)
    pub created_at: u64,
    /// Timestamp when the file was last modified (Unix timestamp)
    pub modified_at: u64,
}

/// File content with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileContent {
    /// Binary content of the file (base64 encoded)
    pub content: String,
    /// File metadata
    pub metadata: FileMetadata,
}

/// Response structure for get_file operation
#[derive(Debug, Clone, Serialize, Deserialize)]
struct GetFileResponse {
    content: String,
    metadata: FileMetadata,
}

/// In-Memory File System client interface
pub struct IMFS;

impl IMFS {
    /// Store a file in the in-memory filesystem
    ///
    /// # Arguments
    /// * `filepath` - Path to the file
    /// * `content` - Binary content of the file (will be base64 encoded)
    /// * `mime_type` - Optional MIME type of the file
    ///
    /// # Returns
    /// Result indicating success or failure
    pub fn store_file(
        filepath: String,
        content: Vec<u8>,
        mime_type: Option<String>,
    ) -> Result<String, WeilError> {
        // Encode content as base32
        let content_base32 = encode(Alphabet::Rfc4648Lower { padding: false }, &content);

        // Prepare parameters for platform call
        let filepath_bytes = get_length_prefixed_bytes_from_string(&filepath, 0);
        let content_bytes = get_length_prefixed_bytes_from_string(&content_base32, 0);
        let mime_type_bytes = if let Some(mime) = &mime_type {
            get_length_prefixed_bytes_from_string(mime, 0)
        } else {
            vec![0u8; 5] // Empty string with length prefix
        };

        // Call platform function
        let result_ptr = unsafe {
            store_file(
                filepath_bytes.as_ptr() as i32,
                content_bytes.as_ptr() as i32,
                mime_type_bytes.as_ptr() as i32,
            )
        };

        // Read result from memory
        let result = read_bytes_from_memory(result_ptr)?;
        Ok(result)

    }

    /// Retrieve a file from the in-memory filesystem
    ///
    /// # Arguments
    /// * `filepath` - Path to the file
    ///
    /// # Returns
    /// Option containing the file content and metadata if found
    pub fn get_file(filepath: String) -> Result<Option<FileContent>, WeilError> {
        // Prepare parameter for platform call
        let filepath_bytes = get_length_prefixed_bytes_from_string(&filepath, 0);

        // Call platform function
        let result_ptr = unsafe { get_file(filepath_bytes.as_ptr() as i32) };

        // Read result from memory
        let result = read_bytes_from_memory(result_ptr)?;

        // Parse result
        match serde_json::from_str::<serde_json::Value>(&result) {
            Ok(json_result) => {
                if let Some(error) = json_result.get("error") {
                    if error.as_str() == Some("File not found") {
                        Ok(None)
                    } else {
                        Err(WeilError::PlatformError(
                            error.as_str().unwrap_or("Unknown error").to_string(),
                        ))
                    }
                } else {
                    // Parse successful response
                    match serde_json::from_str::<GetFileResponse>(&result) {
                        Ok(response) => {
                            let file_content = FileContent {
                                content: response.content,
                                metadata: response.metadata,
                            };
                            Ok(Some(file_content))
                        }
                        Err(_) => Err(WeilError::PlatformError(
                            "Failed to parse file response".to_string(),
                        )),
                    }
                }
            }
            Err(_) => Err(WeilError::PlatformError(
                "Failed to parse platform response".to_string(),
            )),
        }
    }

    /// Check if a file exists
    ///
    /// # Arguments
    /// * `filepath` - Path to the file
    ///
    /// # Returns
    /// True if the file exists, false otherwise
    pub fn file_exists(filepath: String) -> Result<bool, WeilError> {
        match Self::get_file(filepath) {
            Ok(Some(_)) => Ok(true),
            Ok(None) => Ok(false),
            Err(_) => Ok(false), // Treat errors as file not existing
        }
    }

    /// Get the content of a file as bytes (decodes base64)
    ///
    /// # Arguments
    /// * `filepath` - Path to the file
    ///
    /// # Returns
    /// Option containing the file content as bytes if found
    pub fn get_file_bytes(filepath: String) -> Result<Option<Vec<u8>>, WeilError> {
        match Self::get_file(filepath) {
            Ok(Some(file_content)) => {
                match decode(
                    Alphabet::Rfc4648Lower { padding: false },
                    &file_content.content,
                ) {
                    Some(bytes) => Ok(Some(bytes)),
                    None => Err(WeilError::PlatformError(
                        "Failed to decode base32 content".to_string(),
                    )),
                }
            }
            Ok(None) => Ok(None),
            Err(err) => Err(err),
        }
    }
}
