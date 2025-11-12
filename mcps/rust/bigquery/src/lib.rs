use serde::{Deserialize, Serialize};
use weil_macros::{constructor, mutate, query, smart_contract, WeilType};
use weil_rs::collections::map::WeilMap;
use weil_rs::collections::WeilId;
use weil_rs::config::Secrets;
use weil_rs::http::{HttpClient, HttpMethod};
use weil_rs::runtime::Runtime;
use std::collections::{BTreeMap, HashMap};

/// Configuration struct for connecting to Google BigQuery.
///
/// This stores OAuth and project information required
/// for making authenticated API calls to BigQuery.
#[derive(Debug, Serialize, Deserialize, WeilType, Default)]
pub struct BigQueryConfig {
    /// The GCP project ID.
    project_id: String,
    /// The current access token (may expire).
    access_token: String,
    /// The refresh token used to obtain new access tokens.
    refresh_token: String,
    /// The OAuth client ID.
    client_id: String,
    /// The OAuth client secret.
    client_secret: String,
}

/// Represents metadata for a BigQuery dataset.
#[derive(Debug, Serialize, Deserialize)]
pub struct Dataset {
    /// The dataset identifier.
    dataset_id: String,
    /// The GCP project ID.
    project_id: String,
    /// Optional human-readable name.
    friendly_name: Option<String>,
    /// Optional description of the dataset.
    description: Option<String>,
}

/// Represents metadata for a BigQuery table.
#[derive(Debug, Serialize, Deserialize)]
pub struct Table {
    /// The table identifier.
    table_id: String,
    /// The parent dataset identifier.
    dataset_id: String,
    /// The GCP project ID.
    project_id: String,
    /// Optional human-readable name.
    friendly_name: Option<String>,
    /// Optional description of the table.
    description: Option<String>,
}

/// Trait defining the available operations for interacting with BigQuery.
trait BigQuery {
    /// Creates a new contract state instance.
    fn new() -> Result<Self, String>
    where
        Self: Sized;

    /// Creates a new dataset in BigQuery.
    ///
    /// # Arguments
    /// * `dataset_id` - The ID of the dataset to create.
    /// * `friendly_name` - Optional display name.
    /// * `description` - Optional dataset description.
    async fn create_dataset(
        &self,
        dataset_id: String,
        friendly_name: Option<String>,
        description: Option<String>,
    ) -> Result<(), String>;

    /// Fetches metadata of a dataset by its ID.
    async fn get_dataset(&self, dataset_id: String) -> Result<Dataset, String>;

    /// Updates an existing dataset’s metadata.
    ///
    /// You can update the friendly name or description.
    async fn update_dataset(
        &self,
        dataset_id: String,
        friendly_name: Option<String>,
        description: Option<String>,
    ) -> Result<(), String>;

    /// Deletes a dataset from BigQuery.
    ///
    /// # Arguments
    /// * `delete_contents` - Whether to delete contained tables as well.
    async fn delete_dataset(&self, dataset_id: String, delete_contents: bool) -> Result<(), String>;

    /// Lists all tables within a given dataset.
    async fn list_tables(&self, dataset_id: String) -> Result<Vec<Table>, String>;

    /// Executes a raw SQL query on BigQuery.
    ///
    /// Returns the JSON response as a string.
    async fn execute_query(&self, sql: String) -> Result<String, String>;

    /// Returns the tool definitions as JSON for agent use.
    fn tools(&self) -> String;

    /// Returns the available prompt templates.
    fn prompts(&self) -> String;
}

/// Contract state holding secrets for BigQuery API access.
#[derive(Serialize, Deserialize, WeilType)]
pub struct BigQueryContractState {
    /// Secure storage of BigQuery credentials and configuration.
    secrets: Secrets<BigQueryConfig>,
}

impl BigQueryContractState {
    /// Refreshes the current access token using the stored refresh token.
    ///
    /// Makes a POST request to Google’s OAuth2 endpoint and returns a new token.
    fn refresh_access_token(&self) -> Result<String, String> {
        let config = self.secrets.config();
        let body = serde_json::json!({
            "client_id": config.client_id,
            "client_secret": config.client_secret,
            "refresh_token": config.refresh_token,
            "grant_type": "refresh_token"
        });
        Runtime::debug_log("Refreshing access token");

        let mut headers: HashMap<String, String> = HashMap::new();
        headers.insert("Content-Type".into(), "application/json".into());

        let response = HttpClient::request("https://oauth2.googleapis.com/token", HttpMethod::Post)
            .headers(headers)
            .json(&body)
            .send()
            .map_err(|e| format!("Refresh token request failed: {:?}", e))?;

        let json = response.json::<serde_json::Value>()
            .map_err(|e| format!("Failed to parse refresh token response: {:?}", e))?;

        if let Some(new_token) = json.get("access_token").and_then(|v| v.as_str()) {
            Ok(new_token.to_string())
        } else {
            Err(format!("Invalid refresh token response: {:?}", json))
        }
    }

    /// Sends an HTTP request to the BigQuery API with proper authentication headers.
    ///
    /// If the access token is expired (401 Unauthorized), automatically refreshes
    /// and retries the request once.
    ///
    /// # Arguments
    /// * `method` – HTTP method (GET, POST, PUT, DELETE)
    /// * `url` – Full URL to call
    /// * `body` – Optional JSON body to send
    ///
    /// # Returns
    /// A parsed JSON value (`serde_json::Value`) from the response body.
    async fn make_authenticated_request(
        &self,
        method: HttpMethod,
        url: &str,
        body: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, String> {
        let config = self.secrets.config();

        let mut headers = HashMap::new();
        headers.insert("Authorization".into(), format!("Bearer {}", config.access_token));
        headers.insert("Content-Type".into(), "application/json".into());

        let response_result = {
            let mut request = HttpClient::request(url, method.clone()).headers(headers.clone());
            if let Some(body_data) = body.clone() {
                request = request.json(&body_data);
            }
            request.send()
        };

        let mut response = match response_result {
            Ok(resp) => {
                let text = resp.text();
                if text.contains("401") || text.to_lowercase().contains("unauthorized") {

                    let new_token = self.refresh_access_token()?;
                    let mut new_headers = headers.clone();
                    new_headers.insert("Authorization".into(), format!("Bearer {}", new_token));

                    let mut retry_request = HttpClient::request(url, method).headers(new_headers);
                    if let Some(body_data) = body {
                        retry_request = retry_request.json(&body_data);
                    }

                    retry_request.send().map_err(|e| format!("HTTP retry failed: {:?}", e))?
                } else {
                    return serde_json::from_str::<serde_json::Value>(&text)
                        .map_err(|e| format!("Failed to parse JSON response: {:?}", e));
                }
            }
            Err(err) => return Err(format!("HTTP request failed: {:?}", err)),
        };

        response.json::<serde_json::Value>()
            .map_err(|e| format!("Failed to parse JSON response: {:?}", e))
    }
}

#[smart_contract]
impl BigQuery for BigQueryContractState {
    /// Initializes a new BigQuery smart contract state with empty secrets.
    #[constructor]
    fn new() -> Result<Self, String> {
        Ok(Self {
            secrets: Secrets::<BigQueryConfig>::new(),
        })
    }

    /// Creates a new dataset in the configured BigQuery project.
    #[query]
    async fn create_dataset(
        &self,
        dataset_id: String,
        friendly_name: Option<String>,
        description: Option<String>,
    ) -> Result<(), String> {
        let config = self.secrets.config();
        let url = format!(
            "https://bigquery.googleapis.com/bigquery/v2/projects/{}/datasets",
            config.project_id
        );

        let mut dataset_body = serde_json::json!({
            "datasetReference": {
                "datasetId": dataset_id,
                "projectId": config.project_id
            }
        });

        if let Some(name) = friendly_name {
            dataset_body["friendlyName"] = serde_json::Value::String(name);
        }
        if let Some(desc) = description {
            dataset_body["description"] = serde_json::Value::String(desc);
        }

        self.make_authenticated_request(HttpMethod::Post, &url, Some(dataset_body)).await?;
        Ok(())
    }

    /// Retrieves details of a dataset.
    #[query]
    async fn get_dataset(&self, dataset_id: String) -> Result<Dataset, String> {
        let config = self.secrets.config();
        let url = format!(
            "https://bigquery.googleapis.com/bigquery/v2/projects/{}/datasets/{}",
            config.project_id, dataset_id
        );

        let response = self.make_authenticated_request(HttpMethod::Get, &url, None).await?;
        Ok(Dataset {
            dataset_id,
            project_id: config.project_id,
            friendly_name: response["friendlyName"].as_str().map(|s| s.to_string()),
            description: response["description"].as_str().map(|s| s.to_string()),
        })
    }

    /// Updates an existing dataset’s metadata such as name or description.
    #[query]
    async fn update_dataset(
        &self,
        dataset_id: String,
        friendly_name: Option<String>,
        description: Option<String>,
    ) -> Result<(), String> {
        let config = self.secrets.config();
        let url = format!(
            "https://bigquery.googleapis.com/bigquery/v2/projects/{}/datasets/{}",
            config.project_id, dataset_id
        );

        let mut update_body = serde_json::json!({});
        if let Some(name) = friendly_name {
            update_body["friendlyName"] = serde_json::Value::String(name);
        }
        if let Some(desc) = description {
            update_body["description"] = serde_json::Value::String(desc);
        }

        self.make_authenticated_request(HttpMethod::Put, &url, Some(update_body)).await?;
        Ok(())
    }

    /// Deletes a dataset from BigQuery.
    ///
    /// # Arguments
    /// * `delete_contents` - If true, deletes all tables before removing the dataset.
    #[query]
    async fn delete_dataset(&self, dataset_id: String, delete_contents: bool) -> Result<(), String> {
        let config = self.secrets.config();
        let url = format!(
            "https://bigquery.googleapis.com/bigquery/v2/projects/{}/datasets/{}?deleteContents={}",
            config.project_id, dataset_id, delete_contents
        );

        self.make_authenticated_request(HttpMethod::Delete, &url, None).await?;
        Ok(())
    }

    /// Lists all tables within the specified dataset.
    #[query]
    async fn list_tables(&self, dataset_id: String) -> Result<Vec<Table>, String> {
        let config = self.secrets.config();
        let url = format!(
            "https://bigquery.googleapis.com/bigquery/v2/projects/{}/datasets/{}/tables",
            config.project_id, dataset_id
        );

        let response = self.make_authenticated_request(HttpMethod::Get, &url, None).await?;
        let tables = response["tables"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|t| {
                Some(Table {
                    table_id: t["tableReference"]["tableId"].as_str()?.to_string(),
                    dataset_id: t["tableReference"]["datasetId"].as_str()?.to_string(),
                    project_id: t["tableReference"]["projectId"].as_str()?.to_string(),
                    friendly_name: t["friendlyName"].as_str().map(|s| s.to_string()),
                    description: t["description"].as_str().map(|s| s.to_string()),
                })
            })
            .collect();

        Ok(tables)
    }

    /// Executes a SQL query in BigQuery and returns the response as a JSON string.
    #[query]
    async fn execute_query(&self, sql: String) -> Result<String, String> {
        let config = self.secrets.config();
        let url = format!(
            "https://bigquery.googleapis.com/bigquery/v2/projects/{}/queries",
            config.project_id
        );

        let body = serde_json::json!({
            "query": sql,
            "useLegacySql": false,
            "maxResults": 1000
        });

        let response = self.make_authenticated_request(HttpMethod::Post, &url, Some(body)).await?;
        serde_json::to_string(&response).map_err(|e| format!("Failed to serialize response: {}", e))
    }

   #[query]
    fn tools(&self) -> String {
        r#"[
  {
    "type": "function",
    "function": {
      "name": "create_dataset",
      "description": "Create a new dataset in bigquery\n",
      "parameters": {
        "type": "object",
        "properties": {
          "dataset_id": {
            "type": "string",
            "description": "the id of the dataset\n"
          },
          "friendly_name": {
            "type": "string",
            "description": "the friendly name for this dataset you want to give\n"
          },
          "description": {
            "type": "string",
            "description": "the description of the dataset you want to give\n"
          }
        },
        "required": [
          "dataset_id"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "get_dataset",
      "description": "Read/get dataset details in bigquery\n",
      "parameters": {
        "type": "object",
        "properties": {
          "dataset_id": {
            "type": "string",
            "description": "the id of the dataset you want to read\n"
          }
        },
        "required": [
          "dataset_id"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "update_dataset",
      "description": "Update dataset metadata  in bigquery\n",
      "parameters": {
        "type": "object",
        "properties": {
          "dataset_id": {
            "type": "string",
            "description": "the id of the dataset you want to update\n"
          },
          "friendly_name": {
            "type": "string",
            "description": "the new `friendly name` you want to give this dataset\n"
          },
          "description": {
            "type": "string",
            "description": "the new `description` you want to give this dataset\n"
          }
        },
        "required": [
          "dataset_id"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "delete_dataset",
      "description": "Delete a dataset  in bigquery\n",
      "parameters": {
        "type": "object",
        "properties": {
          "dataset_id": {
            "type": "string",
            "description": "the id of the dataset you want to delete\n"
          },
          "delete_contents": {
            "type": "boolean",
            "description": "whether you want to delete the contents also\n"
          }
        },
        "required": [
          "dataset_id",
          "delete_contents"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "list_tables",
      "description": "list all the tables in the dataset  in bigquery\n",
      "parameters": {
        "type": "object",
        "properties": {
          "dataset_id": {
            "type": "string",
            "description": "the id of the dataset you want to list tables of\n"
          }
        },
        "required": [
          "dataset_id"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "execute_query",
      "description": "executes a sql query given by the parameter `sql` on the bigQuery database\n",
      "parameters": {
        "type": "object",
        "properties": {
          "sql": {
            "type": "string",
            "description": "the raw sql query you want to execute\n"
          }
        },
        "required": [
          "sql"
        ]
      }
    }
  }
]"#.to_string()
    }
    
    #[query]
    fn prompts(&self) -> String {
        r#"{
  "prompts": []
}"#.to_string()
    }
}
