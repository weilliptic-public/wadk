use serde::{Deserialize, Serialize};
use weil_macros::{WeilType, constructor, query, smart_contract};
use weil_rs::collections::WeilId;
use weil_rs::config::Secrets;
use weil_rs::runtime::Runtime;
use weil_rs::s3::S3;
use weil_rs::webserver::WebServer;

use weil_rs::mcp::s3::{
    S3BucketParams, S3CreateBucketParams, S3Credentials, S3DeleteParams, S3DownloadParams,
    S3ListParams, S3SetVersioningParams, S3UploadParams,
};

mod url;
use url::shorten_url;

/// Configuration for AWS S3 credentials and region.
#[derive(Debug, Serialize, Deserialize, WeilType, Default, Clone)]
pub struct S3Config {
    /// AWS access key ID for authentication
    access_key_id: String,
    /// AWS secret access key for authentication
    secret_access_key: String,
    /// AWS region where the S3 bucket is located
    region: String,
}

/// Information about an S3 object.
#[derive(Debug, Serialize, Deserialize)]
pub struct S3ObjectInfo {
    /// The object key (file path) in the bucket
    key: String,
    /// Size of the object in bytes
    size: u64,
    /// Last modified timestamp of the object
    last_modified: String,
}

/// Information about an uploaded file.
#[derive(Debug, Serialize, Deserialize)]
pub struct UploadedFileInfo {
    /// Path to the uploaded file
    file_path: String,
    /// Size of the file in bytes
    size_bytes: u32,
    /// Total number of chunks the file was split into
    total_chunks: u32,
}

/// Trait defining AWS S3 operations for object storage.
trait AwsS3 {
    /// Creates a new AWS S3 contract instance.
    fn new() -> Result<Self, String>
    where
        Self: Sized;
    /// Uploads binary data to an S3 bucket.
    async fn upload(&self, bucket: String, key: String, data: Vec<u8>) -> Result<(), String>;
    /// Uploads a file from memory to an S3 bucket.
    async fn upload_from_memory(
        &self,
        bucket: String,
        key: String,
        filepath: String,
    ) -> Result<(), String>;
    /// Downloads an object from an S3 bucket.
    async fn download(&self, bucket: String, key: String) -> Result<String, String>;
    /// Lists objects in an S3 bucket with optional prefix filter.
    async fn list_objects(
        &self,
        bucket: String,
        prefix: Option<String>,
    ) -> Result<Vec<S3ObjectInfo>, String>;
    /// Deletes an object from an S3 bucket.
    async fn delete(&self, bucket: String, key: String) -> Result<(), String>;
    /// Downloads content from an external URL and uploads it to S3.
    async fn upload_external_url_to_s3(
        &self,
        bucket: String,
        key: String,
        url: String,
    ) -> Result<String, String>;
    /// Lists all S3 buckets accessible to the configured credentials.
    async fn list_buckets(&self) -> Result<Vec<String>, String>;
    /// Creates a new S3 bucket.
    async fn create_bucket(&self, bucket: String) -> Result<String, String>;
    /// Deletes an S3 bucket.
    async fn delete_bucket(&self, bucket: String) -> Result<String, String>;
    /// Gets the location constraint of an S3 bucket.
    async fn get_bucket_location(&self, bucket: String) -> Result<String, String>;
    /// Gets the access control list (ACL) of an S3 bucket.
    async fn get_bucket_acl(&self, bucket: String) -> Result<String, String>;
    /// Gets the versioning status of an S3 bucket.
    async fn get_bucket_versioning(&self, bucket: String) -> Result<String, String>;
    /// Sets the versioning status of an S3 bucket.
    async fn set_bucket_versioning(&self, bucket: String, enabled: bool) -> Result<String, String>;
    /// Uploads text content to an S3 bucket.
    async fn upload_text(
        &self,
        bucket: String,
        key: String,
        text: String,
    ) -> Result<String, String>;
    /// Uploads a file from a file descriptor to an S3 bucket.
    async fn upload_from_file(
        &self,
        bucket: String,
        key: String,
        file_descriptor: String,
    ) -> Result<String, String>;
    /// Returns the JSON schema defining available tools for the S3 contract.
    fn tools(&self) -> String;
    /// Returns the JSON schema defining available prompts for the S3 contract.
    fn prompts(&self) -> String;
}

/// Contract state for AWS S3 operations.
#[derive(Serialize, Deserialize, WeilType)]
pub struct AwsS3ContractState {
    /// S3 configuration secrets (credentials and region)
    secrets: Secrets<S3Config>,
    /// Web server instance (we dont use it not but just incase we want to store files like static assets)
    web_server: WebServer,
    /// Simple list of uploaded file paths
    uploaded_file_paths: Vec<String>,
}

#[smart_contract]
impl AwsS3 for AwsS3ContractState {
    /// Creates a new AWS S3 contract instance.
    ///
    /// # Returns
    ///
    /// * `Ok(AwsS3ContractState)` - Successfully created contract state with initialized secrets and web server.
    /// * `Err(String)` - Error message if creation fails.
    #[constructor]
    fn new() -> Result<Self, String>
    where
        Self: Sized,
    {
        Ok(AwsS3ContractState {
            secrets: Secrets::new(),
            web_server: WebServer::new(WeilId(1), Some(16 * 1024)), // 16KB chunks
            uploaded_file_paths: Vec::new(),
        })
    }

    /// Uploads binary data to an S3 bucket.
    ///
    /// # Arguments
    ///
    /// * `bucket` - The name of the S3 bucket.
    /// * `key` - The object key (file path) in the bucket.
    /// * `data` - The binary data to upload.
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Successfully uploaded the data.
    /// * `Err(String)` - Error message if the upload fails.

    /// we dont use it as a tool because llm cant send vec<u8> args
    #[query]
    async fn upload(&self, bucket: String, key: String, data: Vec<u8>) -> Result<(), String> {
        self.upload_data_to_s3(bucket, key, data).await
    }

    /// Uploads a file from memory to an S3 bucket.
    ///
    /// # Arguments
    ///
    /// * `bucket` - The name of the S3 bucket.
    /// * `key` - The object key (file path) in the bucket.
    /// * `file_path` - The path to the file in memory.
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Successfully uploaded the file.
    /// * `Err(String)` - Error message if the upload fails.

    // we don't use it as a tool because we dont have stuff residing in the weilmemory for s3 applet
    #[query]
    async fn upload_from_memory(
        &self,
        bucket: String,
        key: String,
        file_path: String,
    ) -> Result<(), String> {
        // Get file data from WeilMemory
        let file_data = self.get_file_from_weilmemory(file_path.clone())?;

        // Upload to S3
        self.upload_data_to_s3(bucket, key, file_data).await?;

        // Should we remove from memory after successful upload?

        Ok(())
    }

    /// Downloads an object from an S3 bucket.
    ///
    /// # Arguments
    ///
    /// * `bucket` - The name of the S3 bucket.
    /// * `key` - The object key (file path) in the bucket.
    ///
    /// # Returns
    ///
    /// * `Ok(String)` - The content of the downloaded object.
    /// * `Err(String)` - Error message if the download fails.
    #[query]
    async fn download(&self, bucket: String, key: String) -> Result<String, String> {
        let config = self.secrets.config();
        let credentials = S3Credentials {
            access_key_id: config.access_key_id.clone(),
            secret_access_key: config.secret_access_key.clone(),
            region: config.region.clone(),
            session_token: None,
        };
        let params = S3DownloadParams {
            credentials,
            bucket,
            key,
        };
        S3::download(params).map_err(|e| e.to_string())
    }

    /// Lists objects in an S3 bucket with optional prefix filter.
    ///
    /// # Arguments
    ///
    /// * `bucket` - The name of the S3 bucket.
    /// * `prefix` - Optional prefix to filter objects by key.
    ///
    /// # Returns
    ///
    /// * `Ok(Vec<S3ObjectInfo>)` - List of objects matching the criteria.
    /// * `Err(String)` - Error message if the listing fails.
    #[query]
    async fn list_objects(
        &self,
        bucket: String,
        prefix: Option<String>,
    ) -> Result<Vec<S3ObjectInfo>, String> {
        let config = self.secrets.config();
        let credentials = S3Credentials {
            access_key_id: config.access_key_id.clone(),
            secret_access_key: config.secret_access_key.clone(),
            region: config.region.clone(),
            session_token: None,
        };
        let params = S3ListParams {
            credentials,
            bucket,
            prefix,
        };
        let keys = S3::list(params).map_err(|e| e.to_string())?;
        Ok(keys
            .into_iter()
            .map(|key| S3ObjectInfo {
                key,
                size: 0, // Note: S3::list doesn't provide size info, would need separate API call
                last_modified: "".to_string(),
            })
            .collect())
    }

    /// Deletes an object from an S3 bucket.
    ///
    /// # Arguments
    ///
    /// * `bucket` - The name of the S3 bucket.
    /// * `key` - The object key (file path) in the bucket.
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Successfully deleted the object.
    /// * `Err(String)` - Error message if the deletion fails.
    #[query]
    async fn delete(&self, bucket: String, key: String) -> Result<(), String> {
        let config = self.secrets.config();
        let credentials = S3Credentials {
            access_key_id: config.access_key_id.clone(),
            secret_access_key: config.secret_access_key.clone(),
            region: config.region.clone(),
            session_token: None,
        };
        let params = S3DeleteParams {
            credentials,
            bucket,
            key,
        };
        S3::delete(params).map(|_| ()).map_err(|e| e.to_string())
    }

    /// Downloads content from an external URL and uploads it to S3.
    ///
    /// # Arguments
    ///
    /// * `bucket` - The name of the S3 bucket to upload to.
    /// * `key` - The object key (file path) in the bucket.
    /// * `url` - The external URL to download content from.
    ///
    /// # Returns
    ///
    /// * `Ok(String)` - The shortened URL of the uploaded object.
    /// * `Err(String)` - Error message if the operation fails.
    #[query]
    async fn upload_external_url_to_s3(
        &self,
        bucket: String,
        key: String,
        url: String,
    ) -> Result<String, String> {
        let result = self.upload_file_from_url(bucket, key, url).await?;
        // Try to shorten the URL, if it fails, return the original
        match shorten_url(&result) {
            Ok(shortened) => Ok(shortened),
            Err(_) => Ok(result),
        }
    }

    /// Lists all S3 buckets accessible to the configured credentials.
    ///
    /// # Returns
    ///
    /// * `Ok(Vec<String>)` - List of bucket names.
    /// * `Err(String)` - Error message if the listing fails.
    #[query]
    async fn list_buckets(&self) -> Result<Vec<String>, String> {
        let config = self.secrets.config();
        let creds = S3Credentials {
            access_key_id: config.access_key_id,
            secret_access_key: config.secret_access_key,
            region: config.region,
            session_token: None,
        };
        weil_rs::s3::S3::list_buckets(creds).map_err(|e| e.to_string())
    }

    /// Creates a new S3 bucket.
    ///
    /// # Arguments
    ///
    /// * `bucket` - The name of the bucket to create.
    ///
    /// # Returns
    ///
    /// * `Ok(String)` - Success message indicating the bucket was created.
    /// * `Err(String)` - Error message if the creation fails.
    #[query]
    async fn create_bucket(&self, bucket: String) -> Result<String, String> {
        let config = self.secrets.config();

        let creds = S3Credentials {
            access_key_id: config.access_key_id,
            secret_access_key: config.secret_access_key,
            region: config.region,
            session_token: None,
        };

        let sdk_params = S3CreateBucketParams {
            credentials: creds,
            bucket,
            region: None,
        };
        weil_rs::s3::S3::create_bucket(sdk_params).map_err(|e| e.to_string())
    }

    #[query]
    async fn delete_bucket(&self, bucket: String) -> Result<String, String> {
        let config = self.secrets.config();

        let creds = S3Credentials {
            access_key_id: config.access_key_id,
            secret_access_key: config.secret_access_key,
            region: config.region,
            session_token: None,
        };

        let sdk_params = S3BucketParams {
            credentials: creds,
            bucket,
        };
        weil_rs::s3::S3::delete_bucket(sdk_params).map_err(|e| e.to_string())
    }

    /// Gets the location constraint of an S3 bucket.
    ///
    /// # Arguments
    ///
    /// * `bucket` - The name of the bucket.
    ///
    /// # Returns
    ///
    /// * `Ok(String)` - The location constraint of the bucket.
    /// * `Err(String)` - Error message if the operation fails.
    #[query]
    async fn get_bucket_location(&self, bucket: String) -> Result<String, String> {
        let config = self.secrets.config();

        let creds = S3Credentials {
            access_key_id: config.access_key_id,
            secret_access_key: config.secret_access_key,
            region: config.region,
            session_token: None,
        };

        let sdk_params = S3BucketParams {
            credentials: creds,
            bucket,
        };
        weil_rs::s3::S3::get_bucket_location(sdk_params).map_err(|e| e.to_string())
    }

    /// Gets the access control list (ACL) of an S3 bucket.
    ///
    /// # Arguments
    ///
    /// * `bucket` - The name of the bucket.
    ///
    /// # Returns
    ///
    /// * `Ok(String)` - The ACL information of the bucket.
    /// * `Err(String)` - Error message if the operation fails.
    #[query]
    async fn get_bucket_acl(&self, bucket: String) -> Result<String, String> {
        let config = self.secrets.config();

        let creds = S3Credentials {
            access_key_id: config.access_key_id,
            secret_access_key: config.secret_access_key,
            region: config.region,
            session_token: None,
        };

        let sdk_params = S3BucketParams {
            credentials: creds,
            bucket,
        };
        weil_rs::s3::S3::get_bucket_acl(sdk_params).map_err(|e| e.to_string())
    }

    /// Gets the versioning status of an S3 bucket.
    ///
    /// # Arguments
    ///
    /// * `bucket` - The name of the bucket.
    ///
    /// # Returns
    ///
    /// * `Ok(String)` - The versioning status of the bucket.
    /// * `Err(String)` - Error message if the operation fails.
    #[query]
    async fn get_bucket_versioning(&self, bucket: String) -> Result<String, String> {
        let config = self.secrets.config();

        let creds = S3Credentials {
            access_key_id: config.access_key_id,
            secret_access_key: config.secret_access_key,
            region: config.region,
            session_token: None,
        };

        let sdk_params = S3BucketParams {
            credentials: creds,
            bucket,
        };
        weil_rs::s3::S3::get_bucket_versioning(sdk_params).map_err(|e| e.to_string())
    }

    /// Sets the versioning status of an S3 bucket.
    ///
    /// # Arguments
    ///
    /// * `bucket` - The name of the bucket.
    /// * `enabled` - Whether to enable versioning on the bucket.
    ///
    /// # Returns
    ///
    /// * `Ok(String)` - Success message indicating the versioning status was updated.
    /// * `Err(String)` - Error message if the operation fails.
    #[query]
    async fn set_bucket_versioning(&self, bucket: String, enabled: bool) -> Result<String, String> {
        let config = self.secrets.config();

        let creds = S3Credentials {
            access_key_id: config.access_key_id,
            secret_access_key: config.secret_access_key,
            region: config.region,
            session_token: None,
        };

        let sdk_params = S3SetVersioningParams {
            credentials: creds,
            bucket,
            enabled,
        };
        weil_rs::s3::S3::set_bucket_versioning(sdk_params).map_err(|e| e.to_string())
    }

    /// Uploads text content to an S3 bucket.
    ///
    /// # Arguments
    ///
    /// * `bucket` - The name of the S3 bucket.
    /// * `key` - The object key (file path) in the bucket.
    /// * `text` - The text content to upload.
    ///
    /// # Returns
    ///
    /// * `Ok(String)` - The shortened URL of the uploaded object.
    /// * `Err(String)` - Error message if the upload fails.
    #[query]
    async fn upload_text(
        &self,
        bucket: String,
        key: String,
        text: String,
    ) -> Result<String, String> {
        let config = self.secrets.config();
        let creds = S3Credentials {
            access_key_id: config.access_key_id,
            secret_access_key: config.secret_access_key,
            region: config.region,
            session_token: None,
        };
        let result =
            weil_rs::s3::S3::upload_text(creds, bucket, key, &text).map_err(|e| e.to_string())?;

        // Try to shorten the URL, if it fails, return the original
        match shorten_url(&result) {
            Ok(shortened) => Ok(shortened),
            Err(_) => Ok(result),
        }
    }

    /// Uploads a file from a file descriptor to an S3 bucket.
    ///
    /// This method reads a file from the IMFS (In-Memory File System) using a file descriptor
    /// and uploads its content to S3. The file descriptor is obtained from the IMFS contract.
    ///
    /// # Arguments
    ///
    /// * `bucket` - The name of the S3 bucket.
    /// * `key` - The object key (file path) in the bucket.
    /// * `file_descriptor` - The base32-encoded file descriptor from IMFS.
    ///
    /// # Returns
    ///
    /// * `Ok(String)` - The shortened URL of the uploaded object.
    /// * `Err(String)` - Error message if the upload fails.
    #[query]
    async fn upload_from_file(
        &self,
        bucket: String,
        key: String,
        file_descriptor: String,
    ) -> Result<String, String> {
        // parameter definition for the cross contract call to imfs
        #[derive(Serialize, Deserialize)]
        struct Args {
            file_descriptor: String,
        }

        let args = Args { file_descriptor };

        // this will get the address of the imfs deployed on this pod
        let contract_addr = Runtime::contract_id_for_name("imfs");

        let file_content = Runtime::call_contract::<String>(
            contract_addr,                               //address of imfs
            "read".to_string(),                          //method to call
            Some(serde_json::to_string(&args).unwrap()), //serialized args
        )
        .map_err(|err| err.to_string())?;

        // The IMFS contract returns the file content directly as a string
        // Upload the text content to S3
        self.upload_text(bucket, key, file_content).await
    }

    /// Returns the JSON schema defining available tools for the S3 contract.
    ///
    /// # Returns
    ///
    /// * `String` - JSON string containing tool definitions for all S3 operations.
    #[query]
    fn tools(&self) -> String {
        r#"[
  {
    "type": "function",
    "function": {
      "name": "upload_text",
      "description": "Uploads text content to the specified S3 bucket\n",
      "parameters": {
        "type": "object",
        "properties": {
          "bucket": {
            "type": "string",
            "description": "The name of the bucket\n"
          },
          "key": {
            "type": "string",
            "description": "The key (path/filename) for the object\n"
          },
          "text": {
            "type": "string",
            "description": "The text content to upload\n"
          }
        },
        "required": [
          "bucket",
          "key",
          "text"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "upload_from_file",
      "description": "Reads a file from the encoded filedescriptor (like ey9320... ) and uploads its text content to S3\n",
      "parameters": {
        "type": "object",
        "properties": {
          "bucket": {
            "type": "string",
            "description": "The name of the bucket\n"
          },
          "key": {
            "type": "string",
            "description": "The key (path/filename) for the object\n"
          },
          "file_descriptor": {
            "type": "string",
            "description": "The base64 encoded file descriptor to the file to read and upload\n"
          }
        },
        "required": [
          "bucket",
          "key",
          "file_descriptor"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "download",
      "description": "Downloads a file from the specified S3 bucket\n",
      "parameters": {
        "type": "object",
        "properties": {
          "bucket": {
            "type": "string",
            "description": "The name of the bucket\n"
          },
          "key": {
            "type": "string",
            "description": "The key (path/filename) for the object\n"
          }
        },
        "required": [
          "bucket",
          "key"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "list_objects",
      "description": "Lists objects in the specified S3 bucket (optionally under a prefix)\n",
      "parameters": {
        "type": "object",
        "properties": {
          "bucket": {
            "type": "string",
            "description": "The name of the bucket\n"
          },
          "prefix": {
            "type": "string",
            "description": "The prefix to filter objects (optional)\n"
          }
        },
        "required": [
          "bucket"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "delete",
      "description": "Deletes an object from the specified S3 bucket\n",
      "parameters": {
        "type": "object",
        "properties": {
          "bucket": {
            "type": "string",
            "description": "The name of the bucket\n"
          },
          "key": {
            "type": "string",
            "description": "The key (path/filename) for the object\n"
          }
        },
        "required": [
          "bucket",
          "key"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "upload_external_url_to_s3",
      "description": "Uploads a file from an external URL and  to S3\n",
      "parameters": {
        "type": "object",
        "properties": {
          "bucket": {
            "type": "string",
            "description": "The name of the S3 bucket to upload to\n"
          },
          "key": {
            "type": "string",
            "description": "The key (path/filename) for the object in S3\n"
          },
          "url": {
            "type": "string",
            "description": "The external URL to get the file from\n"
          }
        },
        "required": [
          "bucket",
          "key",
          "url"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "list_buckets",
      "description": "Lists all S3 buckets for the given credentials\n",
      "parameters": {
        "type": "object",
        "properties": {},
        "required": []
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "create_bucket",
      "description": "Creates a new S3 bucket\n",
      "parameters": {
        "type": "object",
        "properties": {
          "bucket": {
            "type": "string",
            "description": "The bucket name\n"
          }
        },
        "required": [
          "bucket"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "delete_bucket",
      "description": "Deletes an S3 bucket\n",
      "parameters": {
        "type": "object",
        "properties": {
          "bucket": {
            "type": "string",
            "description": "The bucket name\n"
          }
        },
        "required": [
          "bucket"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "get_bucket_location",
      "description": "Gets the location of an S3 bucket\n",
      "parameters": {
        "type": "object",
        "properties": {
          "bucket": {
            "type": "string",
            "description": ""
          }
        },
        "required": [
          "bucket"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "get_bucket_acl",
      "description": "Gets the ACL of an S3 bucket\n",
      "parameters": {
        "type": "object",
        "properties": {
          "bucket": {
            "type": "string",
            "description": ""
          }
        },
        "required": [
          "bucket"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "get_bucket_versioning",
      "description": "Gets the versioning status of an S3 bucket\n",
      "parameters": {
        "type": "object",
        "properties": {
          "bucket": {
            "type": "string",
            "description": "The bucket name\n"
          }
        },
        "required": [
          "bucket"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "set_bucket_versioning",
      "description": "Enables or disables versioning on an S3 bucket\n",
      "parameters": {
        "type": "object",
        "properties": {
          "bucket": {
            "type": "string",
            "description": "The bucket name\n"
          },
          "enabled": {
            "type": "boolean",
            "description": "Whether to enable versioning\n"
          }
        },
        "required": [
          "bucket",
          "enabled"
        ]
      }
    }
  }
]"#.to_string()
    }

    /// Returns the JSON schema defining available prompts for the S3 contract.
    ///
    /// # Returns
    ///
    /// * `String` - JSON string containing prompt definitions (currently empty).
    #[query]
    fn prompts(&self) -> String {
        r#"{
}"#
        .to_string()
    }
}

impl AwsS3ContractState {
    // Helper method to get file data from WeilMemory
    fn get_file_from_weilmemory(&self, file_path: String) -> Result<Vec<u8>, String> {
        let file_size = self
            .web_server
            .size_bytes(file_path.clone())
            .map_err(|e| format!("File not found: {}", e))?;

        let chunk_size = self.web_server.get_chunk_size();
        let total_chunks = (file_size + chunk_size - 1) / chunk_size;

        let mut file_data = Vec::with_capacity(file_size as usize);

        for chunk_index in 0..total_chunks {
            let (status_code, _headers, chunk_data) =
                self.web_server
                    .http_content(file_path.clone(), chunk_index, "GET".to_string());

            if status_code == 200 {
                file_data.extend_from_slice(&chunk_data);
            } else {
                return Err(format!(
                    "Failed to read chunk {}: HTTP {}",
                    chunk_index, status_code
                ));
            }
        }

        // Trim to actual file size (last chunk might be padded)
        file_data.truncate(file_size as usize);
        Ok(file_data)
    }

    // Helper method to upload data to S3
    async fn upload_data_to_s3(
        &self,
        bucket: String,
        key: String,
        data: Vec<u8>,
    ) -> Result<(), String> {
        let config = self.secrets.config();
        let credentials = S3Credentials {
            access_key_id: config.access_key_id.clone(),
            secret_access_key: config.secret_access_key.clone(),
            region: config.region.clone(),
            session_token: None,
        };
        let params = S3UploadParams {
            credentials,
            bucket,
            key,
            content: data,
        };
        S3::upload(params).map(|_| ()).map_err(|e| e.to_string())
    }

    /// Downloads a file from an external URL as a stream and uploads it to S3 as a single object.
    pub async fn upload_file_from_url(
        &self,
        bucket: String,
        key: String,
        url: String,
    ) -> Result<String, String> {
        // Download file as a stream from the external URL
        let file = S3::download_file_stream(&url)
            .map_err(|e| format!("Failed to download file stream: {}", e))?;

        let config = self.secrets.config();
        let credentials = S3Credentials {
            access_key_id: config.access_key_id.clone(),
            secret_access_key: config.secret_access_key.clone(),
            region: config.region.clone(),
            session_token: None,
        };
        let params = S3UploadParams {
            credentials,
            bucket,
            key,
            content: vec![], // content is ignored for upload from external source
        };
        // Upload the file stream to S3
        S3::upload_file_stream(&file, params)
            .map_err(|e| format!("Failed to upload file stream to S3: {}", e))
    }

    // Method to register uploaded files (call this when files are uploaded to WebServer)
    pub fn register_uploaded_file(&mut self, file_path: String) -> Result<(), String> {
        // Check if file exists in WebServer
        self.web_server
            .size_bytes(file_path.clone())
            .map_err(|e| format!("File not found: {}", e))?;

        // Add to tracking if not already present
        if !self.uploaded_file_paths.contains(&file_path) {
            self.uploaded_file_paths.push(file_path);
        }

        Ok(())
    }

    pub fn list_uploaded_files(&self) -> Result<Vec<UploadedFileInfo>, String> {
        let mut files = Vec::new();
        for file_path in &self.uploaded_file_paths {
            if let Ok(file_info) = self.get_file_info(file_path.clone()) {
                files.push(file_info);
            }
        }
        Ok(files)
    }

    pub fn delete_from_memory(&mut self, file_path: String) -> Result<(), String> {
        // Remove from tracking
        self.uploaded_file_paths.retain(|path| path != &file_path);

        // The file will remain in WeilMemory but won't be tracked
        Ok(())
    }

    pub fn get_file_info(&self, file_path: String) -> Result<UploadedFileInfo, String> {
        // Get info directly from WebServer
        let size_bytes = self
            .web_server
            .size_bytes(file_path.clone())
            .map_err(|e| format!("File not found: {}", e))?;
        let chunk_size = self.web_server.get_chunk_size();
        let total_chunks = (size_bytes + chunk_size - 1) / chunk_size;

        Ok(UploadedFileInfo {
            file_path,
            size_bytes,
            total_chunks,
        })
    }
}
