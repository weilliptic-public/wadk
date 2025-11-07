use serde_json;
use std::collections::HashMap;
use weil_rs::http::{HttpClient, HttpMethod};

/// Job management functions for Databricks
pub struct JobClient {
    base_url: String,
    token: String,
}

impl JobClient {
    /// Create a new JobClient instance
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

    /// List all jobs
    pub async fn list_jobs(&self) -> Result<String, String> {
        let url = format!("{}/jobs/list", self.base_url);
        
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

    /// Get job details
    pub async fn get_job(&self, job_id: String) -> Result<String, String> {
        let url = format!("{}/jobs/get?job_id={}", self.base_url, job_id);
        
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

    /// Run a job now
    pub async fn run_job_now(&self, job_id: String) -> Result<String, String> {
        let url = format!("{}/jobs/run-now", self.base_url);
        
        let request = serde_json::json!({
            "job_id": job_id
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

    /// Get job run details
    pub async fn get_job_run(&self, run_id: String) -> Result<String, String> {
        let url = format!("{}/jobs/runs/get?run_id={}", self.base_url, run_id);
        
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

    /// Cancel a job run
    pub async fn cancel_job_run(&self, run_id: String) -> Result<String, String> {
        let url = format!("{}/jobs/runs/cancel", self.base_url);
        
        let request = serde_json::json!({
            "run_id": run_id
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
