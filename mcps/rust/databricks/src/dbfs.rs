use serde_json;
use std::collections::HashMap;
use weil_rs::http::{HttpClient, HttpMethod};

/// DBFS (Databricks File System) operations
pub struct DbfsClient {
    base_url: String,
    token: String,
}

impl DbfsClient {
    /// Create a new DbfsClient instance
    pub fn new(workspace_url: &str, personal_access_token: &str) -> Self {
        let base_url = format!("{}/api/2.0", workspace_url.trim_end_matches('/'));
        
        Self {
            base_url,
            token: personal_access_token.to_string(),
        }
    }

    /// Get headers with authentication
    fn get_headers(&self) -> HashMap<String, String> {
        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), format!("Bearer {}", self.token));
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        headers
    }

    /// List files and directories in DBFS
    pub async fn list_dbfs_files(&self, path: String) -> Result<String, String> {
        let url = format!("{}/dbfs/list?path={}", self.base_url, urlencoding::encode(&path));
        
        let response = HttpClient::request(&url, HttpMethod::Get)
            .headers(self.get_headers())
            .send()
            .map_err(|e| format!("Request failed: {}", e))?;

        if response.status() < 200 || response.status() >= 300 {
            return Err(format!("API Error: HTTP {}", response.status()));
        }

        let response_text = response.text();
        Ok(response_text)
    }

    /// Get file information
    pub async fn get_dbfs_file_info(&self, path: String) -> Result<String, String> {
        let url = format!("{}/dbfs/get-status?path={}", self.base_url, urlencoding::encode(&path));
        
        let response = HttpClient::request(&url, HttpMethod::Get)
            .headers(self.get_headers())
            .send()
            .map_err(|e| format!("Request failed: {}", e))?;

        if response.status() < 200 || response.status() >= 300 {
            return Err(format!("API Error: HTTP {}", response.status()));
        }

        let response_text = response.text();
        Ok(response_text)
    }

    /// Delete a file or directory
    pub async fn delete_dbfs_file(&self, path: String) -> Result<String, String> {
        let url = format!("{}/dbfs/delete", self.base_url);
        
        let request = serde_json::json!({
            "path": path,
            "recursive": false
        });
        
        let response = HttpClient::request(&url, HttpMethod::Post)
            .headers(self.get_headers())
            .json(&request)
            .send()
            .map_err(|e| format!("Request failed: {}", e))?;

        if response.status() < 200 || response.status() >= 300 {
            return Err(format!("API Error: HTTP {}", response.status()));
        }

        let response_text = response.text();
        Ok(response_text)
    }

    /// Move a file or directory
    pub async fn move_dbfs_file(&self, source_path: String, destination_path: String) -> Result<String, String> {
        let url = format!("{}/dbfs/move", self.base_url);
        
        let request = serde_json::json!({
            "source_path": source_path,
            "destination_path": destination_path
        });
        
        let response = HttpClient::request(&url, HttpMethod::Post)
            .headers(self.get_headers())
            .json(&request)
            .send()
            .map_err(|e| format!("Request failed: {}", e))?;

        if response.status() < 200 || response.status() >= 300 {
            return Err(format!("API Error: HTTP {}", response.status()));
        }

        let response_text = response.text();
        Ok(response_text)
    }

    /// Copy a file or directory
    pub async fn copy_dbfs_file(&self, source_path: String, destination_path: String) -> Result<String, String> {
        let url = format!("{}/dbfs/copy", self.base_url);
        
        let request = serde_json::json!({
            "source_path": source_path,
            "destination_path": destination_path
        });
        
        let response = HttpClient::request(&url, HttpMethod::Post)
            .headers(self.get_headers())
            .json(&request)
            .send()
            .map_err(|e| format!("Request failed: {}", e))?;

        if response.status() < 200 || response.status() >= 300 {
            return Err(format!("API Error: HTTP {}", response.status()));
        }

        let response_text = response.text();
        Ok(response_text)
    }

    /// Write content to a file in DBFS
    pub async fn write_dbfs_file(&self, path: String, content: String, overwrite: bool) -> Result<String, String> {
        let url = format!("{}/dbfs/put", self.base_url);
        
        // Convert content to base64
        let content_bytes = content.as_bytes();
        let encoded_content = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, content_bytes);
        
        let request = serde_json::json!({
            "path": path,
            "contents": encoded_content,
            "overwrite": overwrite
        });
        
        let response = HttpClient::request(&url, HttpMethod::Post)
            .headers(self.get_headers())
            .json(&request)
            .send()
            .map_err(|e| format!("Request failed: {}", e))?;

        if response.status() < 200 || response.status() >= 300 {
            return Err(format!("API Error: HTTP {}", response.status()));
        }

        let response_text = response.text();
        Ok(response_text)
    }

    /// Read file contents from DBFS
    pub async fn read_dbfs_file(&self, path: String, offset: Option<i64>, length: Option<i64>) -> Result<String, String> {
        let mut url = format!("{}/dbfs/read?path={}", self.base_url, urlencoding::encode(&path));
        
        if let Some(offset_val) = offset {
            url.push_str(&format!("&offset={}", offset_val));
        }
        
        if let Some(length_val) = length {
            url.push_str(&format!("&length={}", length_val));
        }
        
        let response = HttpClient::request(&url, HttpMethod::Get)
            .headers(self.get_headers())
            .send()
            .map_err(|e| format!("Request failed: {}", e))?;

        if response.status() < 200 || response.status() >= 300 {
            return Err(format!("API Error: HTTP {}", response.status()));
        }

        let response_text = response.text();
        
        // Try to decode base64 content if present
        if let Ok(parsed_response) = serde_json::from_str::<serde_json::Value>(&response_text) {
            if let Some(contents) = parsed_response.get("data") {
                if let Some(encoded_content) = contents.as_str() {
                    if let Ok(decoded_bytes) = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, encoded_content) {
                        if let Ok(decoded_content) = String::from_utf8(decoded_bytes) {
                            let mut result = parsed_response.clone();
                            result["data"] = serde_json::Value::String(decoded_content);
                            return Ok(serde_json::to_string(&result).unwrap_or(response_text));
                        }
                    }
                }
            }
        }
        
        Ok(response_text)
    }

    /// Create a directory in workspace
    pub async fn create_directory(&self, path: String) -> Result<String, String> {
        let url = format!("{}/workspace/mkdirs", self.base_url);
        
        let request = serde_json::json!({
            "path": path,
            "is_dir": true
        });
        
        let response = HttpClient::request(&url, HttpMethod::Post)
            .headers(self.get_headers())
            .json(&request)
            .send()
            .map_err(|e| format!("Request failed: {}", e))?;

        if response.status() < 200 || response.status() >= 300 {
            return Err(format!("API Error: HTTP {}", response.status()));
        }

        let response_text = response.text();
        Ok(response_text)
    }

    /// List workspace directory contents (alternative to DBFS for workspace paths)
    pub async fn list_workspace_directory(&self, path: String) -> Result<String, String> {
        let url = format!("{}/workspace/list?path={}", self.base_url, urlencoding::encode(&path));
        
        let response = HttpClient::request(&url, HttpMethod::Get)
            .headers(self.get_headers())
            .send()
            .map_err(|e| format!("Request failed: {}", e))?;

        if response.status() < 200 || response.status() >= 300 {
            return Err(format!("API Error: HTTP {}", response.status()));
        }

        let response_text = response.text();
        Ok(response_text)
    }
}
