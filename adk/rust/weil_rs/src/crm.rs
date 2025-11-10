use crate::runtime::{get_length_prefixed_bytes_from_string, read_bytes_from_memory};
use serde::{Deserialize, Serialize};

// Updated external function declarations to match new parameter structure
#[link(wasm_import_module = "env")]
extern "C" {
    fn salesforce_crud(params: i32) -> i32;
    fn salesforce_soql_query(params: i32) -> i32;
    fn salesforce_describe_object(params: i32) -> i32;
}

// These structs need to match exactly with the host side
#[derive(Debug, Serialize, Deserialize)]
pub struct SalesforceCredentials {
    pub client_id: String,
    pub client_secret: String,
    pub username: String,
    pub password: String,
    pub security_token: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SalesforceCrudParams {
    pub credentials: SalesforceCredentials,
    pub object: String,
    pub record_id: String,
    pub fields: String,
    pub operation: CrudOperation,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SalesforceQueryParams {
    pub credentials: SalesforceCredentials,
    pub soql_query: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SalesforceDescribeParams {
    pub credentials: SalesforceCredentials,
    pub object: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CrudOperation {
    Create,
    Read,
    Update,
    Delete,
}

pub struct Salesforce {
    pub client_id: String,
    pub client_secret: String,
    pub username: String,
    pub password: String,
    pub security_token: String,
}

impl Salesforce {
    /// Creates a SalesforceCredentials struct from the client's stored credentials.
    /// This is a convenience method to avoid code duplication across API calls.
    /// 
    /// # Returns
    /// A SalesforceCredentials struct populated with this client's credentials
    fn get_credentials(&self) -> SalesforceCredentials {
        SalesforceCredentials {
            client_id: self.client_id.clone(),
            client_secret: self.client_secret.clone(),
            username: self.username.clone(),
            password: self.password.clone(),
            security_token: self.security_token.clone(),
        }
    }

    /// Performs CRUD operations on Salesforce objects.
    /// 
    /// # Parameters
    /// * `object` - Salesforce object API name (e.g., "Account", "Contact")
    /// * `record_id` - Record ID for read/update/delete operations (use empty string for create)
    /// * `fields` - JSON string containing field names and values
    /// * `operation` - The CRUD operation to perform
    /// 
    /// # Returns
    /// * `Ok(String)` - JSON response from Salesforce API
    /// * `Err(anyhow::Error)` - Error if operation fails
    /// 
    /// # Example
    /// ```
    /// let result = client.call_salesforce_crud(
    ///     "Account", 
    ///     "001XXXXXXXXXXXXXXX", 
    ///     r#"{"Name": "Updated Account Name"}"#,
    ///     CrudOperation::Update
    /// )?;
    /// ```
    pub fn call_salesforce_crud(
        &self,
        object: &str,
        record_id: &str,
        fields: &str,
        operation: CrudOperation,
    ) -> Result<String, anyhow::Error> {
        let params = SalesforceCrudParams {
            credentials: self.get_credentials(),
            object: object.to_string(),
            record_id: record_id.to_string(),
            fields: fields.to_string(),
            operation: operation, // Direct enum, no serialization
        };

        let params_json = serde_json::to_string(&params)?;
        let raw_params = get_length_prefixed_bytes_from_string(&params_json, 0);

        let ptr = unsafe { salesforce_crud(raw_params.as_ptr() as _) };

        let result = read_bytes_from_memory(ptr)?;
        Ok(result)
    }

    /// Executes a SOQL (Salesforce Object Query Language) query.
    /// 
    /// # Parameters
    /// * `soql_query` - The SOQL query string to execute
    /// 
    /// # Returns
    /// * `Ok(String)` - JSON response containing query results
    /// * `Err(anyhow::Error)` - Error if query fails
    /// 
    /// # Example
    /// ```
    /// let result = client.call_salesforce_soql_query(
    ///     "SELECT Id, Name FROM Account LIMIT 10"
    /// )?;
    /// ```
    pub fn call_salesforce_soql_query(&self, soql_query: &str) -> Result<String, anyhow::Error> {
        let params = SalesforceQueryParams {
            credentials: self.get_credentials(),
            soql_query: soql_query.to_string(),
        };

        let params_json = serde_json::to_string(&params)?;
        let raw_params = get_length_prefixed_bytes_from_string(&params_json, 0);

        let ptr = unsafe { salesforce_soql_query(raw_params.as_ptr() as _) };

        let result = read_bytes_from_memory(ptr)?;
        Ok(result)
    }
    
    /// Retrieves metadata and schema information for a Salesforce object.
    /// This includes field definitions, relationships, permissions, etc.
    /// 
    /// # Parameters
    /// * `object` - Salesforce object API name to describe
    /// 
    /// # Returns
    /// * `Ok(String)` - JSON response containing object metadata
    /// * `Err(anyhow::Error)` - Error if describe operation fails
    /// 
    /// # Example
    /// ```
    /// let metadata = client.call_salesforce_describe_object("Account")?;
    /// ```
    pub fn call_salesforce_describe_object(&self, object: &str) -> Result<String, anyhow::Error> {
        let params = SalesforceDescribeParams {
            credentials: self.get_credentials(),
            object: object.to_string(), // Changed from object_name
        };

        let params_json = serde_json::to_string(&params)?;
        let raw_params = get_length_prefixed_bytes_from_string(&params_json, 0);

        let ptr = unsafe { salesforce_describe_object(raw_params.as_ptr() as _) };

        let result = read_bytes_from_memory(ptr)?;
        Ok(result)
    }
}
