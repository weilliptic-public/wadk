use serde_json;
use std::collections::HashMap;
use weil_rs::http::{HttpClient, HttpMethod};

/// Model Serving functions for Databricks
pub struct ModelServingClient {
    base_url: String,
    token: String,
}

impl ModelServingClient {
    /// Create a new ModelServingClient instance
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

    /// List all model serving endpoints
    pub async fn list_serving_endpoints(&self) -> Result<String, String> {
        let url = format!("{}/serving-endpoints", self.base_url);
        
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

    /// Get details of a specific serving endpoint
    pub async fn get_serving_endpoint(&self, name: String) -> Result<String, String> {
        let url = format!("{}/serving-endpoints/{}", self.base_url, name);
        
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

    /// Create a new serving endpoint
    pub async fn create_serving_endpoint(&self, name: String, config: serde_json::Value) -> Result<String, String> {
        let url = format!("{}/serving-endpoints", self.base_url);
        
        let request = serde_json::json!({
            "name": name,
            "config": config
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

    /// Update a serving endpoint configuration
    pub async fn update_serving_endpoint(&self, name: String, config: serde_json::Value) -> Result<String, String> {
        let url = format!("{}/serving-endpoints/{}/config", self.base_url, name);
        
        let request = serde_json::json!({
            "served_entities": config["served_entities"]
        });
        
        let response = HttpClient::request(&url, HttpMethod::Put)
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

    /// Delete a serving endpoint
    pub async fn delete_serving_endpoint(&self, name: String) -> Result<String, String> {
        let url = format!("{}/serving-endpoints/{}", self.base_url, name);
        
        let response = HttpClient::request(&url, HttpMethod::Delete)
            .headers(self.get_headers())
            .send()
            .map_err(|e| format!("Request failed: {}", e))?;

        if response.status() < 200 || response.status() >= 300 {
            return Err(format!("API Error: HTTP {}", response.status()));
        }

        let response_text = response.text();
        Ok(response_text)
    }

    /// Get serving endpoint logs
    pub async fn get_serving_endpoint_logs(&self, name: String, lines: Option<i32>) -> Result<String, String> {
        let mut url = format!("{}/serving-endpoints/{}/logs", self.base_url, name);
        
        if let Some(line_count) = lines {
            url.push_str(&format!("?lines={}", line_count));
        }
        
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

    /// Query a serving endpoint (make predictions)
    pub async fn query_serving_endpoint(&self, name: String, data: serde_json::Value) -> Result<String, String> {
        let url = format!("{}/serving-endpoints/{}/invocations", self.base_url, name);
        
        let response = HttpClient::request(&url, HttpMethod::Post)
            .headers(self.get_headers())
            .json(&data)
            .send()
            .map_err(|e| format!("Request failed: {}", e))?;

        if response.status() < 200 || response.status() >= 300 {
            return Err(format!("API Error: HTTP {}", response.status()));
        }

        let response_text = response.text();
        Ok(response_text)
    }
}
