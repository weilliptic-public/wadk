use base64::{Engine as _, engine::general_purpose};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use weil_macros::{WeilType, constructor, query, smart_contract};
use weil_rs::config::Secrets;
use weil_rs::http::{HttpClient, HttpMethod};

mod structs;
use structs::*;

trait Servicenow {
    fn new() -> Result<Self, String>
    where
        Self: Sized;

    // Incident Management
    async fn create_incident(
        &self,
        short_description: String,
        description: String,
        priority: String,
    ) -> Result<Incident, String>;
    async fn get_incident(&self, sys_id: String) -> Result<Incident, String>;
    async fn delete_incident(&self, sys_id: String) -> Result<(), String>;
    async fn query_incidents(&self, query_str: String, limit: u32)
    -> Result<Vec<Incident>, String>;
    async fn add_comment(
        &self,
        incident_sys_id: String,
        comment: String,
    ) -> Result<Comment, String>;
    async fn resolve_incident(
        &self,
        sys_id: String,
        resolution_notes: String,
    ) -> Result<Incident, String>;
    async fn list_incidents(
        &self,
        query_str: Option<String>,
        limit: Option<u32>,
    ) -> Result<Vec<Incident>, String>;

    // Service Catalog
    async fn list_catalog_items(
        &self,
        query_str: Option<String>,
        limit: Option<u32>,
    ) -> Result<Vec<CatalogItem>, String>;
    async fn get_catalog_item(&self, sys_id: String) -> Result<CatalogItem, String>;
    async fn list_catalog_categories(
        &self,
        query_str: Option<String>,
        limit: Option<u32>,
    ) -> Result<Vec<CatalogCategory>, String>;
    async fn create_catalog_category(
        &self,
        title: String,
        description: Option<String>,
        parent: Option<String>,
    ) -> Result<CatalogCategory, String>;
    async fn move_catalog_items(
        &self,
        item_sys_ids: Vec<String>,
        target_category_sys_id: String,
    ) -> Result<(), String>;
    async fn create_catalog_item_variable(
        &self,
        catalog_item_sys_id: String,
        name: String,
        question_text: String,
        var_type: String,
        mandatory: bool,
    ) -> Result<CatalogVariable, String>;
    async fn list_catalog_item_variables(
        &self,
        catalog_item_sys_id: String,
    ) -> Result<Vec<CatalogVariable>, String>;
    async fn list_catalogs(
        &self,
        query_str: Option<String>,
        limit: Option<u32>,
    ) -> Result<Vec<serde_json::Value>, String>;

    // Catalog Optimization
    async fn get_optimization_recommendations(
        &self,
        catalog_item_sys_id: Option<String>,
    ) -> Result<Vec<serde_json::Value>, String>;

    // Change Management
    async fn create_change_request(
        &self,
        short_description: String,
        description: String,
        priority: String,
        risk: Option<String>,
        impact: Option<String>,
    ) -> Result<ChangeRequest, String>;
    async fn list_change_requests(
        &self,
        query_str: Option<String>,
        limit: Option<u32>,
    ) -> Result<Vec<ChangeRequest>, String>;
    async fn get_change_request_details(&self, sys_id: String) -> Result<ChangeRequest, String>;
    async fn add_change_task(
        &self,
        change_request_sys_id: String,
        short_description: String,
        description: String,
        assigned_to: Option<String>,
    ) -> Result<ChangeTask, String>;
    async fn submit_change_for_approval(&self, sys_id: String) -> Result<ChangeRequest, String>;
    async fn approve_change(
        &self,
        sys_id: String,
        approval_notes: Option<String>,
    ) -> Result<ChangeRequest, String>;
    async fn reject_change(
        &self,
        sys_id: String,
        rejection_notes: String,
    ) -> Result<ChangeRequest, String>;

    // Agile Story Management
    async fn create_story(
        &self,
        short_description: String,
        description: String,
        priority: Option<String>,
        story_points: Option<String>,
        epic_sys_id: Option<String>,
    ) -> Result<Story, String>;
    async fn list_stories(
        &self,
        query_str: Option<String>,
        limit: Option<u32>,
    ) -> Result<Vec<Story>, String>;
    async fn delete_story_dependency(&self, dependency_sys_id: String) -> Result<(), String>;

    // Agile Epic Management
    async fn create_epic(
        &self,
        short_description: String,
        description: String,
        priority: Option<String>,
    ) -> Result<Epic, String>;
    async fn list_epics(
        &self,
        query_str: Option<String>,
        limit: Option<u32>,
    ) -> Result<Vec<Epic>, String>;

    // Scrum Task Management
    async fn create_scrum_task(
        &self,
        short_description: String,
        description: String,
        story_sys_id: Option<String>,
        assigned_to: Option<String>,
    ) -> Result<ScrumTask, String>;
    async fn list_scrum_tasks(
        &self,
        query_str: Option<String>,
        limit: Option<u32>,
    ) -> Result<Vec<ScrumTask>, String>;

    // Project Management
    async fn create_project(
        &self,
        name: String,
        short_description: String,
        goal: Option<String>,
    ) -> Result<Project, String>;
    async fn list_projects(
        &self,
        query_str: Option<String>,
        limit: Option<u32>,
    ) -> Result<Vec<Project>, String>;

    // Workflow Management
    async fn list_workflows(
        &self,
        query_str: Option<String>,
        limit: Option<u32>,
    ) -> Result<Vec<Workflow>, String>;
    async fn get_workflow(&self, sys_id: String) -> Result<Workflow, String>;
    async fn create_workflow(
        &self,
        name: String,
        description: Option<String>,
        table: String,
    ) -> Result<Workflow, String>;
    async fn delete_workflow(&self, sys_id: String) -> Result<(), String>;

    // Script Include Management
    async fn list_script_includes(
        &self,
        query_str: Option<String>,
        limit: Option<u32>,
    ) -> Result<Vec<ScriptInclude>, String>;
    async fn get_script_include(&self, sys_id: String) -> Result<ScriptInclude, String>;
    async fn create_script_include(
        &self,
        name: String,
        description: Option<String>,
        script: String,
        api_name: Option<String>,
    ) -> Result<ScriptInclude, String>;
    async fn delete_script_include(&self, sys_id: String) -> Result<(), String>;

    // Changeset Management
    async fn list_changesets(
        &self,
        query_str: Option<String>,
        limit: Option<u32>,
    ) -> Result<Vec<Changeset>, String>;
    async fn get_changeset_details(&self, sys_id: String) -> Result<Changeset, String>;
    async fn create_changeset(
        &self,
        name: String,
        description: Option<String>,
    ) -> Result<Changeset, String>;
    async fn commit_changeset(&self, sys_id: String) -> Result<Changeset, String>;
    async fn publish_changeset(&self, sys_id: String) -> Result<Changeset, String>;

    // Knowledge Base Management
    async fn create_knowledge_base(
        &self,
        title: String,
        description: Option<String>,
    ) -> Result<KnowledgeBase, String>;
    async fn list_knowledge_bases(
        &self,
        query_str: Option<String>,
        limit: Option<u32>,
    ) -> Result<Vec<KnowledgeBase>, String>;
    async fn create_article(
        &self,
        short_description: String,
        text: String,
        knowledge_base_sys_id: String,
        category_sys_id: Option<String>,
    ) -> Result<KnowledgeArticle, String>;
    async fn publish_article(&self, sys_id: String) -> Result<KnowledgeArticle, String>;
    async fn list_articles(
        &self,
        query_str: Option<String>,
        limit: Option<u32>,
    ) -> Result<Vec<KnowledgeArticle>, String>;
    async fn get_article(&self, sys_id: String) -> Result<KnowledgeArticle, String>;

    // User Management
    async fn create_user(
        &self,
        user_name: String,
        first_name: String,
        last_name: String,
        email: String,
        department: Option<String>,
    ) -> Result<User, String>;
    async fn get_user(&self, identifier: String) -> Result<User, String>;
    async fn list_users(
        &self,
        query_str: Option<String>,
        limit: Option<u32>,
    ) -> Result<Vec<User>, String>;
    async fn create_group(
        &self,
        name: String,
        description: Option<String>,
        manager: Option<String>,
    ) -> Result<Group, String>;
    async fn add_group_members(
        &self,
        group_sys_id: String,
        user_sys_ids: Vec<String>,
    ) -> Result<(), String>;
    async fn remove_group_members(
        &self,
        group_sys_id: String,
        user_sys_ids: Vec<String>,
    ) -> Result<(), String>;
    async fn list_groups(
        &self,
        query_str: Option<String>,
        limit: Option<u32>,
    ) -> Result<Vec<Group>, String>;

    // UI Policy Tools
    async fn create_ui_policy(
        &self,
        name: String,
        description: Option<String>,
        table: String,
        catalog_item_sys_id: Option<String>,
    ) -> Result<UIPolicy, String>;
    async fn create_ui_policy_action(
        &self,
        ui_policy_sys_id: String,
        name: String,
        description: Option<String>,
        field_name: String,
        action: String,
    ) -> Result<UIPolicyAction, String>;

    fn tools(&self) -> String;
    fn prompts(&self) -> String;
}

#[derive(Serialize, Deserialize, WeilType)]
pub struct ServicenowContractState {
    // define your contract state here!
    secrets: Secrets<ServicenowConfig>,
}

impl ServicenowContractState {
    /// Creates a Basic Authentication header for ServiceNow API requests.
    ///
    /// Basic Authentication requires base64 encoding of the credentials in the format "username:password".
    /// This is mandated by the HTTP Basic Authentication standard (RFC 7617) and is what ServiceNow expects.
    /// Unlike reqwest which has a built-in .basic_auth() method, weil_rs::http::HttpClient requires us to
    /// manually construct the Authorization header with base64-encoded credentials.
    fn create_auth_header(&self) -> Result<String, String> {
        let config = self.secrets.config();
        let credentials = format!("{}:{}", config.username, config.password);
        let encoded = general_purpose::STANDARD.encode(credentials.as_bytes());
        Ok(format!("Basic {}", encoded))
    }

    fn get_base_url(&self) -> Result<String, String> {
        let config = self.secrets.config();
        Ok(config.base_url.clone())
    }
}

#[smart_contract]
impl Servicenow for ServicenowContractState {
    #[constructor]
    fn new() -> Result<Self, String>
    where
        Self: Sized,
    {
        Ok(Self {
            secrets: Secrets::<ServicenowConfig>::new(),
        })
    }

    #[query]
    async fn create_incident(
        &self,
        short_description: String,
        description: String,
        priority: String,
    ) -> Result<Incident, String> {
        let url = format!("{}/api/now/table/incident", self.get_base_url()?);
        let auth_header = self.create_auth_header()?;

        let payload = serde_json::json!({
            "short_description": short_description,
            "description": description,
            "priority": priority
        });

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), auth_header);
        headers.insert("Content-Type".to_string(), "application/json".to_string());

        let response = HttpClient::request(&url, HttpMethod::Post)
            .headers(headers)
            .json(&payload)
            .send()
            .map_err(|err| err.to_string())?;

        let response_text = response.text();

        // ServiceNow returns the result in a "result" field
        #[derive(Deserialize)]
        struct ServiceNowResponse {
            result: Incident,
        }

        let sn_response: ServiceNowResponse =
            serde_json::from_str(&response_text).map_err(|err| {
                format!(
                    "Failed to parse response: {}. Response was: {}",
                    err, response_text
                )
            })?;

        Ok(sn_response.result)
    }

    #[query]
    async fn get_incident(&self, sys_id: String) -> Result<Incident, String> {
        let url = format!("{}/api/now/table/incident/{}", self.get_base_url()?, sys_id);
        let auth_header = self.create_auth_header()?;

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), auth_header);

        let response = HttpClient::request(&url, HttpMethod::Get)
            .headers(headers)
            .send()
            .map_err(|err| err.to_string())?;

        let response_text = response.text();

        // ServiceNow returns the result in a "result" field
        #[derive(Deserialize)]
        struct ServiceNowResponse {
            result: Incident,
        }

        let sn_response: ServiceNowResponse =
            serde_json::from_str(&response_text).map_err(|err| {
                format!(
                    "Failed to parse response: {}. Response was: {}",
                    err, response_text
                )
            })?;

        Ok(sn_response.result)
    }

    #[query]
    async fn delete_incident(&self, sys_id: String) -> Result<(), String> {
        let url = format!("{}/api/now/table/incident/{}", self.get_base_url()?, sys_id);
        let auth_header = self.create_auth_header()?;

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), auth_header);

        let response = HttpClient::request(&url, HttpMethod::Delete)
            .headers(headers)
            .send()
            .map_err(|err| err.to_string())?;

        // Check if the deletion was successful (204 No Content is expected)
        if response.status() >= 204 && response.status() < 300 {
            Ok(())
        } else {
            Err(format!(
                "Failed to delete incident. Status: {}",
                response.status()
            ))
        }
    }

    #[query]
    async fn query_incidents(
        &self,
        query_str: String,
        limit: u32,
    ) -> Result<Vec<Incident>, String> {
        let url = format!("{}/api/now/table/incident", self.get_base_url()?);
        let auth_header = self.create_auth_header()?;

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), auth_header);

        // Add query parameters
        let mut query_params = Vec::new();
        query_params.push(("sysparm_query".to_string(), query_str));
        query_params.push(("sysparm_limit".to_string(), limit.to_string()));

        let response = HttpClient::request(&url, HttpMethod::Get)
            .headers(headers)
            .query(query_params)
            .send()
            .map_err(|err| err.to_string())?;

        let response_text = response.text();

        // ServiceNow returns the results in a "result" field as an array
        #[derive(Deserialize)]
        struct ServiceNowResponse {
            result: Vec<Incident>,
        }

        let sn_response: ServiceNowResponse =
            serde_json::from_str(&response_text).map_err(|err| {
                format!(
                    "Failed to parse response: {}. Response was: {}",
                    err, response_text
                )
            })?;

        Ok(sn_response.result)
    }

    #[query]
    async fn add_comment(
        &self,
        incident_sys_id: String,
        comment: String,
    ) -> Result<Comment, String> {
        let url = format!("{}/api/now/table/sys_journal_field", self.get_base_url()?);
        let auth_header = self.create_auth_header()?;

        let payload = serde_json::json!({
            "element": "comments",
            "element_id": incident_sys_id,
            "value": comment
        });

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), auth_header);
        headers.insert("Content-Type".to_string(), "application/json".to_string());

        let response = HttpClient::request(&url, HttpMethod::Post)
            .headers(headers)
            .json(&payload)
            .send()
            .map_err(|err| err.to_string())?;

        let response_text = response.text();

        #[derive(Deserialize)]
        struct ServiceNowResponse {
            result: Comment,
        }

        let sn_response: ServiceNowResponse =
            serde_json::from_str(&response_text).map_err(|err| {
                format!(
                    "Failed to parse response: {}. Response was: {}",
                    err, response_text
                )
            })?;

        Ok(sn_response.result)
    }

    #[query]
    async fn resolve_incident(
        &self,
        sys_id: String,
        resolution_notes: String,
    ) -> Result<Incident, String> {
        let url = format!("{}/api/now/table/incident/{}", self.get_base_url()?, sys_id);
        let auth_header = self.create_auth_header()?;

        let payload = serde_json::json!({
            "state": "6", // Resolved state
            "close_notes": resolution_notes
        });

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), auth_header);
        headers.insert("Content-Type".to_string(), "application/json".to_string());

        let response = HttpClient::request(&url, HttpMethod::Put)
            .headers(headers)
            .json(&payload)
            .send()
            .map_err(|err| err.to_string())?;

        let response_text = response.text();

        #[derive(Deserialize)]
        struct ServiceNowResponse {
            result: Incident,
        }

        let sn_response: ServiceNowResponse =
            serde_json::from_str(&response_text).map_err(|err| {
                format!(
                    "Failed to parse response: {}. Response was: {}",
                    err, response_text
                )
            })?;

        Ok(sn_response.result)
    }

    #[query]
    async fn list_incidents(
        &self,
        query_str: Option<String>,
        limit: Option<u32>,
    ) -> Result<Vec<Incident>, String> {
        let query = query_str.unwrap_or_default();
        let limit_val = limit.unwrap_or(100);
        self.query_incidents(query, limit_val).await
    }

    // Service Catalog Functions
    #[query]
    async fn list_catalog_items(
        &self,
        query_str: Option<String>,
        limit: Option<u32>,
    ) -> Result<Vec<CatalogItem>, String> {
        let url = format!("{}/api/now/table/sc_cat_item", self.get_base_url()?);
        let auth_header = self.create_auth_header()?;

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), auth_header);

        let mut query_params = Vec::new();
        if let Some(query) = query_str {
            query_params.push(("sysparm_query".to_string(), query));
        }
        query_params.push((
            "sysparm_limit".to_string(),
            limit.unwrap_or(100).to_string(),
        ));

        let response = HttpClient::request(&url, HttpMethod::Get)
            .headers(headers)
            .query(query_params)
            .send()
            .map_err(|err| err.to_string())?;

        let response_text = response.text();

        #[derive(Deserialize)]
        struct ServiceNowResponse {
            result: Vec<CatalogItem>,
        }

        let sn_response: ServiceNowResponse =
            serde_json::from_str(&response_text).map_err(|err| {
                format!(
                    "Failed to parse response: {}. Response was: {}",
                    err, response_text
                )
            })?;

        Ok(sn_response.result)
    }

    #[query]
    async fn get_catalog_item(&self, sys_id: String) -> Result<CatalogItem, String> {
        let url = format!(
            "{}/api/now/table/sc_cat_item/{}",
            self.get_base_url()?,
            sys_id
        );
        let auth_header = self.create_auth_header()?;

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), auth_header);

        let response = HttpClient::request(&url, HttpMethod::Get)
            .headers(headers)
            .send()
            .map_err(|err| err.to_string())?;

        let response_text = response.text();

        #[derive(Deserialize)]
        struct ServiceNowResponse {
            result: CatalogItem,
        }

        let sn_response: ServiceNowResponse =
            serde_json::from_str(&response_text).map_err(|err| {
                format!(
                    "Failed to parse response: {}. Response was: {}",
                    err, response_text
                )
            })?;

        Ok(sn_response.result)
    }

    #[query]
    async fn list_catalog_categories(
        &self,
        query_str: Option<String>,
        limit: Option<u32>,
    ) -> Result<Vec<CatalogCategory>, String> {
        let url = format!("{}/api/now/table/sc_category", self.get_base_url()?);
        let auth_header = self.create_auth_header()?;

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), auth_header);

        let mut query_params = Vec::new();
        if let Some(query) = query_str {
            query_params.push(("sysparm_query".to_string(), query));
        }
        query_params.push((
            "sysparm_limit".to_string(),
            limit.unwrap_or(100).to_string(),
        ));

        let response = HttpClient::request(&url, HttpMethod::Get)
            .headers(headers)
            .query(query_params)
            .send()
            .map_err(|err| err.to_string())?;

        let response_text = response.text();

        #[derive(Deserialize)]
        struct ServiceNowResponse {
            result: Vec<CatalogCategory>,
        }

        let sn_response: ServiceNowResponse =
            serde_json::from_str(&response_text).map_err(|err| {
                format!(
                    "Failed to parse response: {}. Response was: {}",
                    err, response_text
                )
            })?;

        Ok(sn_response.result)
    }

    #[query]
    async fn create_catalog_category(
        &self,
        title: String,
        description: Option<String>,
        parent: Option<String>,
    ) -> Result<CatalogCategory, String> {
        let url = format!("{}/api/now/table/sc_category", self.get_base_url()?);
        let auth_header = self.create_auth_header()?;

        let mut payload = serde_json::json!({
            "title": title
        });

        if let Some(desc) = description {
            payload["description"] = serde_json::Value::String(desc);
        }
        if let Some(parent_id) = parent {
            payload["parent"] = serde_json::Value::String(parent_id);
        }

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), auth_header);
        headers.insert("Content-Type".to_string(), "application/json".to_string());

        let response = HttpClient::request(&url, HttpMethod::Post)
            .headers(headers)
            .json(&payload)
            .send()
            .map_err(|err| err.to_string())?;

        let response_text = response.text();

        #[derive(Deserialize)]
        struct ServiceNowResponse {
            result: CatalogCategory,
        }

        let sn_response: ServiceNowResponse =
            serde_json::from_str(&response_text).map_err(|err| {
                format!(
                    "Failed to parse response: {}. Response was: {}",
                    err, response_text
                )
            })?;

        Ok(sn_response.result)
    }

    #[query]
    async fn move_catalog_items(
        &self,
        item_sys_ids: Vec<String>,
        target_category_sys_id: String,
    ) -> Result<(), String> {
        for item_sys_id in item_sys_ids {
            let url = format!(
                "{}/api/now/table/sc_cat_item/{}",
                self.get_base_url()?,
                item_sys_id
            );
            let auth_header = self.create_auth_header()?;

            let payload = serde_json::json!({
                "category": target_category_sys_id
            });

            let mut headers = HashMap::new();
            headers.insert("Authorization".to_string(), auth_header);
            headers.insert("Content-Type".to_string(), "application/json".to_string());

            let _response = HttpClient::request(&url, HttpMethod::Put)
                .headers(headers)
                .json(&payload)
                .send()
                .map_err(|err| err.to_string())?;
        }
        Ok(())
    }

    #[query]
    async fn create_catalog_item_variable(
        &self,
        catalog_item_sys_id: String,
        name: String,
        question_text: String,
        var_type: String,
        mandatory: bool,
    ) -> Result<CatalogVariable, String> {
        let url = format!("{}/api/now/table/item_option_new", self.get_base_url()?);
        let auth_header = self.create_auth_header()?;

        let payload = serde_json::json!({
            "name": name,
            "question_text": question_text,
            "type": var_type,
            "mandatory": mandatory,
            "catalog_item": catalog_item_sys_id
        });

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), auth_header);
        headers.insert("Content-Type".to_string(), "application/json".to_string());

        let response = HttpClient::request(&url, HttpMethod::Post)
            .headers(headers)
            .json(&payload)
            .send()
            .map_err(|err| err.to_string())?;

        let response_text = response.text();

        #[derive(Deserialize)]
        struct ServiceNowResponse {
            result: CatalogVariable,
        }

        let sn_response: ServiceNowResponse =
            serde_json::from_str(&response_text).map_err(|err| {
                format!(
                    "Failed to parse response: {}. Response was: {}",
                    err, response_text
                )
            })?;

        Ok(sn_response.result)
    }

    #[query]
    async fn list_catalog_item_variables(
        &self,
        catalog_item_sys_id: String,
    ) -> Result<Vec<CatalogVariable>, String> {
        let url = format!("{}/api/now/table/item_option_new", self.get_base_url()?);
        let auth_header = self.create_auth_header()?;

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), auth_header);

        let query_params = vec![
            (
                "sysparm_query".to_string(),
                format!("catalog_item={}", catalog_item_sys_id),
            ),
            ("sysparm_limit".to_string(), "100".to_string()),
        ];

        let response = HttpClient::request(&url, HttpMethod::Get)
            .headers(headers)
            .query(query_params)
            .send()
            .map_err(|err| err.to_string())?;

        let response_text = response.text();

        #[derive(Deserialize)]
        struct ServiceNowResponse {
            result: Vec<CatalogVariable>,
        }

        let sn_response: ServiceNowResponse =
            serde_json::from_str(&response_text).map_err(|err| {
                format!(
                    "Failed to parse response: {}. Response was: {}",
                    err, response_text
                )
            })?;

        Ok(sn_response.result)
    }

    #[query]
    async fn list_catalogs(
        &self,
        query_str: Option<String>,
        limit: Option<u32>,
    ) -> Result<Vec<serde_json::Value>, String> {
        let url = format!("{}/api/now/table/sc_catalog", self.get_base_url()?);
        let auth_header = self.create_auth_header()?;

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), auth_header);

        let mut query_params = Vec::new();
        if let Some(query) = query_str {
            query_params.push(("sysparm_query".to_string(), query));
        }
        query_params.push((
            "sysparm_limit".to_string(),
            limit.unwrap_or(100).to_string(),
        ));

        let response = HttpClient::request(&url, HttpMethod::Get)
            .headers(headers)
            .query(query_params)
            .send()
            .map_err(|err| err.to_string())?;

        let response_text = response.text();

        #[derive(Deserialize)]
        struct ServiceNowResponse {
            result: Vec<serde_json::Value>,
        }

        let sn_response: ServiceNowResponse =
            serde_json::from_str(&response_text).map_err(|err| {
                format!(
                    "Failed to parse response: {}. Response was: {}",
                    err, response_text
                )
            })?;

        Ok(sn_response.result)
    }

    // Catalog Optimization Functions
    #[query]
    async fn get_optimization_recommendations(
        &self,
        catalog_item_sys_id: Option<String>,
    ) -> Result<Vec<serde_json::Value>, String> {
        let url = format!(
            "{}/api/now/table/sc_cat_item_optimization",
            self.get_base_url()?
        );
        let auth_header = self.create_auth_header()?;

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), auth_header);

        let mut query_params = Vec::new();
        if let Some(item_id) = catalog_item_sys_id {
            query_params.push((
                "sysparm_query".to_string(),
                format!("catalog_item={}", item_id),
            ));
        }
        query_params.push(("sysparm_limit".to_string(), "100".to_string()));

        let response = HttpClient::request(&url, HttpMethod::Get)
            .headers(headers)
            .query(query_params)
            .send()
            .map_err(|err| err.to_string())?;

        let response_text = response.text();

        #[derive(Deserialize)]
        struct ServiceNowResponse {
            result: Vec<serde_json::Value>,
        }

        let sn_response: ServiceNowResponse =
            serde_json::from_str(&response_text).map_err(|err| {
                format!(
                    "Failed to parse response: {}. Response was: {}",
                    err, response_text
                )
            })?;

        Ok(sn_response.result)
    }

    // Change Management Functions
    #[query]
    async fn create_change_request(
        &self,
        short_description: String,
        description: String,
        priority: String,
        risk: Option<String>,
        impact: Option<String>,
    ) -> Result<ChangeRequest, String> {
        let url = format!("{}/api/now/table/change_request", self.get_base_url()?);
        let auth_header = self.create_auth_header()?;

        let mut payload = serde_json::json!({
            "short_description": short_description,
            "description": description,
            "priority": priority
        });

        if let Some(risk_val) = risk {
            payload["risk"] = serde_json::Value::String(risk_val);
        }
        if let Some(impact_val) = impact {
            payload["impact"] = serde_json::Value::String(impact_val);
        }

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), auth_header);
        headers.insert("Content-Type".to_string(), "application/json".to_string());

        let response = HttpClient::request(&url, HttpMethod::Post)
            .headers(headers)
            .json(&payload)
            .send()
            .map_err(|err| err.to_string())?;

        let response_text = response.text();

        #[derive(Deserialize)]
        struct ServiceNowResponse {
            result: ChangeRequest,
        }

        let sn_response: ServiceNowResponse =
            serde_json::from_str(&response_text).map_err(|err| {
                format!(
                    "Failed to parse response: {}. Response was: {}",
                    err, response_text
                )
            })?;

        Ok(sn_response.result)
    }

    #[query]
    async fn list_change_requests(
        &self,
        query_str: Option<String>,
        limit: Option<u32>,
    ) -> Result<Vec<ChangeRequest>, String> {
        let url = format!("{}/api/now/table/change_request", self.get_base_url()?);
        let auth_header = self.create_auth_header()?;

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), auth_header);

        let mut query_params = Vec::new();
        if let Some(query) = query_str {
            query_params.push(("sysparm_query".to_string(), query));
        }
        query_params.push((
            "sysparm_limit".to_string(),
            limit.unwrap_or(100).to_string(),
        ));

        let response = HttpClient::request(&url, HttpMethod::Get)
            .headers(headers)
            .query(query_params)
            .send()
            .map_err(|err| err.to_string())?;

        let response_text = response.text();

        #[derive(Deserialize)]
        struct ServiceNowResponse {
            result: Vec<ChangeRequest>,
        }

        let sn_response: ServiceNowResponse =
            serde_json::from_str(&response_text).map_err(|err| {
                format!(
                    "Failed to parse response: {}. Response was: {}",
                    err, response_text
                )
            })?;

        Ok(sn_response.result)
    }

    #[query]
    async fn get_change_request_details(&self, sys_id: String) -> Result<ChangeRequest, String> {
        let url = format!(
            "{}/api/now/table/change_request/{}",
            self.get_base_url()?,
            sys_id
        );
        let auth_header = self.create_auth_header()?;

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), auth_header);

        let response = HttpClient::request(&url, HttpMethod::Get)
            .headers(headers)
            .send()
            .map_err(|err| err.to_string())?;

        let response_text = response.text();

        #[derive(Deserialize)]
        struct ServiceNowResponse {
            result: ChangeRequest,
        }

        let sn_response: ServiceNowResponse =
            serde_json::from_str(&response_text).map_err(|err| {
                format!(
                    "Failed to parse response: {}. Response was: {}",
                    err, response_text
                )
            })?;

        Ok(sn_response.result)
    }

    #[query]
    async fn add_change_task(
        &self,
        change_request_sys_id: String,
        short_description: String,
        description: String,
        assigned_to: Option<String>,
    ) -> Result<ChangeTask, String> {
        let url = format!("{}/api/now/table/change_task", self.get_base_url()?);
        let auth_header = self.create_auth_header()?;

        let mut payload = serde_json::json!({
            "short_description": short_description,
            "description": description,
            "change_request": change_request_sys_id
        });

        if let Some(assignee) = assigned_to {
            payload["assigned_to"] = serde_json::Value::String(assignee);
        }

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), auth_header);
        headers.insert("Content-Type".to_string(), "application/json".to_string());

        let response = HttpClient::request(&url, HttpMethod::Post)
            .headers(headers)
            .json(&payload)
            .send()
            .map_err(|err| err.to_string())?;

        let response_text = response.text();

        #[derive(Deserialize)]
        struct ServiceNowResponse {
            result: ChangeTask,
        }

        let sn_response: ServiceNowResponse =
            serde_json::from_str(&response_text).map_err(|err| {
                format!(
                    "Failed to parse response: {}. Response was: {}",
                    err, response_text
                )
            })?;

        Ok(sn_response.result)
    }

    #[query]
    async fn submit_change_for_approval(&self, sys_id: String) -> Result<ChangeRequest, String> {
        let url = format!(
            "{}/api/now/table/change_request/{}",
            self.get_base_url()?,
            sys_id
        );
        let auth_header = self.create_auth_header()?;

        let payload = serde_json::json!({
            "state": "1" // Submitted for approval
        });

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), auth_header);
        headers.insert("Content-Type".to_string(), "application/json".to_string());

        let response = HttpClient::request(&url, HttpMethod::Put)
            .headers(headers)
            .json(&payload)
            .send()
            .map_err(|err| err.to_string())?;

        let response_text = response.text();

        #[derive(Deserialize)]
        struct ServiceNowResponse {
            result: ChangeRequest,
        }

        let sn_response: ServiceNowResponse =
            serde_json::from_str(&response_text).map_err(|err| {
                format!(
                    "Failed to parse response: {}. Response was: {}",
                    err, response_text
                )
            })?;

        Ok(sn_response.result)
    }

    #[query]
    async fn approve_change(
        &self,
        sys_id: String,
        approval_notes: Option<String>,
    ) -> Result<ChangeRequest, String> {
        let url = format!(
            "{}/api/now/table/change_request/{}",
            self.get_base_url()?,
            sys_id
        );
        let auth_header = self.create_auth_header()?;

        let mut payload = serde_json::json!({
            "state": "2" // Approved
        });

        if let Some(notes) = approval_notes {
            payload["approval_notes"] = serde_json::Value::String(notes);
        }

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), auth_header);
        headers.insert("Content-Type".to_string(), "application/json".to_string());

        let response = HttpClient::request(&url, HttpMethod::Put)
            .headers(headers)
            .json(&payload)
            .send()
            .map_err(|err| err.to_string())?;

        let response_text = response.text();

        #[derive(Deserialize)]
        struct ServiceNowResponse {
            result: ChangeRequest,
        }

        let sn_response: ServiceNowResponse =
            serde_json::from_str(&response_text).map_err(|err| {
                format!(
                    "Failed to parse response: {}. Response was: {}",
                    err, response_text
                )
            })?;

        Ok(sn_response.result)
    }

    #[query]
    async fn reject_change(
        &self,
        sys_id: String,
        rejection_notes: String,
    ) -> Result<ChangeRequest, String> {
        let url = format!(
            "{}/api/now/table/change_request/{}",
            self.get_base_url()?,
            sys_id
        );
        let auth_header = self.create_auth_header()?;

        let payload = serde_json::json!({
            "state": "3", // Rejected
            "rejection_notes": rejection_notes
        });

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), auth_header);
        headers.insert("Content-Type".to_string(), "application/json".to_string());

        let response = HttpClient::request(&url, HttpMethod::Put)
            .headers(headers)
            .json(&payload)
            .send()
            .map_err(|err| err.to_string())?;

        let response_text = response.text();

        #[derive(Deserialize)]
        struct ServiceNowResponse {
            result: ChangeRequest,
        }

        let sn_response: ServiceNowResponse =
            serde_json::from_str(&response_text).map_err(|err| {
                format!(
                    "Failed to parse response: {}. Response was: {}",
                    err, response_text
                )
            })?;

        Ok(sn_response.result)
    }

    // Agile Story Management Functions
    #[query]
    async fn create_story(
        &self,
        short_description: String,
        description: String,
        priority: Option<String>,
        story_points: Option<String>,
        epic_sys_id: Option<String>,
    ) -> Result<Story, String> {
        let url = format!("{}/api/now/table/rm_story", self.get_base_url()?);
        let auth_header = self.create_auth_header()?;

        let mut payload = serde_json::json!({
            "short_description": short_description,
            "description": description
        });

        if let Some(pri) = priority {
            payload["priority"] = serde_json::Value::String(pri);
        }
        if let Some(points) = story_points {
            payload["story_points"] = serde_json::Value::String(points);
        }
        if let Some(epic) = epic_sys_id {
            payload["epic"] = serde_json::Value::String(epic);
        }

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), auth_header);
        headers.insert("Content-Type".to_string(), "application/json".to_string());

        let response = HttpClient::request(&url, HttpMethod::Post)
            .headers(headers)
            .json(&payload)
            .send()
            .map_err(|err| err.to_string())?;

        let response_text = response.text();

        #[derive(Deserialize)]
        struct ServiceNowResponse {
            result: Story,
        }

        let sn_response: ServiceNowResponse =
            serde_json::from_str(&response_text).map_err(|err| {
                format!(
                    "Failed to parse response: {}. Response was: {}",
                    err, response_text
                )
            })?;

        Ok(sn_response.result)
    }

    #[query]
    async fn list_stories(
        &self,
        query_str: Option<String>,
        limit: Option<u32>,
    ) -> Result<Vec<Story>, String> {
        let url = format!("{}/api/now/table/rm_story", self.get_base_url()?);
        let auth_header = self.create_auth_header()?;

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), auth_header);

        let mut query_params = Vec::new();
        if let Some(query) = query_str {
            query_params.push(("sysparm_query".to_string(), query));
        }
        query_params.push((
            "sysparm_limit".to_string(),
            limit.unwrap_or(100).to_string(),
        ));

        let response = HttpClient::request(&url, HttpMethod::Get)
            .headers(headers)
            .query(query_params)
            .send()
            .map_err(|err| err.to_string())?;

        let response_text = response.text();

        #[derive(Deserialize)]
        struct ServiceNowResponse {
            result: Vec<Story>,
        }

        let sn_response: ServiceNowResponse =
            serde_json::from_str(&response_text).map_err(|err| {
                format!(
                    "Failed to parse response: {}. Response was: {}",
                    err, response_text
                )
            })?;

        Ok(sn_response.result)
    }

    #[query]
    async fn delete_story_dependency(&self, dependency_sys_id: String) -> Result<(), String> {
        let url = format!(
            "{}/api/now/table/rm_story_dependency/{}",
            self.get_base_url()?,
            dependency_sys_id
        );
        let auth_header = self.create_auth_header()?;

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), auth_header);

        let response = HttpClient::request(&url, HttpMethod::Delete)
            .headers(headers)
            .send()
            .map_err(|err| err.to_string())?;

        if response.status() >= 204 && response.status() < 300 {
            Ok(())
        } else {
            Err(format!(
                "Failed to delete story dependency. Status: {}",
                response.status()
            ))
        }
    }

    // Agile Epic Management Functions
    #[query]
    async fn create_epic(
        &self,
        short_description: String,
        description: String,
        priority: Option<String>,
    ) -> Result<Epic, String> {
        let url = format!("{}/api/now/table/rm_epic", self.get_base_url()?);
        let auth_header = self.create_auth_header()?;

        let mut payload = serde_json::json!({
            "short_description": short_description,
            "description": description
        });

        if let Some(pri) = priority {
            payload["priority"] = serde_json::Value::String(pri);
        }

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), auth_header);
        headers.insert("Content-Type".to_string(), "application/json".to_string());

        let response = HttpClient::request(&url, HttpMethod::Post)
            .headers(headers)
            .json(&payload)
            .send()
            .map_err(|err| err.to_string())?;

        let response_text = response.text();

        #[derive(Deserialize)]
        struct ServiceNowResponse {
            result: Epic,
        }

        let sn_response: ServiceNowResponse =
            serde_json::from_str(&response_text).map_err(|err| {
                format!(
                    "Failed to parse response: {}. Response was: {}",
                    err, response_text
                )
            })?;

        Ok(sn_response.result)
    }

    #[query]
    async fn list_epics(
        &self,
        query_str: Option<String>,
        limit: Option<u32>,
    ) -> Result<Vec<Epic>, String> {
        let url = format!("{}/api/now/table/rm_epic", self.get_base_url()?);
        let auth_header = self.create_auth_header()?;

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), auth_header);

        let mut query_params = Vec::new();
        if let Some(query) = query_str {
            query_params.push(("sysparm_query".to_string(), query));
        }
        query_params.push((
            "sysparm_limit".to_string(),
            limit.unwrap_or(100).to_string(),
        ));

        let response = HttpClient::request(&url, HttpMethod::Get)
            .headers(headers)
            .query(query_params)
            .send()
            .map_err(|err| err.to_string())?;

        let response_text = response.text();

        #[derive(Deserialize)]
        struct ServiceNowResponse {
            result: Vec<Epic>,
        }

        let sn_response: ServiceNowResponse =
            serde_json::from_str(&response_text).map_err(|err| {
                format!(
                    "Failed to parse response: {}. Response was: {}",
                    err, response_text
                )
            })?;

        Ok(sn_response.result)
    }

    // Scrum Task Management Functions
    #[query]
    async fn create_scrum_task(
        &self,
        short_description: String,
        description: String,
        story_sys_id: Option<String>,
        assigned_to: Option<String>,
    ) -> Result<ScrumTask, String> {
        let url = format!("{}/api/now/table/rm_scrum_task", self.get_base_url()?);
        let auth_header = self.create_auth_header()?;

        let mut payload = serde_json::json!({
            "short_description": short_description,
            "description": description
        });

        if let Some(story) = story_sys_id {
            payload["story"] = serde_json::Value::String(story);
        }
        if let Some(assignee) = assigned_to {
            payload["assigned_to"] = serde_json::Value::String(assignee);
        }

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), auth_header);
        headers.insert("Content-Type".to_string(), "application/json".to_string());

        let response = HttpClient::request(&url, HttpMethod::Post)
            .headers(headers)
            .json(&payload)
            .send()
            .map_err(|err| err.to_string())?;

        let response_text = response.text();

        #[derive(Deserialize)]
        struct ServiceNowResponse {
            result: ScrumTask,
        }

        let sn_response: ServiceNowResponse =
            serde_json::from_str(&response_text).map_err(|err| {
                format!(
                    "Failed to parse response: {}. Response was: {}",
                    err, response_text
                )
            })?;

        Ok(sn_response.result)
    }

    #[query]
    async fn list_scrum_tasks(
        &self,
        query_str: Option<String>,
        limit: Option<u32>,
    ) -> Result<Vec<ScrumTask>, String> {
        let url = format!("{}/api/now/table/rm_scrum_task", self.get_base_url()?);
        let auth_header = self.create_auth_header()?;

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), auth_header);

        let mut query_params = Vec::new();
        if let Some(query) = query_str {
            query_params.push(("sysparm_query".to_string(), query));
        }
        query_params.push((
            "sysparm_limit".to_string(),
            limit.unwrap_or(100).to_string(),
        ));

        let response = HttpClient::request(&url, HttpMethod::Get)
            .headers(headers)
            .query(query_params)
            .send()
            .map_err(|err| err.to_string())?;

        let response_text = response.text();

        #[derive(Deserialize)]
        struct ServiceNowResponse {
            result: Vec<ScrumTask>,
        }

        let sn_response: ServiceNowResponse =
            serde_json::from_str(&response_text).map_err(|err| {
                format!(
                    "Failed to parse response: {}. Response was: {}",
                    err, response_text
                )
            })?;

        Ok(sn_response.result)
    }

    // Project Management Functions
    #[query]
    async fn create_project(
        &self,
        name: String,
        short_description: String,
        goal: Option<String>,
    ) -> Result<Project, String> {
        let url = format!("{}/api/now/table/promin_project", self.get_base_url()?);
        let auth_header = self.create_auth_header()?;

        let mut payload = serde_json::json!({
            "name": name,
            "short_description": short_description
        });

        if let Some(goal_val) = goal {
            payload["goal"] = serde_json::Value::String(goal_val);
        }

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), auth_header);
        headers.insert("Content-Type".to_string(), "application/json".to_string());

        let response = HttpClient::request(&url, HttpMethod::Post)
            .headers(headers)
            .json(&payload)
            .send()
            .map_err(|err| err.to_string())?;

        let response_text = response.text();

        #[derive(Deserialize)]
        struct ServiceNowResponse {
            result: Project,
        }

        let sn_response: ServiceNowResponse =
            serde_json::from_str(&response_text).map_err(|err| {
                format!(
                    "Failed to parse response: {}. Response was: {}",
                    err, response_text
                )
            })?;

        Ok(sn_response.result)
    }

    #[query]
    async fn list_projects(
        &self,
        query_str: Option<String>,
        limit: Option<u32>,
    ) -> Result<Vec<Project>, String> {
        let url = format!("{}/api/now/table/promin_project", self.get_base_url()?);
        let auth_header = self.create_auth_header()?;

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), auth_header);

        let mut query_params = Vec::new();
        if let Some(query) = query_str {
            query_params.push(("sysparm_query".to_string(), query));
        }
        query_params.push((
            "sysparm_limit".to_string(),
            limit.unwrap_or(100).to_string(),
        ));

        let response = HttpClient::request(&url, HttpMethod::Get)
            .headers(headers)
            .query(query_params)
            .send()
            .map_err(|err| err.to_string())?;

        let response_text = response.text();

        #[derive(Deserialize)]
        struct ServiceNowResponse {
            result: Vec<Project>,
        }

        let sn_response: ServiceNowResponse =
            serde_json::from_str(&response_text).map_err(|err| {
                format!(
                    "Failed to parse response: {}. Response was: {}",
                    err, response_text
                )
            })?;

        Ok(sn_response.result)
    }

    // Workflow Management Functions
    #[query]
    async fn list_workflows(
        &self,
        query_str: Option<String>,
        limit: Option<u32>,
    ) -> Result<Vec<Workflow>, String> {
        let url = format!("{}/api/now/table/wf_workflow", self.get_base_url()?);
        let auth_header = self.create_auth_header()?;

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), auth_header);

        let mut query_params = Vec::new();
        if let Some(query) = query_str {
            query_params.push(("sysparm_query".to_string(), query));
        }
        query_params.push((
            "sysparm_limit".to_string(),
            limit.unwrap_or(100).to_string(),
        ));

        let response = HttpClient::request(&url, HttpMethod::Get)
            .headers(headers)
            .query(query_params)
            .send()
            .map_err(|err| err.to_string())?;

        let response_text = response.text();

        #[derive(Deserialize)]
        struct ServiceNowResponse {
            result: Vec<Workflow>,
        }

        let sn_response: ServiceNowResponse =
            serde_json::from_str(&response_text).map_err(|err| {
                format!(
                    "Failed to parse response: {}. Response was: {}",
                    err, response_text
                )
            })?;

        Ok(sn_response.result)
    }

    #[query]
    async fn get_workflow(&self, sys_id: String) -> Result<Workflow, String> {
        let url = format!(
            "{}/api/now/table/wf_workflow/{}",
            self.get_base_url()?,
            sys_id
        );
        let auth_header = self.create_auth_header()?;

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), auth_header);

        let response = HttpClient::request(&url, HttpMethod::Get)
            .headers(headers)
            .send()
            .map_err(|err| err.to_string())?;

        let response_text = response.text();

        #[derive(Deserialize)]
        struct ServiceNowResponse {
            result: Workflow,
        }

        let sn_response: ServiceNowResponse =
            serde_json::from_str(&response_text).map_err(|err| {
                format!(
                    "Failed to parse response: {}. Response was: {}",
                    err, response_text
                )
            })?;

        Ok(sn_response.result)
    }

    #[query]
    async fn create_workflow(
        &self,
        name: String,
        description: Option<String>,
        table: String,
    ) -> Result<Workflow, String> {
        let url = format!("{}/api/now/table/wf_workflow", self.get_base_url()?);
        let auth_header = self.create_auth_header()?;

        let mut payload = serde_json::json!({
            "name": name,
            "table": table
        });

        if let Some(desc) = description {
            payload["description"] = serde_json::Value::String(desc);
        }

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), auth_header);
        headers.insert("Content-Type".to_string(), "application/json".to_string());

        let response = HttpClient::request(&url, HttpMethod::Post)
            .headers(headers)
            .json(&payload)
            .send()
            .map_err(|err| err.to_string())?;

        let response_text = response.text();

        #[derive(Deserialize)]
        struct ServiceNowResponse {
            result: Workflow,
        }

        let sn_response: ServiceNowResponse =
            serde_json::from_str(&response_text).map_err(|err| {
                format!(
                    "Failed to parse response: {}. Response was: {}",
                    err, response_text
                )
            })?;

        Ok(sn_response.result)
    }

    #[query]
    async fn delete_workflow(&self, sys_id: String) -> Result<(), String> {
        let url = format!(
            "{}/api/now/table/wf_workflow/{}",
            self.get_base_url()?,
            sys_id
        );
        let auth_header = self.create_auth_header()?;

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), auth_header);

        let response = HttpClient::request(&url, HttpMethod::Delete)
            .headers(headers)
            .send()
            .map_err(|err| err.to_string())?;

        if response.status() >= 204 && response.status() < 300 {
            Ok(())
        } else {
            Err(format!(
                "Failed to delete workflow. Status: {}",
                response.status()
            ))
        }
    }

    // Script Include Management Functions
    #[query]
    async fn list_script_includes(
        &self,
        query_str: Option<String>,
        limit: Option<u32>,
    ) -> Result<Vec<ScriptInclude>, String> {
        let url = format!("{}/api/now/table/sys_script_include", self.get_base_url()?);
        let auth_header = self.create_auth_header()?;

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), auth_header);

        let mut query_params = Vec::new();
        if let Some(query) = query_str {
            query_params.push(("sysparm_query".to_string(), query));
        }
        query_params.push((
            "sysparm_limit".to_string(),
            limit.unwrap_or(100).to_string(),
        ));

        let response = HttpClient::request(&url, HttpMethod::Get)
            .headers(headers)
            .query(query_params)
            .send()
            .map_err(|err| err.to_string())?;

        let response_text = response.text();

        #[derive(Deserialize)]
        struct ServiceNowResponse {
            result: Vec<ScriptInclude>,
        }

        let sn_response: ServiceNowResponse =
            serde_json::from_str(&response_text).map_err(|err| {
                format!(
                    "Failed to parse response: {}. Response was: {}",
                    err, response_text
                )
            })?;

        Ok(sn_response.result)
    }

    #[query]
    async fn get_script_include(&self, sys_id: String) -> Result<ScriptInclude, String> {
        let url = format!(
            "{}/api/now/table/sys_script_include/{}",
            self.get_base_url()?,
            sys_id
        );
        let auth_header = self.create_auth_header()?;

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), auth_header);

        let response = HttpClient::request(&url, HttpMethod::Get)
            .headers(headers)
            .send()
            .map_err(|err| err.to_string())?;

        let response_text = response.text();

        #[derive(Deserialize)]
        struct ServiceNowResponse {
            result: ScriptInclude,
        }

        let sn_response: ServiceNowResponse =
            serde_json::from_str(&response_text).map_err(|err| {
                format!(
                    "Failed to parse response: {}. Response was: {}",
                    err, response_text
                )
            })?;

        Ok(sn_response.result)
    }

    #[query]
    async fn create_script_include(
        &self,
        name: String,
        description: Option<String>,
        script: String,
        api_name: Option<String>,
    ) -> Result<ScriptInclude, String> {
        let url = format!("{}/api/now/table/sys_script_include", self.get_base_url()?);
        let auth_header = self.create_auth_header()?;

        let mut payload = serde_json::json!({
            "name": name,
            "script": script
        });

        if let Some(desc) = description {
            payload["description"] = serde_json::Value::String(desc);
        }
        if let Some(api) = api_name {
            payload["api_name"] = serde_json::Value::String(api);
        }

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), auth_header);
        headers.insert("Content-Type".to_string(), "application/json".to_string());

        let response = HttpClient::request(&url, HttpMethod::Post)
            .headers(headers)
            .json(&payload)
            .send()
            .map_err(|err| err.to_string())?;

        let response_text = response.text();

        #[derive(Deserialize)]
        struct ServiceNowResponse {
            result: ScriptInclude,
        }

        let sn_response: ServiceNowResponse =
            serde_json::from_str(&response_text).map_err(|err| {
                format!(
                    "Failed to parse response: {}. Response was: {}",
                    err, response_text
                )
            })?;

        Ok(sn_response.result)
    }

    #[query]
    async fn delete_script_include(&self, sys_id: String) -> Result<(), String> {
        let url = format!(
            "{}/api/now/table/sys_script_include/{}",
            self.get_base_url()?,
            sys_id
        );
        let auth_header = self.create_auth_header()?;

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), auth_header);

        let response = HttpClient::request(&url, HttpMethod::Delete)
            .headers(headers)
            .send()
            .map_err(|err| err.to_string())?;

        if response.status() >= 204 && response.status() < 300 {
            Ok(())
        } else {
            Err(format!(
                "Failed to delete script include. Status: {}",
                response.status()
            ))
        }
    }

    // Changeset Management Functions
    #[query]
    async fn list_changesets(
        &self,
        query_str: Option<String>,
        limit: Option<u32>,
    ) -> Result<Vec<Changeset>, String> {
        let url = format!("{}/api/now/table/sys_update_set", self.get_base_url()?);
        let auth_header = self.create_auth_header()?;

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), auth_header);

        let mut query_params = Vec::new();
        if let Some(query) = query_str {
            query_params.push(("sysparm_query".to_string(), query));
        }
        query_params.push((
            "sysparm_limit".to_string(),
            limit.unwrap_or(100).to_string(),
        ));

        let response = HttpClient::request(&url, HttpMethod::Get)
            .headers(headers)
            .query(query_params)
            .send()
            .map_err(|err| err.to_string())?;

        let response_text = response.text();

        #[derive(Deserialize)]
        struct ServiceNowResponse {
            result: Vec<Changeset>,
        }

        let sn_response: ServiceNowResponse =
            serde_json::from_str(&response_text).map_err(|err| {
                format!(
                    "Failed to parse response: {}. Response was: {}",
                    err, response_text
                )
            })?;

        Ok(sn_response.result)
    }

    #[query]
    async fn get_changeset_details(&self, sys_id: String) -> Result<Changeset, String> {
        let url = format!(
            "{}/api/now/table/sys_update_set/{}",
            self.get_base_url()?,
            sys_id
        );
        let auth_header = self.create_auth_header()?;

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), auth_header);

        let response = HttpClient::request(&url, HttpMethod::Get)
            .headers(headers)
            .send()
            .map_err(|err| err.to_string())?;

        let response_text = response.text();

        #[derive(Deserialize)]
        struct ServiceNowResponse {
            result: Changeset,
        }

        let sn_response: ServiceNowResponse =
            serde_json::from_str(&response_text).map_err(|err| {
                format!(
                    "Failed to parse response: {}. Response was: {}",
                    err, response_text
                )
            })?;

        Ok(sn_response.result)
    }

    #[query]
    async fn create_changeset(
        &self,
        name: String,
        description: Option<String>,
    ) -> Result<Changeset, String> {
        let url = format!("{}/api/now/table/sys_update_set", self.get_base_url()?);
        let auth_header = self.create_auth_header()?;

        let mut payload = serde_json::json!({
            "name": name
        });

        if let Some(desc) = description {
            payload["description"] = serde_json::Value::String(desc);
        }

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), auth_header);
        headers.insert("Content-Type".to_string(), "application/json".to_string());

        let response = HttpClient::request(&url, HttpMethod::Post)
            .headers(headers)
            .json(&payload)
            .send()
            .map_err(|err| err.to_string())?;

        let response_text = response.text();

        #[derive(Deserialize)]
        struct ServiceNowResponse {
            result: Changeset,
        }

        let sn_response: ServiceNowResponse =
            serde_json::from_str(&response_text).map_err(|err| {
                format!(
                    "Failed to parse response: {}. Response was: {}",
                    err, response_text
                )
            })?;

        Ok(sn_response.result)
    }

    #[query]
    async fn commit_changeset(&self, sys_id: String) -> Result<Changeset, String> {
        let url = format!(
            "{}/api/now/table/sys_update_set/{}",
            self.get_base_url()?,
            sys_id
        );
        let auth_header = self.create_auth_header()?;

        let payload = serde_json::json!({
            "state": "committed"
        });

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), auth_header);
        headers.insert("Content-Type".to_string(), "application/json".to_string());

        let response = HttpClient::request(&url, HttpMethod::Put)
            .headers(headers)
            .json(&payload)
            .send()
            .map_err(|err| err.to_string())?;

        let response_text = response.text();

        #[derive(Deserialize)]
        struct ServiceNowResponse {
            result: Changeset,
        }

        let sn_response: ServiceNowResponse =
            serde_json::from_str(&response_text).map_err(|err| {
                format!(
                    "Failed to parse response: {}. Response was: {}",
                    err, response_text
                )
            })?;

        Ok(sn_response.result)
    }

    #[query]
    async fn publish_changeset(&self, sys_id: String) -> Result<Changeset, String> {
        let url = format!(
            "{}/api/now/table/sys_update_set/{}",
            self.get_base_url()?,
            sys_id
        );
        let auth_header = self.create_auth_header()?;

        let payload = serde_json::json!({
            "state": "published"
        });

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), auth_header);
        headers.insert("Content-Type".to_string(), "application/json".to_string());

        let response = HttpClient::request(&url, HttpMethod::Put)
            .headers(headers)
            .json(&payload)
            .send()
            .map_err(|err| err.to_string())?;

        let response_text = response.text();

        #[derive(Deserialize)]
        struct ServiceNowResponse {
            result: Changeset,
        }

        let sn_response: ServiceNowResponse =
            serde_json::from_str(&response_text).map_err(|err| {
                format!(
                    "Failed to parse response: {}. Response was: {}",
                    err, response_text
                )
            })?;

        Ok(sn_response.result)
    }

    // Knowledge Base Management Functions
    #[query]
    async fn create_knowledge_base(
        &self,
        title: String,
        description: Option<String>,
    ) -> Result<KnowledgeBase, String> {
        let url = format!("{}/api/now/table/kb_knowledge_base", self.get_base_url()?);
        let auth_header = self.create_auth_header()?;

        let mut payload = serde_json::json!({
            "title": title
        });

        if let Some(desc) = description {
            payload["description"] = serde_json::Value::String(desc);
        }

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), auth_header);
        headers.insert("Content-Type".to_string(), "application/json".to_string());

        let response = HttpClient::request(&url, HttpMethod::Post)
            .headers(headers)
            .json(&payload)
            .send()
            .map_err(|err| err.to_string())?;

        let response_text = response.text();

        #[derive(Deserialize)]
        struct ServiceNowResponse {
            result: KnowledgeBase,
        }

        let sn_response: ServiceNowResponse =
            serde_json::from_str(&response_text).map_err(|err| {
                format!(
                    "Failed to parse response: {}. Response was: {}",
                    err, response_text
                )
            })?;

        Ok(sn_response.result)
    }

    #[query]
    async fn list_knowledge_bases(
        &self,
        query_str: Option<String>,
        limit: Option<u32>,
    ) -> Result<Vec<KnowledgeBase>, String> {
        let url = format!("{}/api/now/table/kb_knowledge_base", self.get_base_url()?);
        let auth_header = self.create_auth_header()?;

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), auth_header);

        let mut query_params = Vec::new();
        if let Some(query) = query_str {
            query_params.push(("sysparm_query".to_string(), query));
        }
        query_params.push((
            "sysparm_limit".to_string(),
            limit.unwrap_or(100).to_string(),
        ));

        let response = HttpClient::request(&url, HttpMethod::Get)
            .headers(headers)
            .query(query_params)
            .send()
            .map_err(|err| err.to_string())?;

        let response_text = response.text();

        #[derive(Deserialize)]
        struct ServiceNowResponse {
            result: Vec<KnowledgeBase>,
        }

        let sn_response: ServiceNowResponse =
            serde_json::from_str(&response_text).map_err(|err| {
                format!(
                    "Failed to parse response: {}. Response was: {}",
                    err, response_text
                )
            })?;

        Ok(sn_response.result)
    }

    #[query]
    async fn create_article(
        &self,
        short_description: String,
        text: String,
        knowledge_base_sys_id: String,
        category_sys_id: Option<String>,
    ) -> Result<KnowledgeArticle, String> {
        let url = format!("{}/api/now/table/kb_knowledge", self.get_base_url()?);
        let auth_header = self.create_auth_header()?;

        let mut payload = serde_json::json!({
            "short_description": short_description,
            "text": text,
            "kb_knowledge_base": knowledge_base_sys_id
        });

        if let Some(category) = category_sys_id {
            payload["kb_category"] = serde_json::Value::String(category);
        }

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), auth_header);
        headers.insert("Content-Type".to_string(), "application/json".to_string());

        let response = HttpClient::request(&url, HttpMethod::Post)
            .headers(headers)
            .json(&payload)
            .send()
            .map_err(|err| err.to_string())?;

        let response_text = response.text();

        #[derive(Deserialize)]
        struct ServiceNowResponse {
            result: KnowledgeArticle,
        }

        let sn_response: ServiceNowResponse =
            serde_json::from_str(&response_text).map_err(|err| {
                format!(
                    "Failed to parse response: {}. Response was: {}",
                    err, response_text
                )
            })?;

        Ok(sn_response.result)
    }

    #[query]
    async fn publish_article(&self, sys_id: String) -> Result<KnowledgeArticle, String> {
        let url = format!(
            "{}/api/now/table/kb_knowledge/{}",
            self.get_base_url()?,
            sys_id
        );
        let auth_header = self.create_auth_header()?;

        let payload = serde_json::json!({
            "workflow_state": "published"
        });

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), auth_header);
        headers.insert("Content-Type".to_string(), "application/json".to_string());

        let response = HttpClient::request(&url, HttpMethod::Put)
            .headers(headers)
            .json(&payload)
            .send()
            .map_err(|err| err.to_string())?;

        let response_text = response.text();

        #[derive(Deserialize)]
        struct ServiceNowResponse {
            result: KnowledgeArticle,
        }

        let sn_response: ServiceNowResponse =
            serde_json::from_str(&response_text).map_err(|err| {
                format!(
                    "Failed to parse response: {}. Response was: {}",
                    err, response_text
                )
            })?;

        Ok(sn_response.result)
    }

    #[query]
    async fn list_articles(
        &self,
        query_str: Option<String>,
        limit: Option<u32>,
    ) -> Result<Vec<KnowledgeArticle>, String> {
        let url = format!("{}/api/now/table/kb_knowledge", self.get_base_url()?);
        let auth_header = self.create_auth_header()?;

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), auth_header);

        let mut query_params = Vec::new();
        if let Some(query) = query_str {
            query_params.push(("sysparm_query".to_string(), query));
        }
        query_params.push((
            "sysparm_limit".to_string(),
            limit.unwrap_or(100).to_string(),
        ));

        let response = HttpClient::request(&url, HttpMethod::Get)
            .headers(headers)
            .query(query_params)
            .send()
            .map_err(|err| err.to_string())?;

        let response_text = response.text();

        #[derive(Deserialize)]
        struct ServiceNowResponse {
            result: Vec<KnowledgeArticle>,
        }

        let sn_response: ServiceNowResponse =
            serde_json::from_str(&response_text).map_err(|err| {
                format!(
                    "Failed to parse response: {}. Response was: {}",
                    err, response_text
                )
            })?;

        Ok(sn_response.result)
    }

    #[query]
    async fn get_article(&self, sys_id: String) -> Result<KnowledgeArticle, String> {
        let url = format!(
            "{}/api/now/table/kb_knowledge/{}",
            self.get_base_url()?,
            sys_id
        );
        let auth_header = self.create_auth_header()?;

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), auth_header);

        let response = HttpClient::request(&url, HttpMethod::Get)
            .headers(headers)
            .send()
            .map_err(|err| err.to_string())?;

        let response_text = response.text();

        #[derive(Deserialize)]
        struct ServiceNowResponse {
            result: KnowledgeArticle,
        }

        let sn_response: ServiceNowResponse =
            serde_json::from_str(&response_text).map_err(|err| {
                format!(
                    "Failed to parse response: {}. Response was: {}",
                    err, response_text
                )
            })?;

        Ok(sn_response.result)
    }

    // User Management Functions
    #[query]
    async fn create_user(
        &self,
        user_name: String,
        first_name: String,
        last_name: String,
        email: String,
        department: Option<String>,
    ) -> Result<User, String> {
        let url = format!("{}/api/now/table/sys_user", self.get_base_url()?);
        let auth_header = self.create_auth_header()?;

        let mut payload = serde_json::json!({
            "user_name": user_name,
            "first_name": first_name,
            "last_name": last_name,
            "email": email
        });

        if let Some(dept) = department {
            payload["department"] = serde_json::Value::String(dept);
        }

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), auth_header);
        headers.insert("Content-Type".to_string(), "application/json".to_string());

        let response = HttpClient::request(&url, HttpMethod::Post)
            .headers(headers)
            .json(&payload)
            .send()
            .map_err(|err| err.to_string())?;

        let response_text = response.text();

        #[derive(Deserialize)]
        struct ServiceNowResponse {
            result: User,
        }

        let sn_response: ServiceNowResponse =
            serde_json::from_str(&response_text).map_err(|err| {
                format!(
                    "Failed to parse response: {}. Response was: {}",
                    err, response_text
                )
            })?;

        Ok(sn_response.result)
    }

    #[query]
    async fn get_user(&self, identifier: String) -> Result<User, String> {
        let url = format!("{}/api/now/table/sys_user", self.get_base_url()?);
        let auth_header = self.create_auth_header()?;

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), auth_header);

        // Try to find user by sys_id, user_name, or email
        let query_params = vec![
            (
                "sysparm_query".to_string(),
                format!(
                    "sys_id={} OR user_name={} OR email={}",
                    identifier, identifier, identifier
                ),
            ),
            ("sysparm_limit".to_string(), "1".to_string()),
        ];

        let response = HttpClient::request(&url, HttpMethod::Get)
            .headers(headers)
            .query(query_params)
            .send()
            .map_err(|err| err.to_string())?;

        let response_text = response.text();

        #[derive(Deserialize)]
        struct ServiceNowResponse {
            result: Vec<User>,
        }

        let sn_response: ServiceNowResponse =
            serde_json::from_str(&response_text).map_err(|err| {
                format!(
                    "Failed to parse response: {}. Response was: {}",
                    err, response_text
                )
            })?;

        sn_response
            .result
            .into_iter()
            .next()
            .ok_or_else(|| "User not found".to_string())
    }

    #[query]
    async fn list_users(
        &self,
        query_str: Option<String>,
        limit: Option<u32>,
    ) -> Result<Vec<User>, String> {
        let url = format!("{}/api/now/table/sys_user", self.get_base_url()?);
        let auth_header = self.create_auth_header()?;

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), auth_header);

        let mut query_params = Vec::new();
        if let Some(query) = query_str {
            query_params.push(("sysparm_query".to_string(), query));
        }
        query_params.push((
            "sysparm_limit".to_string(),
            limit.unwrap_or(100).to_string(),
        ));

        let response = HttpClient::request(&url, HttpMethod::Get)
            .headers(headers)
            .query(query_params)
            .send()
            .map_err(|err| err.to_string())?;

        let response_text = response.text();

        #[derive(Deserialize)]
        struct ServiceNowResponse {
            result: Vec<User>,
        }

        let sn_response: ServiceNowResponse =
            serde_json::from_str(&response_text).map_err(|err| {
                format!(
                    "Failed to parse response: {}. Response was: {}",
                    err, response_text
                )
            })?;

        Ok(sn_response.result)
    }

    #[query]
    async fn create_group(
        &self,
        name: String,
        description: Option<String>,
        manager: Option<String>,
    ) -> Result<Group, String> {
        let url = format!("{}/api/now/table/sys_user_group", self.get_base_url()?);
        let auth_header = self.create_auth_header()?;

        let mut payload = serde_json::json!({
            "name": name
        });

        if let Some(desc) = description {
            payload["description"] = serde_json::Value::String(desc);
        }
        if let Some(mgr) = manager {
            payload["manager"] = serde_json::Value::String(mgr);
        }

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), auth_header);
        headers.insert("Content-Type".to_string(), "application/json".to_string());

        let response = HttpClient::request(&url, HttpMethod::Post)
            .headers(headers)
            .json(&payload)
            .send()
            .map_err(|err| err.to_string())?;

        let response_text = response.text();

        #[derive(Deserialize)]
        struct ServiceNowResponse {
            result: Group,
        }

        let sn_response: ServiceNowResponse =
            serde_json::from_str(&response_text).map_err(|err| {
                format!(
                    "Failed to parse response: {}. Response was: {}",
                    err, response_text
                )
            })?;

        Ok(sn_response.result)
    }

    #[query]
    async fn add_group_members(
        &self,
        group_sys_id: String,
        user_sys_ids: Vec<String>,
    ) -> Result<(), String> {
        for user_sys_id in user_sys_ids {
            let url = format!("{}/api/now/table/sys_user_grmember", self.get_base_url()?);
            let auth_header = self.create_auth_header()?;

            let payload = serde_json::json!({
                "group": group_sys_id,
                "user": user_sys_id
            });

            let mut headers = HashMap::new();
            headers.insert("Authorization".to_string(), auth_header);
            headers.insert("Content-Type".to_string(), "application/json".to_string());

            let _response = HttpClient::request(&url, HttpMethod::Post)
                .headers(headers)
                .json(&payload)
                .send()
                .map_err(|err| err.to_string())?;
        }
        Ok(())
    }

    #[query]
    async fn remove_group_members(
        &self,
        group_sys_id: String,
        user_sys_ids: Vec<String>,
    ) -> Result<(), String> {
        for user_sys_id in user_sys_ids {
            let url = format!("{}/api/now/table/sys_user_grmember", self.get_base_url()?);
            let auth_header = self.create_auth_header()?;

            let mut headers = HashMap::new();
            headers.insert("Authorization".to_string(), auth_header);

            let query_params = vec![(
                "sysparm_query".to_string(),
                format!("group={} AND user={}", group_sys_id, user_sys_id),
            )];

            let response = HttpClient::request(&url, HttpMethod::Get)
                .headers(headers)
                .query(query_params)
                .send()
                .map_err(|err| err.to_string())?;

            let response_text = response.text();

            #[derive(Deserialize)]
            struct ServiceNowResponse {
                result: Vec<serde_json::Value>,
            }

            let sn_response: ServiceNowResponse =
                serde_json::from_str(&response_text).map_err(|err| {
                    format!(
                        "Failed to parse response: {}. Response was: {}",
                        err, response_text
                    )
                })?;

            if let Some(member) = sn_response.result.first() {
                if let Some(sys_id) = member.get("sys_id").and_then(|v| v.as_str()) {
                    let delete_url = format!(
                        "{}/api/now/table/sys_user_grmember/{}",
                        self.get_base_url()?,
                        sys_id
                    );
                    let delete_auth_header = self.create_auth_header()?;

                    let mut delete_headers = HashMap::new();
                    delete_headers.insert("Authorization".to_string(), delete_auth_header);

                    let _delete_response = HttpClient::request(&delete_url, HttpMethod::Delete)
                        .headers(delete_headers)
                        .send()
                        .map_err(|err| err.to_string())?;
                }
            }
        }
        Ok(())
    }

    #[query]
    async fn list_groups(
        &self,
        query_str: Option<String>,
        limit: Option<u32>,
    ) -> Result<Vec<Group>, String> {
        let url = format!("{}/api/now/table/sys_user_group", self.get_base_url()?);
        let auth_header = self.create_auth_header()?;

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), auth_header);

        let mut query_params = Vec::new();
        if let Some(query) = query_str {
            query_params.push(("sysparm_query".to_string(), query));
        }
        query_params.push((
            "sysparm_limit".to_string(),
            limit.unwrap_or(100).to_string(),
        ));

        let response = HttpClient::request(&url, HttpMethod::Get)
            .headers(headers)
            .query(query_params)
            .send()
            .map_err(|err| err.to_string())?;

        let response_text = response.text();

        #[derive(Deserialize)]
        struct ServiceNowResponse {
            result: Vec<Group>,
        }

        let sn_response: ServiceNowResponse =
            serde_json::from_str(&response_text).map_err(|err| {
                format!(
                    "Failed to parse response: {}. Response was: {}",
                    err, response_text
                )
            })?;

        Ok(sn_response.result)
    }

    // UI Policy Functions
    #[query]
    async fn create_ui_policy(
        &self,
        name: String,
        description: Option<String>,
        table: String,
        catalog_item_sys_id: Option<String>,
    ) -> Result<UIPolicy, String> {
        let url = format!("{}/api/now/table/sys_ui_policy", self.get_base_url()?);
        let auth_header = self.create_auth_header()?;

        let mut payload = serde_json::json!({
            "name": name,
            "table": table
        });

        if let Some(desc) = description {
            payload["description"] = serde_json::Value::String(desc);
        }
        if let Some(catalog_item) = catalog_item_sys_id {
            payload["catalog_item"] = serde_json::Value::String(catalog_item);
        }

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), auth_header);
        headers.insert("Content-Type".to_string(), "application/json".to_string());

        let response = HttpClient::request(&url, HttpMethod::Post)
            .headers(headers)
            .json(&payload)
            .send()
            .map_err(|err| err.to_string())?;

        let response_text = response.text();

        #[derive(Deserialize)]
        struct ServiceNowResponse {
            result: UIPolicy,
        }

        let sn_response: ServiceNowResponse =
            serde_json::from_str(&response_text).map_err(|err| {
                format!(
                    "Failed to parse response: {}. Response was: {}",
                    err, response_text
                )
            })?;

        Ok(sn_response.result)
    }

    #[query]
    async fn create_ui_policy_action(
        &self,
        ui_policy_sys_id: String,
        name: String,
        description: Option<String>,
        field_name: String,
        action: String,
    ) -> Result<UIPolicyAction, String> {
        let url = format!(
            "{}/api/now/table/sys_ui_policy_action",
            self.get_base_url()?
        );
        let auth_header = self.create_auth_header()?;

        let mut payload = serde_json::json!({
            "name": name,
            "ui_policy": ui_policy_sys_id,
            "field_name": field_name,
            "action": action
        });

        if let Some(desc) = description {
            payload["description"] = serde_json::Value::String(desc);
        }

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), auth_header);
        headers.insert("Content-Type".to_string(), "application/json".to_string());

        let response = HttpClient::request(&url, HttpMethod::Post)
            .headers(headers)
            .json(&payload)
            .send()
            .map_err(|err| err.to_string())?;

        let response_text = response.text();

        #[derive(Deserialize)]
        struct ServiceNowResponse {
            result: UIPolicyAction,
        }

        let sn_response: ServiceNowResponse =
            serde_json::from_str(&response_text).map_err(|err| {
                format!(
                    "Failed to parse response: {}. Response was: {}",
                    err, response_text
                )
            })?;

        Ok(sn_response.result)
    }

    #[query]
    fn tools(&self) -> String {
        r#"[
  {
    "type": "function",
    "function": {
      "name": "create_incident",
      "description": "create an incident on servicenow\n",
      "parameters": {
        "type": "object",
        "properties": {
          "short_description": {
            "type": "string",
            "description": "a short description for the incident\n"
          },
          "description": {
            "type": "string",
            "description": "a description for the incident\n"
          },
          "priority": {
            "type": "string",
            "description": "priority for the incident\n"
          }
        },
        "required": [
          "short_description",
          "description",
          "priority"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "get_incident",
      "description": "get an incident from servicenow\n",
      "parameters": {
        "type": "object",
        "properties": {
          "sys_id": {
            "type": "string",
            "description": "id of the incident\n"
          }
        },
        "required": [
          "sys_id"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "delete_incident",
      "description": "delete an incident on servicenow\n",
      "parameters": {
        "type": "object",
        "properties": {
          "sys_id": {
            "type": "string",
            "description": "id of the incident\n"
          }
        },
        "required": [
          "sys_id"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "query_incidents",
      "description": "query incidents on servicenow\n",
      "parameters": {
        "type": "object",
        "properties": {
          "query_str": {
            "type": "string",
            "description": "the query to run\n"
          },
          "limit": {
            "type": "integer",
            "description": "the limit on the number of results\n"
          }
        },
        "required": [
          "query_str",
          "limit"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "add_comment",
      "description": "add a comment to an incident\n",
      "parameters": {
        "type": "object",
        "properties": {
          "incident_sys_id": {
            "type": "string",
            "description": "system id of the incident\n"
          },
          "comment": {
            "type": "string",
            "description": "comment text to add\n"
          }
        },
        "required": [
          "incident_sys_id",
          "comment"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "resolve_incident",
      "description": "resolve an incident\n",
      "parameters": {
        "type": "object",
        "properties": {
          "sys_id": {
            "type": "string",
            "description": "system id of the incident\n"
          },
          "resolution_notes": {
            "type": "string",
            "description": "resolution notes\n"
          }
        },
        "required": [
          "sys_id",
          "resolution_notes"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "list_incidents",
      "description": "list incidents from servicenow\n",
      "parameters": {
        "type": "object",
        "properties": {
          "query_str": {
            "type": "string",
            "description": "query string (optional)\n"
          },
          "limit": {
            "type": "integer",
            "description": "limit on number of results (optional)\n"
          }
        },
        "required": []
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "list_catalog_items",
      "description": "list service catalog items\n",
      "parameters": {
        "type": "object",
        "properties": {
          "query_str": {
            "type": "string",
            "description": "query string (optional)\n"
          },
          "limit": {
            "type": "integer",
            "description": "limit on number of results (optional)\n"
          }
        },
        "required": []
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "get_catalog_item",
      "description": "get a specific catalog item\n",
      "parameters": {
        "type": "object",
        "properties": {
          "sys_id": {
            "type": "string",
            "description": "system id of the catalog item\n"
          }
        },
        "required": [
          "sys_id"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "list_catalog_categories",
      "description": "list catalog categories\n",
      "parameters": {
        "type": "object",
        "properties": {
          "query_str": {
            "type": "string",
            "description": "query string (optional)\n"
          },
          "limit": {
            "type": "integer",
            "description": "limit on number of results (optional)\n"
          }
        },
        "required": []
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "create_catalog_category",
      "description": "create a new catalog category\n",
      "parameters": {
        "type": "object",
        "properties": {
          "title": {
            "type": "string",
            "description": "category title\n"
          },
          "description": {
            "type": "string",
            "description": "category description (optional)\n"
          },
          "parent_category_id": {
            "type": "string",
            "description": "parent category id (optional)\n"
          }
        },
        "required": [
          "title"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "move_catalog_items",
      "description": "move catalog items between categories\n",
      "parameters": {
        "type": "object",
        "properties": {
          "catalog_item_ids": {
            "type": "array",
            "description": "array of catalog item system ids\n"
          },
          "target_category_id": {
            "type": "string",
            "description": "target category system id\n"
          }
        },
        "required": [
          "catalog_item_ids",
          "target_category_id"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "create_catalog_item_variable",
      "description": "create a new variable for a catalog item\n",
      "parameters": {
        "type": "object",
        "properties": {
          "catalog_item_id": {
            "type": "string",
            "description": "catalog item system id\n"
          },
          "variable_name": {
            "type": "string",
            "description": "variable name\n"
          },
          "question_text": {
            "type": "string",
            "description": "question text for the variable\n"
          },
          "variable_type": {
            "type": "string",
            "description": "variable type\n"
          },
          "is_mandatory": {
            "type": "boolean",
            "description": "whether the variable is mandatory\n"
          }
        },
        "required": [
          "catalog_item_id",
          "variable_name",
          "question_text",
          "variable_type",
          "is_mandatory"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "list_catalog_item_variables",
      "description": "list variables for a catalog item\n",
      "parameters": {
        "type": "object",
        "properties": {
          "catalog_item_id": {
            "type": "string",
            "description": "catalog item system id\n"
          }
        },
        "required": [
          "catalog_item_id"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "list_catalogs",
      "description": "list service catalogs\n",
      "parameters": {
        "type": "object",
        "properties": {
          "query_str": {
            "type": "string",
            "description": "query string (optional)\n"
          },
          "limit": {
            "type": "integer",
            "description": "limit on number of results (optional)\n"
          }
        },
        "required": []
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "get_optimization_recommendations",
      "description": "get optimization recommendations for catalog items\n",
      "parameters": {
        "type": "object",
        "properties": {
          "catalog_item_id": {
            "type": "string",
            "description": "catalog item system id (optional)\n"
          }
        },
        "required": []
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "create_change_request",
      "description": "create a new change request\n",
      "parameters": {
        "type": "object",
        "properties": {
          "short_description": {
            "type": "string",
            "description": "short description\n"
          },
          "description": {
            "type": "string",
            "description": "description\n"
          },
          "priority": {
            "type": "string",
            "description": "priority\n"
          },
          "risk": {
            "type": "string",
            "description": "risk level (optional)\n"
          },
          "impact": {
            "type": "string",
            "description": "impact level (optional)\n"
          }
        },
        "required": [
          "short_description",
          "description",
          "priority"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "list_change_requests",
      "description": "list change requests\n",
      "parameters": {
        "type": "object",
        "properties": {
          "query_str": {
            "type": "string",
            "description": "query string (optional)\n"
          },
          "limit": {
            "type": "integer",
            "description": "limit on number of results (optional)\n"
          }
        },
        "required": []
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "get_change_request_details",
      "description": "get detailed information about a change request\n",
      "parameters": {
        "type": "object",
        "properties": {
          "sys_id": {
            "type": "string",
            "description": "system id of the change request\n"
          }
        },
        "required": [
          "sys_id"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "add_change_task",
      "description": "add a task to a change request\n",
      "parameters": {
        "type": "object",
        "properties": {
          "change_request_sys_id": {
            "type": "string",
            "description": "change request system id\n"
          },
          "short_description": {
            "type": "string",
            "description": "task short description\n"
          },
          "description": {
            "type": "string",
            "description": "task description\n"
          },
          "assigned_to": {
            "type": "string",
            "description": "assigned user (optional)\n"
          }
        },
        "required": [
          "change_request_sys_id",
          "short_description",
          "description"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "submit_change_for_approval",
      "description": "submit a change request for approval\n",
      "parameters": {
        "type": "object",
        "properties": {
          "sys_id": {
            "type": "string",
            "description": "system id of the change request\n"
          }
        },
        "required": [
          "sys_id"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "approve_change",
      "description": "approve a change request\n",
      "parameters": {
        "type": "object",
        "properties": {
          "sys_id": {
            "type": "string",
            "description": "system id of the change request\n"
          },
          "notes": {
            "type": "string",
            "description": "approval notes (optional)\n"
          }
        },
        "required": [
          "sys_id"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "reject_change",
      "description": "reject a change request\n",
      "parameters": {
        "type": "object",
        "properties": {
          "sys_id": {
            "type": "string",
            "description": "system id of the change request\n"
          },
          "notes": {
            "type": "string",
            "description": "rejection notes\n"
          }
        },
        "required": [
          "sys_id",
          "notes"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "create_story",
      "description": "create a new user story\n",
      "parameters": {
        "type": "object",
        "properties": {
          "short_description": {
            "type": "string",
            "description": "short description\n"
          },
          "description": {
            "type": "string",
            "description": "description\n"
          },
          "priority": {
            "type": "string",
            "description": "priority (optional)\n"
          },
          "story_points": {
            "type": "string",
            "description": "story points (optional)\n"
          },
          "epic_sys_id": {
            "type": "string",
            "description": "epic system id (optional)\n"
          }
        },
        "required": [
          "short_description",
          "description"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "list_stories",
      "description": "list user stories\n",
      "parameters": {
        "type": "object",
        "properties": {
          "query_str": {
            "type": "string",
            "description": "query string (optional)\n"
          },
          "limit": {
            "type": "integer",
            "description": "limit on number of results (optional)\n"
          }
        },
        "required": []
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "delete_story_dependency",
      "description": "delete a dependency between stories\n",
      "parameters": {
        "type": "object",
        "properties": {
          "dependency_id": {
            "type": "string",
            "description": "dependency system id\n"
          }
        },
        "required": [
          "dependency_id"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "create_epic",
      "description": "create a new epic\n",
      "parameters": {
        "type": "object",
        "properties": {
          "short_description": {
            "type": "string",
            "description": "short description\n"
          },
          "description": {
            "type": "string",
            "description": "description\n"
          },
          "priority": {
            "type": "string",
            "description": "priority (optional)\n"
          }
        },
        "required": [
          "short_description",
          "description"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "list_epics",
      "description": "list epics\n",
      "parameters": {
        "type": "object",
        "properties": {
          "query_str": {
            "type": "string",
            "description": "query string (optional)\n"
          },
          "limit": {
            "type": "integer",
            "description": "limit on number of results (optional)\n"
          }
        },
        "required": []
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "create_scrum_task",
      "description": "create a new scrum task\n",
      "parameters": {
        "type": "object",
        "properties": {
          "short_description": {
            "type": "string",
            "description": "short description\n"
          },
          "description": {
            "type": "string",
            "description": "description\n"
          },
          "story_id": {
            "type": "string",
            "description": "story system id (optional)\n"
          },
          "assigned_to": {
            "type": "string",
            "description": "assigned user (optional)\n"
          }
        },
        "required": [
          "short_description",
          "description"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "list_scrum_tasks",
      "description": "list scrum tasks\n",
      "parameters": {
        "type": "object",
        "properties": {
          "query_str": {
            "type": "string",
            "description": "query string (optional)\n"
          },
          "limit": {
            "type": "integer",
            "description": "limit on number of results (optional)\n"
          }
        },
        "required": []
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "create_project",
      "description": "create a new project\n",
      "parameters": {
        "type": "object",
        "properties": {
          "name": {
            "type": "string",
            "description": "project name\n"
          },
          "short_description": {
            "type": "string",
            "description": "short description\n"
          },
          "goal": {
            "type": "string",
            "description": "project goal (optional)\n"
          }
        },
        "required": [
          "name",
          "short_description"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "list_projects",
      "description": "list projects\n",
      "parameters": {
        "type": "object",
        "properties": {
          "query_str": {
            "type": "string",
            "description": "query string (optional)\n"
          },
          "limit": {
            "type": "integer",
            "description": "limit on number of results (optional)\n"
          }
        },
        "required": []
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "list_workflows",
      "description": "list workflows\n",
      "parameters": {
        "type": "object",
        "properties": {
          "query_str": {
            "type": "string",
            "description": "query string (optional)\n"
          },
          "limit": {
            "type": "integer",
            "description": "limit on number of results (optional)\n"
          }
        },
        "required": []
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "get_workflow",
      "description": "get a specific workflow\n",
      "parameters": {
        "type": "object",
        "properties": {
          "sys_id": {
            "type": "string",
            "description": "system id of the workflow\n"
          }
        },
        "required": [
          "sys_id"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "create_workflow",
      "description": "create a new workflow\n",
      "parameters": {
        "type": "object",
        "properties": {
          "name": {
            "type": "string",
            "description": "workflow name\n"
          },
          "description": {
            "type": "string",
            "description": "description (optional)\n"
          },
          "table": {
            "type": "string",
            "description": "table name\n"
          }
        },
        "required": [
          "name",
          "table"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "delete_workflow",
      "description": "delete a workflow\n",
      "parameters": {
        "type": "object",
        "properties": {
          "sys_id": {
            "type": "string",
            "description": "system id of the workflow\n"
          }
        },
        "required": [
          "sys_id"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "list_script_includes",
      "description": "list script includes\n",
      "parameters": {
        "type": "object",
        "properties": {
          "query_str": {
            "type": "string",
            "description": "query string (optional)\n"
          },
          "limit": {
            "type": "integer",
            "description": "limit on number of results (optional)\n"
          }
        },
        "required": []
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "get_script_include",
      "description": "get a specific script include\n",
      "parameters": {
        "type": "object",
        "properties": {
          "sys_id": {
            "type": "string",
            "description": "system id of the script include\n"
          }
        },
        "required": [
          "sys_id"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "create_script_include",
      "description": "create a new script include\n",
      "parameters": {
        "type": "object",
        "properties": {
          "name": {
            "type": "string",
            "description": "script include name\n"
          },
          "description": {
            "type": "string",
            "description": "description (optional)\n"
          },
          "script": {
            "type": "string",
            "description": "script content\n"
          },
          "api_name": {
            "type": "string",
            "description": "api name (optional)\n"
          }
        },
        "required": [
          "name",
          "script"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "delete_script_include",
      "description": "delete a script include\n",
      "parameters": {
        "type": "object",
        "properties": {
          "sys_id": {
            "type": "string",
            "description": "system id of the script include\n"
          }
        },
        "required": [
          "sys_id"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "list_changesets",
      "description": "list changesets\n",
      "parameters": {
        "type": "object",
        "properties": {
          "query_str": {
            "type": "string",
            "description": "query string (optional)\n"
          },
          "limit": {
            "type": "integer",
            "description": "limit on number of results (optional)\n"
          }
        },
        "required": []
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "get_changeset_details",
      "description": "get detailed information about a changeset\n",
      "parameters": {
        "type": "object",
        "properties": {
          "sys_id": {
            "type": "string",
            "description": "system id of the changeset\n"
          }
        },
        "required": [
          "sys_id"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "create_changeset",
      "description": "create a new changeset\n",
      "parameters": {
        "type": "object",
        "properties": {
          "name": {
            "type": "string",
            "description": "changeset name\n"
          },
          "description": {
            "type": "string",
            "description": "description (optional)\n"
          }
        },
        "required": [
          "name"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "commit_changeset",
      "description": "commit a changeset\n",
      "parameters": {
        "type": "object",
        "properties": {
          "sys_id": {
            "type": "string",
            "description": "system id of the changeset\n"
          }
        },
        "required": [
          "sys_id"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "publish_changeset",
      "description": "publish a changeset\n",
      "parameters": {
        "type": "object",
        "properties": {
          "sys_id": {
            "type": "string",
            "description": "system id of the changeset\n"
          }
        },
        "required": [
          "sys_id"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "create_knowledge_base",
      "description": "Knowledge Base Management\ncreate a new knowledge base\n",
      "parameters": {
        "type": "object",
        "properties": {
          "title": {
            "type": "string",
            "description": "knowledge base title\n"
          },
          "description": {
            "type": "string",
            "description": "description (optional)\n"
          }
        },
        "required": [
          "title"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "list_knowledge_bases",
      "description": "list knowledge bases\n",
      "parameters": {
        "type": "object",
        "properties": {
          "query_str": {
            "type": "string",
            "description": "query string (optional)\n"
          },
          "limit": {
            "type": "integer",
            "description": "limit on number of results (optional)\n"
          }
        },
        "required": []
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "create_article",
      "description": "create a new knowledge article\n",
      "parameters": {
        "type": "object",
        "properties": {
          "short_description": {
            "type": "string",
            "description": "short description\n"
          },
          "text": {
            "type": "string",
            "description": "article content\n"
          },
          "knowledge_base_sys_id": {
            "type": "string",
            "description": "knowledge base system id\n"
          },
          "category_sys_id": {
            "type": "string",
            "description": "category system id (optional)\n"
          }
        },
        "required": [
          "short_description",
          "text",
          "knowledge_base_sys_id"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "publish_article",
      "description": "publish a knowledge article\n",
      "parameters": {
        "type": "object",
        "properties": {
          "sys_id": {
            "type": "string",
            "description": "system id of the article\n"
          }
        },
        "required": [
          "sys_id"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "list_articles",
      "description": "list knowledge articles\n",
      "parameters": {
        "type": "object",
        "properties": {
          "query_str": {
            "type": "string",
            "description": "query string (optional)\n"
          },
          "limit": {
            "type": "integer",
            "description": "limit on number of results (optional)\n"
          }
        },
        "required": []
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "get_article",
      "description": "get a specific knowledge article\n",
      "parameters": {
        "type": "object",
        "properties": {
          "sys_id": {
            "type": "string",
            "description": "system id of the article\n"
          }
        },
        "required": [
          "sys_id"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "create_user",
      "description": "create a new user\n",
      "parameters": {
        "type": "object",
        "properties": {
          "user_name": {
            "type": "string",
            "description": "username\n"
          },
          "first_name": {
            "type": "string",
            "description": "first name\n"
          },
          "last_name": {
            "type": "string",
            "description": "last name\n"
          },
          "email": {
            "type": "string",
            "description": "email address\n"
          },
          "department": {
            "type": "string",
            "description": "department (optional)\n"
          }
        },
        "required": [
          "user_name",
          "first_name",
          "last_name",
          "email"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "get_user",
      "description": "get a specific user\n",
      "parameters": {
        "type": "object",
        "properties": {
          "identifier": {
            "type": "string",
            "description": "user id, username, or email\n"
          }
        },
        "required": [
          "identifier"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "list_users",
      "description": "list users\n",
      "parameters": {
        "type": "object",
        "properties": {
          "query_str": {
            "type": "string",
            "description": "query string (optional)\n"
          },
          "limit": {
            "type": "integer",
            "description": "limit on number of results (optional)\n"
          }
        },
        "required": []
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "create_group",
      "description": "create a new group\n",
      "parameters": {
        "type": "object",
        "properties": {
          "name": {
            "type": "string",
            "description": "group name\n"
          },
          "description": {
            "type": "string",
            "description": "description (optional)\n"
          },
          "manager": {
            "type": "string",
            "description": "group manager (optional)\n"
          }
        },
        "required": [
          "name"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "add_group_members",
      "description": "add members to a group\n",
      "parameters": {
        "type": "object",
        "properties": {
          "group_id": {
            "type": "string",
            "description": "group system id\n"
          },
          "user_ids": {
            "type": "array",
            "description": "array of user system ids\n"
          }
        },
        "required": [
          "group_id",
          "user_ids"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "remove_group_members",
      "description": "remove members from a group\n",
      "parameters": {
        "type": "object",
        "properties": {
          "group_id": {
            "type": "string",
            "description": "group system id\n"
          },
          "user_ids": {
            "type": "array",
            "description": "array of user system ids\n"
          }
        },
        "required": [
          "group_id",
          "user_ids"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "list_groups",
      "description": "list groups\n",
      "parameters": {
        "type": "object",
        "properties": {
          "query_str": {
            "type": "string",
            "description": "query string (optional)\n"
          },
          "limit": {
            "type": "integer",
            "description": "limit on number of results (optional)\n"
          }
        },
        "required": []
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "create_ui_policy",
      "description": "UI Policy Management\ncreate a ui policy\n",
      "parameters": {
        "type": "object",
        "properties": {
          "name": {
            "type": "string",
            "description": "policy name\n"
          },
          "description": {
            "type": "string",
            "description": "description (optional)\n"
          },
          "table": {
            "type": "string",
            "description": "table name\n"
          },
          "catalog_item_id": {
            "type": "string",
            "description": "catalog item system id (optional)\n"
          }
        },
        "required": [
          "name",
          "table"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "create_ui_policy_action",
      "description": "create a ui policy action\n",
      "parameters": {
        "type": "object",
        "properties": {
          "ui_policy_sys_id": {
            "type": "string",
            "description": "ui policy system id\n"
          },
          "name": {
            "type": "string",
            "description": "action name\n"
          },
          "description": {
            "type": "string",
            "description": "description (optional)\n"
          },
          "field_name": {
            "type": "string",
            "description": "field name\n"
          },
          "action": {
            "type": "string",
            "description": "action type\n"
          }
        },
        "required": [
          "ui_policy_sys_id",
          "name",
          "field_name",
          "action"
        ]
      }
    }
  }
]"#
        .to_string()
    }

    #[query]
    fn prompts(&self) -> String {
        r#"{
  "prompts": []
}"#
        .to_string()
    }
}
