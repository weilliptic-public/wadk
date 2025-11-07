//! # Salesforce MCP Server (WeilChain Applet)
//!
//! This module implements a **Model Context Server (MCP)** applet exposing
//! high-level Salesforce CRM operations to on-chain agents. It supports:
//!
//! - CRUD via [`SalesforceCRM::create`], [`SalesforceCRM::read`],
//!   [`SalesforceCRM::update`], and [`SalesforceCRM::delete`]
//! - Running arbitrary SOQL queries via [`SalesforceCRM::execute_soql_query`]
//! - Object metadata introspection via [`SalesforceCRM::get_object_metadata`]
//! - Apex test orchestration via [`SalesforceCRM::run_apex_test`],
//!   [`SalesforceCRM::get_test_status`], and [`SalesforceCRM::run_all_tests`]
//! - Permission set inspection via [`SalesforceCRM::get_permission_set_details`]
//! - Agent-facing tool schemas via [`SalesforceCRM::tools`] and prompt packs via [`SalesforceCRM::prompts`]
//!
//! ## Flow
//! * Credentials are sourced from [`Secrets<SalesforceConfig>`].
//! * Calls are delegated to the platform-provided [`weil_rs::crm::Salesforce`]
//!   helper, which performs auth and REST calls to Salesforce APIs.
//! * Inputs are validated minimally (e.g., non-empty IDs, field name/value arity).
//!
//! ## Security notes
//! * Use least-privilege OAuth apps and profiles. Avoid broad permissions.
//! * Never log secrets. This contract does not emit secrets; keep it that way.
//! * Consider rate limits and governor limits when composing bulk operations.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use weil_macros::{WeilType, constructor, query, smart_contract};
use weil_rs::config::Secrets;
use weil_rs::crm::{CrudOperation, Salesforce};

/// OAuth and user credentials required to authenticate with Salesforce.
///
/// Fields are retrieved from the contract's secret store.
#[derive(Debug, Serialize, Deserialize, WeilType, Default)]
pub struct SalesforceConfig {
    /// Connected App client ID.
    client_id: String,
    /// Connected App client secret.
    client_secret: String,
    /// Salesforce username (login email).
    username: String,
    /// Salesforce password for the user.
    password: String,
    /// User's security token (appended to password in some auth flows).
    security_token: String,
}

/// Public MCP interface implemented by [`SalesforceCRMContractState`].
trait SalesforceCRM {
    /// Initialize contract state and secret handle.
    fn new() -> Result<Self, String>
    where
        Self: Sized;

    /// Create a new record for a given object type.
    ///
    /// # Parameters
    /// * `object_type` — e.g., `"Account"`, `"Contact"`, etc.
    /// * `field_names` — ordered list of field API names.
    /// * `field_values` — ordered list of field values (same length as `field_names`).
    ///
    /// # Errors
    /// * Returns an error if `field_names.len() != field_values.len()`.
    async fn create(
        &self,
        object_type: String,
        field_names: Vec<String>,
        field_values: Vec<String>,
    ) -> Result<String, String>;

    /// Read a record by object type and record ID (e.g., `"001..."`).
    ///
    /// # Errors
    /// * Returns an error if `record_id` is empty.
    async fn read(&self, object_type: String, record_id: String) -> Result<String, String>;

    /// Update a record by object type and record ID with field/value pairs.
    ///
    /// # Errors
    /// * Returns an error if `record_id` is empty.
    /// * Returns an error if `field_names.len() != field_values.len()`.
    async fn update(
        &self,
        object_type: String,
        record_id: String,
        field_names: Vec<String>,
        field_values: Vec<String>,
    ) -> Result<String, String>;

    /// Delete a record by object type and record ID.
    ///
    /// # Errors
    /// * Returns an error if `record_id` is empty.
    async fn delete(&self, object_type: String, record_id: String) -> Result<String, String>;

    /// Execute a raw SOQL query string and return the JSON response payload.
    async fn execute_soql_query(&self, soql_query: String) -> Result<String, String>;

    /// Retrieve object metadata (describe) for a given object API name.
    async fn get_object_metadata(&self, object_name: String) -> Result<String, String>;

    /// Enqueue and run a specific Apex test class; returns the parent job ID.
    ///
    /// # Errors
    /// * Returns an error if `test_class_name` is empty or not found.
    async fn run_apex_test(&self, test_class_name: String) -> Result<String, String>;

    /// Retrieve the status for an Async Apex Job.
    ///
    /// # Errors
    /// * Returns an error if `job_id` is empty or not found.
    async fn get_test_status(&self, job_id: String) -> Result<String, String>;

    /// Enqueue all Apex test classes (by simple name heuristic) and report job IDs.
    async fn run_all_tests(&self) -> Result<String, String>;

    /// Get a permission set and its assigned users, returned as pretty JSON.
    ///
    /// # Errors
    /// * Returns an error if the permission set name is empty or not found.
    async fn get_permission_set_details(
        &self,
        permission_set_name: String,
    ) -> Result<String, String>;

    /// Return agent tool schema JSON describing callable functions/parameters.
    fn tools(&self) -> String;

    /// Return prompt pack(s) for agent guidance (currently empty).
    fn prompts(&self) -> String;
}

/// On-chain contract state holding connection secrets and exposing MCP queries.
#[derive(Serialize, Deserialize, WeilType)]
pub struct SalesforceCRMContractState {
    /// Contract-scoped secret storage containing [`SalesforceConfig`].
    secrets: Secrets<SalesforceConfig>,
}

#[smart_contract]
impl SalesforceCRM for SalesforceCRMContractState {
    /// Initialize the contract with a fresh `Secrets<SalesforceConfig>` handle.
    #[constructor]
    fn new() -> Result<Self, String>
    where
        Self: Sized,
    {
        Ok(SalesforceCRMContractState {
            secrets: Secrets::<SalesforceConfig>::new(),
        })
    }

    /// See [`SalesforceCRM::create`].
    #[query]
    async fn create(
        &self,
        object_type: String,
        field_names: Vec<String>,
        field_values: Vec<String>,
    ) -> Result<String, String> {
        let credentials = self.secrets.config();
        if field_names.len() != field_values.len() {
            return Err("Field names and values must have the same length".to_string());
        }
        let fields_json = self.create_fields_json(&field_names, &field_values)?;
        let sf = Salesforce {
            client_id: credentials.client_id,
            client_secret: credentials.client_secret,
            username: credentials.username,
            password: credentials.password,
            security_token: credentials.security_token,
        };
        sf.call_salesforce_crud(
            &object_type,
            "", // No record ID for create operation
            &fields_json,
            CrudOperation::Create,
        )
        .map_err(|e| e.to_string())
    }

    /// See [`SalesforceCRM::read`].
    #[query]
    async fn read(&self, object_type: String, record_id: String) -> Result<String, String> {
        let credentials = self.secrets.config();
        if record_id.is_empty() {
            return Err("Record ID is required for read operation".to_string());
        }

        let sf = Salesforce {
            client_id: credentials.client_id.clone(),
            client_secret: credentials.client_secret.clone(),
            username: credentials.username.clone(),
            password: credentials.password.clone(),
            security_token: credentials.security_token.clone(),
        };

        sf.call_salesforce_crud(
            &object_type,
            &record_id,
            "", // No fields needed for read
            CrudOperation::Read,
        )
        .map_err(|e| e.to_string())
    }

    /// See [`SalesforceCRM::update`].
    #[query]
    async fn update(
        &self,
        object_type: String,
        record_id: String,
        field_names: Vec<String>,
        field_values: Vec<String>,
    ) -> Result<String, String> {
        let credentials = self.secrets.config();
        if record_id.is_empty() {
            return Err("Record ID is required for read operation".to_string());
        }

        let fields_json = self.create_fields_json(&field_names, &field_values)?;

        let sf = Salesforce {
            client_id: credentials.client_id,
            client_secret: credentials.client_secret,
            username: credentials.username,
            password: credentials.password,
            security_token: credentials.security_token,
        };

        sf.call_salesforce_crud(
            &object_type,
            &record_id,
            &fields_json,
            CrudOperation::Update,
        )
        .map_err(|e| e.to_string())
    }

    /// See [`SalesforceCRM::delete`].
    #[query]
    async fn delete(&self, object_type: String, record_id: String) -> Result<String, String> {
        let credentials = self.secrets.config();
        if record_id.is_empty() {
            return Err("Record ID is required for read operation".to_string());
        }

        let sf = Salesforce {
            client_id: credentials.client_id.clone(),
            client_secret: credentials.client_secret.clone(),
            username: credentials.username.clone(),
            password: credentials.password.clone(),
            security_token: credentials.security_token.clone(),
        };

        sf.call_salesforce_crud(
            &object_type,
            &record_id,
            "", // No fields needed for delete
            CrudOperation::Delete,
        )
        .map_err(|e| e.to_string())
    }

    /// See [`SalesforceCRM::execute_soql_query`].
    #[query]
    async fn execute_soql_query(&self, soql_query: String) -> Result<String, String> {
        let credentials = self.secrets.config();
        let sf = Salesforce {
            client_id: credentials.client_id.clone(),
            client_secret: credentials.client_secret.clone(),
            username: credentials.username.clone(),
            password: credentials.password.clone(),
            security_token: credentials.security_token.clone(),
        };

        sf.call_salesforce_soql_query(&soql_query)
            .map_err(|e| e.to_string())
    }

    /// See [`SalesforceCRM::get_object_metadata`].
    #[query]
    async fn get_object_metadata(&self, object_name: String) -> Result<String, String> {
        let credentials = self.secrets.config();
        let sf = Salesforce {
            client_id: credentials.client_id.clone(),
            client_secret: credentials.client_secret.clone(),
            username: credentials.username.clone(),
            password: credentials.password.clone(),
            security_token: credentials.security_token.clone(),
        };

        sf.call_salesforce_describe_object(&object_name)
            .map_err(|e| e.to_string())
    }

    /// See [`SalesforceCRM::run_apex_test`].
    ///
    /// Steps:
    /// 1. Lookup the Apex class ID by name.
    /// 2. Create an `ApexTestQueueItem` for that class.
    /// 3. Retrieve the `ParentJobId` for the queued item (AsyncApexJob).
    #[query]
    async fn run_apex_test(&self, test_class_name: String) -> Result<String, String> {
        if test_class_name.is_empty() {
            return Err("Test class name is required".to_string());
        }

        // First, find the test class ID
        let find_class_query = format!(
            "SELECT Id FROM ApexClass WHERE Name = '{}' AND NamespacePrefix = null",
            test_class_name
        );

        let class_result = self.execute_soql_query(find_class_query).await?;

        // Parse the result to extract class ID
        let parsed: serde_json::Value = serde_json::from_str(&class_result)
            .map_err(|e| format!("Failed to parse class query result: {}", e))?;

        let class_id = parsed
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|record| record.get("Id"))
            .and_then(|id| id.as_str())
            .ok_or_else(|| format!("Test class '{}' not found", test_class_name))?;

        // Create test queue item to run the specific test
        let queue_item_id = self
            .create(
                "ApexTestQueueItem".to_string(),
                vec!["ApexClassId".to_string()],
                vec![class_id.to_string()],
            )
            .await?;

        // Find the AsyncApexJob ID associated with this specific test class
        let job_query = format!(
            "SELECT ParentJobId FROM ApexTestQueueItem WHERE Id = '{}'",
            queue_item_id
        );

        let job_result = self.execute_soql_query(job_query).await?;
        let job_parsed: serde_json::Value = serde_json::from_str(&job_result)
            .map_err(|e| format!("Failed to parse job query result: {}", e))?;

        let job_id = job_parsed
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|record| record.get("ParentJobId"))
            .and_then(|id| id.as_str())
            .ok_or_else(|| "Failed to get test job ID from queue item".to_string())?;

        Ok(job_id.to_string())
    }

    /// See [`SalesforceCRM::get_test_status`].
    #[query]
    async fn get_test_status(&self, job_id: String) -> Result<String, String> {
        if job_id.is_empty() {
            return Err("Job ID is required".to_string());
        }

        let status_query = format!(
            "SELECT Id, Status FROM AsyncApexJob WHERE Id = '{}'",
            job_id
        );

        let result = self.execute_soql_query(status_query).await?;

        // Parse and format the status information
        let parsed: serde_json::Value = serde_json::from_str(&result)
            .map_err(|e| format!("Failed to parse status query result: {}", e))?;

        if let Some(job) = parsed.as_array().and_then(|arr| arr.first()) {
            Ok(serde_json::to_string_pretty(job)
                .unwrap_or_else(|_| "Failed to format job status".to_string()))
        } else {
            Err("Job not found".to_string())
        }
    }

    /// See [`SalesforceCRM::run_all_tests`].
    ///
    /// Heuristic: selects Apex classes whose names contain `Test`/`test`.
    /// Enqueues each into `ApexTestQueueItem`, then resolves one or more job IDs.
    #[query]
    async fn run_all_tests(&self) -> Result<String, String> {
        // Get all test classes
        let test_classes_query = "SELECT Id, Name FROM ApexClass WHERE NamespacePrefix = null AND (Name LIKE '%Test%' OR Name LIKE '%test%')";
        let classes_result = self
            .execute_soql_query(test_classes_query.to_string())
            .await?;

        let parsed: serde_json::Value = serde_json::from_str(&classes_result)
            .map_err(|e| format!("Failed to parse test classes result: {}", e))?;

        let test_classes = parsed
            .as_array()
            .ok_or_else(|| "Invalid test classes response format".to_string())?;

        if test_classes.is_empty() {
            return Ok("No test classes found in the org".to_string());
        }

        let mut queue_item_ids = Vec::new();
        for class in test_classes {
            if let Some(class_id) = class.get("Id").and_then(|id| id.as_str()) {
                let queue_item_id = self
                    .create(
                        "ApexTestQueueItem".to_string(),
                        vec!["ApexClassId".to_string()],
                        vec![class_id.to_string()],
                    )
                    .await?;
                queue_item_ids.push(queue_item_id);
            }
        }

        let queue_ids_str = queue_item_ids
            .iter()
            .map(|id| format!("'{}'", id))
            .collect::<Vec<_>>()
            .join(",");

        let job_query = format!(
            "SELECT ParentJobId FROM ApexTestQueueItem WHERE Id IN ({})",
            queue_ids_str
        );

        let job_result = self.execute_soql_query(job_query).await?;
        let job_parsed: serde_json::Value = serde_json::from_str(&job_result)
            .map_err(|e| format!("Failed to parse job query result: {}", e))?;

        let job_ids: Vec<String> = job_parsed
            .as_array()
            .unwrap_or(&Vec::new())
            .iter()
            .filter_map(|record| record.get("ParentJobId"))
            .filter_map(|id| id.as_str())
            .map(|id| id.to_string())
            .collect();

        if job_ids.is_empty() {
            return Err("Failed to get any test job IDs".to_string());
        }

        let unique_job_ids: std::collections::HashSet<String> = job_ids.into_iter().collect();
        let unique_jobs: Vec<String> = unique_job_ids.into_iter().collect();

        if unique_jobs.len() == 1 {
            Ok(format!(
                "Started running {} test classes in job: {}",
                test_classes.len(),
                unique_jobs[0]
            ))
        } else {
            Ok(format!(
                "Started running {} test classes across {} jobs: {}",
                test_classes.len(),
                unique_jobs.len(),
                unique_jobs.join(", ")
            ))
        }
    }

    /// See [`SalesforceCRM::get_permission_set_details`].
    ///
    /// Returns a pretty-printed JSON object that includes:
    /// * `permission_set`: the PermissionSet record
    /// * `assigned_users`: the list of user assignments
    #[query]
    async fn get_permission_set_details(
        &self,
        permission_set_name: String,
    ) -> Result<String, String> {
        if permission_set_name.is_empty() {
            return Err("Permission set name is required".to_string());
        }

        let perm_set_query = format!(
            "SELECT Id, Name, Label, Description FROM PermissionSet WHERE Name = '{}'",
            permission_set_name
        );

        let perm_set_result = self.execute_soql_query(perm_set_query).await?;

        let parsed: serde_json::Value = serde_json::from_str(&perm_set_result)
            .map_err(|e| format!("Failed to parse permission set result: {}", e))?;

        let perm_set = parsed
            .as_array()
            .and_then(|arr| arr.first())
            .ok_or_else(|| format!("Permission set '{}' not found", permission_set_name))?;

        let perm_set_id = perm_set
            .get("Id")
            .and_then(|id| id.as_str())
            .ok_or_else(|| "Permission set ID not found".to_string())?;

        let assignments_query = format!(
            "SELECT Assignee.Id, Assignee.Username, Assignee.Name FROM PermissionSetAssignment WHERE PermissionSetId = '{}'",
            perm_set_id
        );

        let assignments_result = self.execute_soql_query(assignments_query).await?;

        // Combine permission set details with assignments
        let mut result = serde_json::Map::new();
        result.insert("permission_set".to_string(), perm_set.clone());

        let assignments: serde_json::Value = serde_json::from_str(&assignments_result)
            .map_err(|e| format!("Failed to parse assignments result: {}", e))?;
        result.insert("assigned_users".to_string(), assignments);

        Ok(serde_json::to_string_pretty(&result)
            .unwrap_or_else(|_| "Failed to format permission set details".to_string()))
    }

    /// Return the JSON tool schema used by function-calling agents to validate inputs.
    #[query]
    fn tools(&self) -> String {
        r#"[
  {
    "type": "function",
    "function": {
      "name": "create",
      "description": "Create a new record in Salesforce\n",
      "parameters": {
        "type": "object",
        "properties": {
          "object_type": {
            "type": "string",
            "description": "Salesforce object type (e.g., \"Account\", \"Contact\")\n"
          },
          "field_names": {
            "type": "array",
            "description": "Names of fields to set, all in strings\n"
          },
          "field_values": {
            "type": "array",
            "description": "Values for the fields (must match field_names in order and length), all in strings\n"
          }
        },
        "required": [
          "object_type",
          "field_names",
          "field_values"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "read",
      "description": "Read a record from Salesforce\n",
      "parameters": {
        "type": "object",
        "properties": {
          "object_type": {
            "type": "string",
            "description": "Salesforce object type (e.g., \"Account\", \"Contact\")\n"
          },
          "record_id": {
            "type": "string",
            "description": "ID of the record to read\n"
          }
        },
        "required": [
          "object_type",
          "record_id"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "update",
      "description": "Update an existing record in Salesforce\n",
      "parameters": {
        "type": "object",
        "properties": {
          "object_type": {
            "type": "string",
            "description": "Salesforce object type (e.g., \"Account\", \"Contact\")\n"
          },
          "record_id": {
            "type": "string",
            "description": "ID of the record to update\n"
          },
          "field_names": {
            "type": "array",
            "description": "Names of fields to update, all in strings\n"
          },
          "field_values": {
            "type": "array",
            "description": "Updated values (must match field_names in order and length), all in strings\n"
          }
        },
        "required": [
          "object_type",
          "record_id",
          "field_names",
          "field_values"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "delete",
      "description": "Delete a record from Salesforce\n",
      "parameters": {
        "type": "object",
        "properties": {
          "object_type": {
            "type": "string",
            "description": "Salesforce object type (e.g., \"Account\", \"Contact\")\n"
          },
          "record_id": {
            "type": "string",
            "description": "ID of the record to delete\n"
          }
        },
        "required": [
          "object_type",
          "record_id"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "execute_soql_query",
      "description": "Takes a raw query and executes it on the salesforce server\n",
      "parameters": {
        "type": "object",
        "properties": {
          "soql_query": {
            "type": "string",
            "description": "the raw soql query\n"
          }
        },
        "required": [
          "soql_query"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "get_object_metadata",
      "description": "gets the schema of the given object from salesforce\n",
      "parameters": {
        "type": "object",
        "properties": {
          "object_name": {
            "type": "string",
            "description": "the name of the object to get the metadata of\n"
          }
        },
        "required": [
          "object_name"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "run_apex_test",
      "description": "Run a specific Apex test class and return the job ID\n",
      "parameters": {
        "type": "object",
        "properties": {
          "test_class_name": {
            "type": "string",
            "description": "Name of the Apex test class to run (e.g., 'AccountControllerTest')\n"
          }
        },
        "required": [
          "test_class_name"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "get_test_status",
      "description": "Get the status and results of an Apex test job\n",
      "parameters": {
        "type": "object",
        "properties": {
          "job_id": {
            "type": "string",
            "description": "AsyncApexJob ID returned from run_apex_test\n"
          }
        },
        "required": [
          "job_id"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "run_all_tests",
      "description": "Run all Apex test classes in the org\n",
      "parameters": {
        "type": "object",
        "properties": {},
        "required": []
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "get_permission_set_details",
      "description": "Get details about a permission set including assigned users\n",
      "parameters": {
        "type": "object",
        "properties": {
          "permission_set_name": {
            "type": "string",
            "description": "Name of the permission set (e.g., 'Account_Manager_Extra_Access')\n"
          }
        },
        "required": [
          "permission_set_name"
        ]
      }
    }
  }
]"#.to_string()
    }

    /// Return an (extensible) prompt pack JSON for agent guidance.
    ///
    /// Currently returns an empty array; populate as needed to steer LLM behavior.
    #[query]
    fn prompts(&self) -> String {
        r#"{
  "prompts": []
}"#
        .to_string()
    }
}

impl SalesforceCRMContractState {
    /// Helper to serialize parallel `field_names`/`field_values` into a JSON object.
    ///
    /// # Errors
    /// * Returns an error if the vectors differ in length.
    fn create_fields_json(
        &self,
        field_names: &[String],
        field_values: &[String],
    ) -> Result<String, String> {
        if field_names.len() != field_values.len() {
            return Err("Field names and values must have the same length".to_string());
        }

        let mut fields = HashMap::new();
        for (i, name) in field_names.iter().enumerate() {
            fields.insert(name.clone(), field_values[i].clone());
        }

        serde_json::to_string(&fields).map_err(|e| format!("Failed to serialize fields: {}", e))
    }
}
