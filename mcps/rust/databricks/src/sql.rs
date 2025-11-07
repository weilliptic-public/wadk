use serde_json;
use std::collections::HashMap;
use weil_rs::http::{HttpClient, HttpMethod};

/// SQL warehouse and query execution functions for Databricks
pub struct SqlClient {
    base_url: String,
    token: String,
}

impl SqlClient {
    /// Create a new SqlClient instance
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

    /// Execute a SQL statement
    pub async fn execute_sql(&self, query_str: String, warehouse_id: String) -> Result<String, String> {
        let url = format!("{}/sql/statements", self.base_url);
        
        let request = serde_json::json!({
            "statement": query_str,
            "warehouse_id": warehouse_id,
            "wait_timeout": "20s",
            "on_wait_timeout": "CANCEL"
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

    /// List SQL warehouses
    pub async fn list_sql_warehouses(&self, _warehouse_id: String) -> Result<String, String> {
        let url = format!("{}/sql/warehouses", self.base_url);
        
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

    /// Start a SQL warehouse
    pub async fn start_sql_warehouse(&self, warehouse_id: String) -> Result<String, String> {
        let url = format!("{}/sql/warehouses/{}/start", self.base_url, warehouse_id);
        
        let response = HttpClient::request(&url, HttpMethod::Post)
            .headers(self.get_headers())
            .json(&serde_json::json!({}))
            .send()
            .map_err(|e| format!("Request failed: {}", e))?;

        if response.status() < 200 || response.status() >= 300 {
            return Err(format!("API Error: HTTP {}", response.status()));
        }

        let response_text = response.text();
        Ok(response_text)
    }

    /// Stop a SQL warehouse
    pub async fn stop_sql_warehouse(&self, warehouse_id: String) -> Result<String, String> {
        let url = format!("{}/sql/warehouses/{}/stop", self.base_url, warehouse_id);
        
        let response = HttpClient::request(&url, HttpMethod::Post)
            .headers(self.get_headers())
            .json(&serde_json::json!({}))
            .send()
            .map_err(|e| format!("Request failed: {}", e))?;

        if response.status() < 200 || response.status() >= 300 {
            return Err(format!("API Error: HTTP {}", response.status()));
        }

        let response_text = response.text();
        Ok(response_text)
    }

    /// Create a SQL warehouse
    pub async fn create_sql_warehouse(&self, name: String, cluster_size: String, min_num_clusters: i32, max_num_clusters: i32, auto_stop_mins: i32) -> Result<String, String> {
        let url = format!("{}/sql/warehouses", self.base_url);
        
        let request = serde_json::json!({
            "name": name,
            "cluster_size": cluster_size,
            "min_num_clusters": min_num_clusters,
            "max_num_clusters": max_num_clusters,
            "auto_stop_mins": auto_stop_mins,
            "warehouse_type": "PRO",
            "enable_photon": true,
            "enable_serverless_compute": true
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

    /// List SQL queries
    pub async fn list_sql_queries(&self, user_id: String, include_metrics: Option<bool>) -> Result<String, String> {
        let mut url = format!("{}/sql/queries?user_id={}", self.base_url, user_id);
        
        if let Some(include_metrics_val) = include_metrics {
            url.push_str(&format!("&include_metrics={}", include_metrics_val));
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


    /// Create a SQL alert
    pub async fn create_sql_alert(&self, name: String, query_id: String, column: String, op: String, threshold: String, rearm: i32) -> Result<String, String> {
        let url = format!("{}/preview/sql/alerts", self.base_url);
        
        // Parse threshold as a number (the API expects a numeric value, not a string)
        let threshold_value: f64 = threshold.parse()
            .map_err(|_| format!("Invalid threshold value: {}", threshold))?;
        
        let request = serde_json::json!({
            "name": name,
            "query_id": query_id,
            "options": {
                "column": column,
                "op": op,
                "value": threshold_value
            },
            "rearm": rearm
        });
        
        let response = HttpClient::request(&url, HttpMethod::Post)
            .headers(self.get_headers())
            .json(&request)
            .send()
            .map_err(|e| format!("Request failed: {}", e))?;

        if response.status() < 200 || response.status() >= 300 {
            let status = response.status();
            let error_text = response.text();
            return Err(format!("API Error: HTTP {} - {}", status, error_text));
        }

        let response_text = response.text();
        Ok(response_text)
    }

}