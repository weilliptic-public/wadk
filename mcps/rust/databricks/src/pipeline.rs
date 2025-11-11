use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::HashMap;
use weil_rs::http::{HttpClient, HttpMethod};

#[derive(Debug)]
pub struct PipelineClient {
    base_url: String,
    token: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PipelineCreateRequest {
    pub name: String,
    pub libraries: Vec<Library>,
    pub catalog: String,
    pub target: String,
    pub serverless: bool,
    pub continuous: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Library {
    pub notebook: Notebook,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Notebook {
    pub path: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PipelineUpdateRequest {
    pub name: Option<String>,
    pub libraries: Option<Vec<Library>>,
    pub catalog: Option<String>,
    pub target: Option<String>,
    pub serverless: Option<bool>,
    pub continuous: Option<bool>,
}

impl PipelineClient {
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

    pub async fn list_pipelines(&self) -> Result<String, String> {
        let url = format!("{}/pipelines", self.base_url);
        
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

    pub async fn create_pipeline(&self, request: PipelineCreateRequest) -> Result<String, String> {
        let url = format!("{}/pipelines", self.base_url);
        
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

    pub async fn get_pipeline(&self, pipeline_id: String) -> Result<String, String> {
        let url = format!("{}/pipelines/{}", self.base_url, pipeline_id);
        
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

    pub async fn update_pipeline(&self, pipeline_id: String, request: PipelineUpdateRequest) -> Result<String, String> {
        let url = format!("{}/pipelines/{}", self.base_url, pipeline_id);
        
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

    pub async fn delete_pipeline(&self, pipeline_id: String) -> Result<String, String> {
        let url = format!("{}/pipelines/{}", self.base_url, pipeline_id);
        
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

    pub async fn execute_pipeline(&self, pipeline_id: String) -> Result<String, String> {
        let url = format!("{}/pipelines/{}/updates", self.base_url, pipeline_id);
        
        let request = serde_json::json!({});
        
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

    pub async fn get_pipeline_events(&self, pipeline_id: String) -> Result<String, String> {
        let url = format!("{}/pipelines/{}/events", self.base_url, pipeline_id);
        
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
