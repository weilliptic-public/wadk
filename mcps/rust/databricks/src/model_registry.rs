use serde_json;
use std::collections::HashMap;
use weil_rs::http::{HttpClient, HttpMethod};

/// Model Registry functions for Databricks MLflow
pub struct ModelRegistryClient {
    base_url: String,
    token: String,
}

impl ModelRegistryClient {
    /// Create a new ModelRegistryClient instance
    pub fn new(workspace_url: &str, personal_access_token: &str) -> Self {
        let base_url = format!("{}/api/2.0/mlflow", workspace_url.trim_end_matches('/'));
        
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

    /// List all registered models
    pub async fn list_registered_models(&self) -> Result<String, String> {
        let url = format!("{}/registered-models/search", self.base_url);
        
        let request = serde_json::json!({
            "max_results": 100
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

    /// Get details of a specific registered model
    pub async fn get_registered_model(&self, name: String) -> Result<String, String> {
        let url = format!("{}/registered-models/get", self.base_url);
        
        let request = serde_json::json!({
            "name": name
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

    /// Create a new registered model
    pub async fn create_registered_model(&self, name: String, description: Option<String>) -> Result<String, String> {
        let url = format!("{}/registered-models/create", self.base_url);
        
        let mut request = serde_json::json!({
            "name": name
        });
        
        if let Some(desc) = description {
            request["description"] = serde_json::Value::String(desc);
        }
        
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

    /// List model versions for a registered model
    pub async fn list_model_versions(&self, name: String) -> Result<String, String> {
        let url = format!("{}/model-versions/list", self.base_url);
        
        let request = serde_json::json!({
            "name": name,
            "max_results": 100
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

    /// Get details of a specific model version
    pub async fn get_model_version(&self, name: String, version: String) -> Result<String, String> {
        let url = format!("{}/model-versions/get", self.base_url);
        
        let request = serde_json::json!({
            "name": name,
            "version": version
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

    /// Set a model version as the production stage
    pub async fn set_model_version_stage(&self, name: String, version: String, stage: String) -> Result<String, String> {
        let url = format!("{}/model-versions/set-stage", self.base_url);
        
        let request = serde_json::json!({
            "name": name,
            "version": version,
            "stage": stage
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

    /// Delete a registered model
    pub async fn delete_registered_model(&self, name: String) -> Result<String, String> {
        let url = format!("{}/registered-models/delete", self.base_url);
        
        let request = serde_json::json!({
            "name": name
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
}
