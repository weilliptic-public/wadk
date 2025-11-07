use crate::{
    mcp::s3::*,
    runtime::{get_length_prefixed_bytes_from_string, read_bytes_from_memory},
};
use base32::{decode, Alphabet};
use serde::{Deserialize, Serialize};

#[link(wasm_import_module = "env")]
extern "C" {
    fn s3_upload(params: i32) -> i32;
    fn s3_download(params: i32) -> i32;
    fn s3_list(params: i32) -> i32;
    fn s3_delete(params: i32) -> i32;
    fn stream_file_download(url_ptr: i32) -> i32;
    fn read_file_chunk(id: i32) -> i32;
    fn stream_file_upload_to_s3(id: i32, params: i32) -> i32;
    fn s3_list_buckets(params: i32) -> i32;
    fn s3_create_bucket(params: i32) -> i32;
    fn s3_delete_bucket(params: i32) -> i32;
    fn s3_get_bucket_location(params: i32) -> i32;
    fn s3_get_bucket_acl(params: i32) -> i32;
    fn s3_get_bucket_versioning(params: i32) -> i32;
    fn s3_set_bucket_versioning(params: i32) -> i32;
}

pub struct S3;

impl S3 {
    /// Uploads a file to S3
    pub fn upload(params: S3UploadParams) -> Result<String, anyhow::Error> {
        let params_json = serde_json::to_string(&params)?;
        let raw_params = get_length_prefixed_bytes_from_string(&params_json, 0);
        let ptr = unsafe { s3_upload(raw_params.as_ptr() as _) };
        let result = read_bytes_from_memory(ptr)?;
        Ok(result)
    }

    /// Downloads a file from S3. Returns the decoded content as a UTF-8 string.
    pub fn download(params: S3DownloadParams) -> Result<String, anyhow::Error> {
        let params_json = serde_json::to_string(&params)?;
        let raw_params = get_length_prefixed_bytes_from_string(&params_json, 0);
        let ptr = unsafe { s3_download(raw_params.as_ptr() as _) };
        let base32_result = read_bytes_from_memory(ptr)?;
        let bytes = decode(Alphabet::Rfc4648Lower { padding: false }, &base32_result)
            .ok_or_else(|| anyhow::anyhow!(String::from("Base32 decoding failed")))?;
        let text = String::from_utf8(bytes)?;
        Ok(text)
    }

    /// Lists objects in an S3 bucket (optionally with prefix). Returns a JSON array of keys.
    pub fn list(params: S3ListParams) -> Result<Vec<String>, anyhow::Error> {
        let params_json = serde_json::to_string(&params)?;
        let raw_params = get_length_prefixed_bytes_from_string(&params_json, 0);
        let ptr = unsafe { s3_list(raw_params.as_ptr() as _) };
        let result = read_bytes_from_memory(ptr)?;
        let keys: Vec<String> = serde_json::from_str(&result)?;
        Ok(keys)
    }

    /// Deletes an object from S3
    pub fn delete(params: S3DeleteParams) -> Result<String, anyhow::Error> {
        let params_json = serde_json::to_string(&params)?;
        let raw_params = get_length_prefixed_bytes_from_string(&params_json, 0);
        let ptr = unsafe { s3_delete(raw_params.as_ptr() as _) };
        let result = read_bytes_from_memory(ptr)?;
        Ok(result)
    }

    /// Initiates a streaming file download from a given URL. Returns a File handle.
    pub fn download_file_stream(url: &str) -> Result<File, anyhow::Error> {
        let url_bytes = get_length_prefixed_bytes_from_string(url, 0);
        let ptr = unsafe { stream_file_download(url_bytes.as_ptr() as _) };
        let result = read_bytes_from_memory(ptr)?;
        let id: u32 = result.parse()?;
        Ok(File(id))
    }

    /// Streams file chunks from a File and uploads to S3 as a single object.
    pub fn upload_file_stream(
        file: &File,
        params: S3UploadParams,
    ) -> Result<String, anyhow::Error> {
        let params_json = serde_json::to_string(&params)?;
        let raw_params = get_length_prefixed_bytes_from_string(&params_json, 0);
        let ptr = unsafe { stream_file_upload_to_s3(file.0 as i32, raw_params.as_ptr() as _) };
        let result = read_bytes_from_memory(ptr)?;
        Ok(result)
    }

    /// List all S3 buckets for the given credentials
    pub fn list_buckets(credentials: S3Credentials) -> Result<Vec<String>, anyhow::Error> {
        let params_json = serde_json::to_string(&credentials)?;
        let raw_params = get_length_prefixed_bytes_from_string(&params_json, 0);
        let ptr = unsafe { s3_list_buckets(raw_params.as_ptr() as _) };
        let result = read_bytes_from_memory(ptr)?;
        let buckets: Vec<String> = serde_json::from_str(&result)?;
        Ok(buckets)
    }

    /// Create a new S3 bucket
    pub fn create_bucket(params: S3CreateBucketParams) -> Result<String, anyhow::Error> {
        let params_json = serde_json::to_string(&params)?;
        let raw_params = get_length_prefixed_bytes_from_string(&params_json, 0);
        let ptr = unsafe { s3_create_bucket(raw_params.as_ptr() as _) };
        let result = read_bytes_from_memory(ptr)?;
        Ok(result)
    }

    /// Delete an S3 bucket
    pub fn delete_bucket(params: S3BucketParams) -> Result<String, anyhow::Error> {
        let params_json = serde_json::to_string(&params)?;
        let raw_params = get_length_prefixed_bytes_from_string(&params_json, 0);
        let ptr = unsafe { s3_delete_bucket(raw_params.as_ptr() as _) };
        let result = read_bytes_from_memory(ptr)?;
        Ok(result)
    }

    /// Get the location of an S3 bucket
    pub fn get_bucket_location(params: S3BucketParams) -> Result<String, anyhow::Error> {
        let params_json = serde_json::to_string(&params)?;
        let raw_params = get_length_prefixed_bytes_from_string(&params_json, 0);
        let ptr = unsafe { s3_get_bucket_location(raw_params.as_ptr() as _) };
        let result = read_bytes_from_memory(ptr)?;
        Ok(result)
    }

    /// Get the ACL of an S3 bucket
    pub fn get_bucket_acl(params: S3BucketParams) -> Result<String, anyhow::Error> {
        let params_json = serde_json::to_string(&params)?;
        let raw_params = get_length_prefixed_bytes_from_string(&params_json, 0);
        let ptr = unsafe { s3_get_bucket_acl(raw_params.as_ptr() as _) };
        let result = read_bytes_from_memory(ptr)?;
        Ok(result)
    }

    /// Get the versioning status of an S3 bucket
    pub fn get_bucket_versioning(params: S3BucketParams) -> Result<String, anyhow::Error> {
        let params_json = serde_json::to_string(&params)?;
        let raw_params = get_length_prefixed_bytes_from_string(&params_json, 0);
        let ptr = unsafe { s3_get_bucket_versioning(raw_params.as_ptr() as _) };
        let result = read_bytes_from_memory(ptr)?;
        Ok(result)
    }

    /// Enable or disable versioning on an S3 bucket
    pub fn set_bucket_versioning(params: S3SetVersioningParams) -> Result<String, anyhow::Error> {
        let params_json = serde_json::to_string(&params)?;
        let raw_params = get_length_prefixed_bytes_from_string(&params_json, 0);
        let ptr = unsafe { s3_set_bucket_versioning(raw_params.as_ptr() as _) };
        let result = read_bytes_from_memory(ptr)?;
        Ok(result)
    }

    /// Uploads text content as a file to S3
    pub fn upload_text(
        credentials: S3Credentials,
        bucket: String,
        key: String,
        text: &str,
    ) -> Result<String, anyhow::Error> {
        let params = S3UploadParams {
            credentials,
            bucket,
            key,
            content: text.as_bytes().to_vec(),
        };
        Self::upload(params)
    }
}

#[link(wasm_import_module = "env")]
extern "C" {
    fn sts_get_session_token(params: i32) -> i32;
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct STSCredentials {
    pub access_key_id: String,
    pub secret_access_key: String,
    pub region: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct STSSessionTokenResponse {
    pub session_token: String,
    pub access_key_id: String,
    pub secret_access_key: String,
    pub expiration: String, // ISO8601 timestamp
}

pub struct STS;

impl STS {
    /// Gets a session token from AWS STS
    pub fn get_session_token(
        params: STSCredentials,
    ) -> Result<STSSessionTokenResponse, anyhow::Error> {
        let params_json = serde_json::to_string(&params)?;
        let raw_params = get_length_prefixed_bytes_from_string(&params_json, 0);
        let ptr = unsafe { sts_get_session_token(raw_params.as_ptr() as _) };
        let result = read_bytes_from_memory(ptr)?;
        let response: STSSessionTokenResponse = serde_json::from_str(&result)?;
        Ok(response)
    }
}

/// A handle to a streaming file download/upload, wrapping a platform-side file stream id.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct File(pub u32);

impl File {
    /// Reads the next chunk from the file stream. Returns None if the stream is finished.
    pub fn read_chunk(&self) -> Result<Option<Vec<u8>>, anyhow::Error> {
        let ptr = unsafe { read_file_chunk(self.0 as i32) };
        let result = read_bytes_from_memory(ptr)?;
        // The platform returns a JSON Option<FileChunk> where FileChunk { data: Vec<u8>, is_last: bool }
        #[derive(Deserialize)]
        struct FileChunk {
            data: Vec<u8>,
            is_last: bool,
        }
        let opt_chunk: Option<FileChunk> = serde_json::from_str(&result)?;
        match opt_chunk {
            Some(chunk) if !chunk.is_last => Ok(Some(chunk.data)),
            Some(chunk) if chunk.is_last && !chunk.data.is_empty() => Ok(Some(chunk.data)),
            _ => Ok(None),
        }
    }
}
