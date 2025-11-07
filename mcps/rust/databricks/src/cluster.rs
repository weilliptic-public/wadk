use serde_json;
use std::collections::HashMap;
use weil_rs::http::{HttpClient, HttpMethod};

/// Cluster management operations for Databricks
pub struct ClusterClient {
    base_url: String,
    token: String,
}

impl ClusterClient {
    /// Create a new ClusterClient instance
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

    /// List all clusters
    pub async fn list_clusters(&self) -> Result<String, String> {
        let url = format!("{}/clusters/list", self.base_url);
        
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

    /// Get a specific cluster by ID
    pub async fn get_cluster(&self, cluster_id: String) -> Result<String, String> {
        let url = format!("{}/clusters/get?cluster_id={}", self.base_url, cluster_id);
        
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

    /// Create a new cluster
    pub async fn create_cluster(&self, name: String, spark_version: String, node_type: String, num_workers: i32) -> Result<String, String> {
        let url = format!("{}/clusters/create", self.base_url);
        
        let request = serde_json::json!({
            "cluster_name": name,
            "spark_version": spark_version,
            "node_type_id": node_type,
            "driver_node_type_id": node_type,
            "num_workers": num_workers,
            "autotermination_minutes": 30,
            "enable_elastic_disk": true
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
