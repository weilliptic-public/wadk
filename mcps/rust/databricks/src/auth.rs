use serde_json;
use std::collections::HashMap;
use weil_rs::http::{HttpClient, HttpMethod};

/// Authentication and user management functions for Databricks
pub struct AuthClient {
    base_url: String,
    token: String,
}

impl AuthClient {
    /// Create a new AuthClient instance
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

    /// List all users in the workspace
    pub async fn list_users(&self) -> Result<String, String> {
        let url = format!("{}/preview/scim/v2/Users", self.base_url);
        
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

    /// Get a specific user by ID
    pub async fn get_user(&self, user_id: String) -> Result<String, String> {
        let url = format!("{}/preview/scim/v2/Users/{}", self.base_url, user_id);
        
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

    /// Create a new user
    pub async fn create_user(&self, username: String, email: String, display_name: Option<String>) -> Result<String, String> {
        let url = format!("{}/preview/scim/v2/Users", self.base_url);
        
        let request = serde_json::json!({
            "schemas": ["urn:ietf:params:scim:schemas:core:2.0:User"],
            "userName": username,
            "displayName": display_name,
            "emails": [{
                "value": email,
                "type": "work",
                "primary": true
            }],
            "active": true
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
