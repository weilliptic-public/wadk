use serde::{Deserialize, Serialize};

/// Credentials for authenticating against Amazon S3-compatible services.
///
/// ## Notes
/// - Treat all fields as **secrets** except `region`; avoid logging them.
/// - `session_token` is required for temporary credentials (e.g., STS).
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct S3Credentials {
    /// Access key ID for the IAM user/role.
    pub access_key_id: String,
    /// Secret access key paired with the access key ID.
    pub secret_access_key: String,
    /// AWS region of the target S3 service (e.g., `"us-east-1"`).
    pub region: String,
    /// Optional session token (for temporary credentials issued by STS).
    pub session_token: Option<String>,
}

/// Parameters to upload an object to S3.
///
/// Uploads the raw `content` bytes to `s3://{bucket}/{key}` using the given
/// `credentials`.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct S3UploadParams {
    /// Authentication material used for the request.
    pub credentials: S3Credentials,
    /// Name of the destination bucket.
    pub bucket: String,
    /// Object key (path) within the bucket.
    pub key: String,
    /// Raw object payload to upload.
    pub content: Vec<u8>,
}

/// Parameters to download an object from S3.
///
/// Identifies a single object by `bucket` + `key`.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct S3DownloadParams {
    /// Authentication material used for the request.
    pub credentials: S3Credentials,
    /// Name of the source bucket.
    pub bucket: String,
    /// Object key (path) within the bucket.
    pub key: String,
}

/// Parameters to list objects in S3.
///
/// Optionally restricts results to a `prefix` under the specified `bucket`.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct S3ListParams {
    /// Authentication material used for the request.
    pub credentials: S3Credentials,
    /// Name of the bucket to list.
    pub bucket: String,
    /// Optional key prefix to filter results (e.g., `"inbox/2025/"`).
    pub prefix: Option<String>,
}

/// Parameters to delete an object from S3.
///
/// Targets a single object by `bucket` + `key`.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct S3DeleteParams {
    /// Authentication material used for the request.
    pub credentials: S3Credentials,
    /// Name of the bucket containing the object.
    pub bucket: String,
    /// Object key (path) to delete.
    pub key: String,
}

/// Parameters for bucket-scoped operations (no object key).
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct S3BucketParams {
    /// Authentication material used for the request.
    pub credentials: S3Credentials,
    /// Target bucket name.
    pub bucket: String,
}

/// Parameters to create a new S3 bucket.
///
/// The `region` field allows overriding the region used for bucket creation
/// when it differs from the credentials' default region.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct S3CreateBucketParams {
    /// Authentication material used for the request.
    pub credentials: S3Credentials,
    /// Name of the bucket to create.
    pub bucket: String,
    /// Optional region for the new bucket (e.g., `"us-west-2"`).
    pub region: Option<String>,
}

/// Parameters to enable or disable S3 bucket versioning.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct S3SetVersioningParams {
    /// Authentication material used for the request.
    pub credentials: S3Credentials,
    /// Target bucket name.
    pub bucket: String,
    /// `true` to enable versioning; `false` to suspend it.
    pub enabled: bool,
}

/// Long-lived STS base credentials (without session token).
///
/// Typically used to request temporary session credentials.
#[derive(Debug, Serialize, Deserialize)]
pub struct STSCredentials {
    /// Access key ID.
    pub access_key_id: String,
    /// Secret access key.
    pub secret_access_key: String,
    /// AWS region used for STS calls (e.g., `"us-east-1"`).
    pub region: String,
}

/// Response payload containing **temporary** session credentials from STS.
///
/// These values are time-limited and should be refreshed before `expiration`.
#[derive(Debug, Serialize, Deserialize)]
pub struct STSSessionTokenResponse {
    /// Temporary session token to be used alongside the keys.
    pub session_token: String,
    /// Temporary access key ID.
    pub access_key_id: String,
    /// Temporary secret access key.
    pub secret_access_key: String,
    /// Expiration timestamp (ISO-8601 string as returned by STS).
    pub expiration: String,
}
