use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use weil_rs::http::{HttpClient, HttpMethod};

#[derive(Debug, Serialize, Deserialize)]
pub struct FunctionInfo {
    pub name: String,
    pub catalog_name: String,
    pub schema_name: String,
    pub input_params: Option<Vec<FunctionParameter>>,
    pub data_type: String,
    pub full_data_type: String,
    pub parameter_style: String,
    pub routine_body: String,
    pub routine_definition: String,
    pub language: String,
    pub is_deterministic: bool,
    pub sql_data_access: String,
    pub is_null_call: bool,
    pub security_type: String,
    pub specific_name: String,
    pub comment: Option<String>,
    pub properties: Option<Value>,
    pub created_at: Option<i64>,
    pub created_by: Option<String>,
    pub updated_at: Option<i64>,
    pub updated_by: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FunctionParameter {
    pub name: String,
    #[serde(rename = "type")]
    pub type_name: String,
    pub comment: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateFunctionRequest {
    pub function_info: FunctionInfo,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListFunctionsResponse {
    pub functions: Vec<FunctionInfo>,
}

pub struct FunctionsClient {
    workspace_url: String,
    pat_token: String,
}

impl FunctionsClient {
    pub fn new(workspace_url: &str, pat_token: &str) -> Self {
        Self {
            workspace_url: workspace_url.to_string(),
            pat_token: pat_token.to_string(),
        }
    }

    /// Get headers with authentication
    fn get_headers(&self) -> HashMap<String, String> {
        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), format!("Bearer {}", self.pat_token));
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        headers
    }

    pub async fn list_functions(&self, catalog_name: &str, schema_name: &str) -> Result<String, String> {
        let url = format!(
            "{}/api/2.1/unity-catalog/functions?catalog_name={}&schema_name={}",
            self.workspace_url, catalog_name, schema_name
        );

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

    pub async fn get_function(&self, function_name: &str) -> Result<String, String> {
        let url = format!(
            "{}/api/2.1/unity-catalog/functions/{}",
            self.workspace_url, function_name
        );

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

    pub async fn create_function(&self, function_info: FunctionInfo) -> Result<String, String> {
        let url = format!("{}/api/2.1/unity-catalog/functions", self.workspace_url);

        let request_body = CreateFunctionRequest { function_info };
        let json_body = serde_json::to_string(&request_body)
            .map_err(|e| format!("Failed to serialize request: {}", e))?;

        let response = HttpClient::request(&url, HttpMethod::Post)
            .headers(self.get_headers())
            .body(json_body)
            .send()
            .map_err(|e| format!("Request failed: {}", e))?;

        if response.status() < 200 || response.status() >= 300 {
            return Err(format!("API Error: HTTP {}", response.status()));
        }

        let response_text = response.text();
        Ok(response_text)
    }

    pub async fn delete_function(&self, function_name: &str) -> Result<String, String> {
        let url = format!(
            "{}/api/2.1/unity-catalog/functions/{}",
            self.workspace_url, function_name
        );

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
}
