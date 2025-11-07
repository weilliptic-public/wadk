use std::collections::HashMap;
use weil_rs::http::{HttpClient, HttpMethod};

/// Catalog management functions for Databricks Unity Catalog
pub struct CatalogClient {
    base_url: String,
    token: String,
}

impl CatalogClient {
    /// Create a new CatalogClient instance
    pub fn new(workspace_url: &str, personal_access_token: &str) -> Self {
        let base_url = format!("{}/api/2.1", workspace_url.trim_end_matches('/'));
        
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

    /// List all catalogs
    pub async fn list_catalogs(&self) -> Result<String, String> {
        let url = format!("{}/unity-catalog/catalogs", self.base_url);
        
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

    /// Get catalog details
    pub async fn get_catalog(&self, catalog_name: String) -> Result<String, String> {
        let url = format!("{}/unity-catalog/catalogs/{}", self.base_url, catalog_name);
        
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

    /// List schemas in a catalog
    pub async fn list_schemas(&self, catalog_name: String) -> Result<String, String> {
        let url = format!("{}/unity-catalog/schemas?catalog_name={}", self.base_url, catalog_name);
        
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

    /// Get schema details
    pub async fn get_schema(&self, catalog_name: String, schema_name: String) -> Result<String, String> {
        let url = format!("{}/unity-catalog/schemas/{}.{}", self.base_url, catalog_name, schema_name);
        
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

    /// List tables in a schema
    pub async fn list_tables(&self, catalog_name: String, schema_name: String) -> Result<String, String> {
        let url = format!("{}/unity-catalog/tables?catalog_name={}&schema_name={}", 
                         self.base_url, catalog_name, schema_name);
        
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

    /// Get table details
    pub async fn get_table(&self, catalog_name: String, schema_name: String, table_name: String) -> Result<String, String> {
        let url = format!("{}/unity-catalog/tables/{}.{}.{}", 
                         self.base_url, catalog_name, schema_name, table_name);
        
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

    /// List metastores
    pub async fn list_metastores(&self) -> Result<String, String> {
        let url = format!("{}/unity-catalog/metastores", self.base_url);
        
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
