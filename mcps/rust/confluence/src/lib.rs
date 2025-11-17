//! # Confluence Weilchain Applet
//!
//! This module provides an Model Context Protocol server for interacting with the Confluence Cloud REST v2 API.
//! It allows you to manage pages, blog posts, comments, labels, and other Confluence resources
//! in a type-safe and ergonomic way.
//!
//! ## Features
//! - Page management (CRUD operations)
//! - Blog post management
//! - Comment management (footer)
//! - Label management
//! - Space management
//! - Content hierarchy navigation
//! - Table creation and manipulation

mod page_body_types;
mod responses;
mod types;

use page_body_types::{CellAttrs, Content, Document, Node, TableAttrs};
use responses::{CreateBlogPostResponse, CreatePageResponse};
use serde_json::{Value, json};
use types::*;
use weil_rs::runtime::Runtime;

use std::collections::{BTreeMap, HashMap};
use std::u64;

use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use serde::{Deserialize, Serialize};
use weil_macros::{WeilType, constructor, mutate, query, smart_contract};
use weil_rs::config::Secrets;
use weil_rs::http::{HttpClient, HttpMethod};

/// Constants used for API requests and content formatting.
///
/// These are string constants fed into Confluence's REST v2 query params and ADF body
/// representations.
const BODY_FORMAT: &str = "body-format";
const ATLAS_DOC_FORMAT: &str = "atlas_doc_format";
const STORAGE: &str = "storage";
const DOC: &str = "doc";
const TEXT: &str = "text";
const LIMIT: &str = "limit";
const DEPTH: &str = "depth";

/// Configuration for authenticating to Confluence Cloud.
///
/// Values are normally provided via the chain `Secrets` mechanism. `api_key` is the Confluence
/// API token associated with `email`. `confluence_url` should be the base like
/// `https://your-domain.atlassian.net`.
#[derive(Debug, Serialize, Deserialize, WeilType, Default)]
pub struct ConfluenceConfig {
    confluence_url: String,
    email: String,
    api_key: String,
}

/// Response type for listing spaces
#[derive(Debug, Serialize, Deserialize)]
pub struct SpaceListResponse {
    /// Total number of spaces available
    pub total_spaces: u32,
    /// List of space keys
    pub space_keys: Vec<String>,
    /// Detailed information about each space
    pub spaces: Vec<types::SpaceSummary>,
}

/// Minimal ancestor representation returned by Confluence hierarchy endpoints.
#[derive(Debug, Serialize, Deserialize)]
pub struct PageAncestor {
    id: String,
}

/// Trait that defines the Confluence MCP interface exposed as a smart contract.
///
/// All methods return typed results where possible, or serialized JSON strings for passthrough
/// endpoints. Errors are returned as `String` for simplicity at the contract boundary.
trait Confluence {
    /// Initialize the smart-contract state with an empty `Secrets<ConfluenceConfig>`.
    fn new() -> Result<Self, String>
    where
        Self: Sized;

    /// List all Confluence spaces (paginated under the hood).
    async fn list_spaces(&self) -> Result<Vec<SpaceSummary>, String>;

    /// Create a page in a space by numeric `space_id` using Atlas Doc Format (ADF).
    async fn create_page_by_space_id(
        &self,
        space_id: u64,
        title: String,
        content: String,
    ) -> Result<CreatePageResponse, String>;

    /// Create a page with a table under a **parent page** identified by name, resolving `space_name` and `parent_page_name`.
    async fn create_page_with_table_by_space_name_with_parent_page(
        &self,
        space_name: String,
        title: String,
        parent_page_name: String,
        headers: String,
        rows: String,
    ) -> Result<CreatePageResponse, String>;

    /// Upload a file (via IMFS) and create a page with a table under a **parent page**.
    async fn import_file_and_create_page_with_table_with_parent_page(
        &self,
        space_name: String,
        title: String,
        parent_page_name: String,
        file_descriptor: String,
    ) -> Result<CreatePageResponse, String>;

    /// Upload a file (via IMFS) and create a page with a table **without** a parent page.
    async fn import_file_and_create_page_with_table_without_parent_page(
        &self,
        space_name: String,
        title: String,
        file_descriptor: String,
    ) -> Result<CreatePageResponse, String>;

    /// Create a page with a table in a space by name (no parent).
    async fn create_page_with_table_by_space_name_without_parent_page(
        &self,
        space_name: String,
        title: String,
        headers: String,
        rows: String,
    ) -> Result<CreatePageResponse, String>;

    /// Append text content to a page by numeric ID (storage representation).
    async fn append_to_page_by_id(
        &self,
        page_id: u64,
        content: String,
    ) -> Result<ContentDetails, String>;

    /// Upload an IMFS file and append a **table** to a page resolved by name and space.
    async fn import_file_and_append_table_to_page_by_page_name(
        &self,
        file_descriptor: String,
        page_name: String,
        space_name: String,
    ) -> Result<ContentDetails, String>;

    /// Append text content to a page resolved by name and space.
    async fn append_to_page_by_page_name(
        &self,
        page_name: String,
        space_name: String,
        content: String,
    ) -> Result<ContentDetails, String>;

    /// Append a **table** to a page resolved by name and space.
    async fn append_table_to_page_by_page_name(
        &self,
        page_name: String,
        space_name: String,
        headers: String,
        rows: String,
    ) -> Result<ContentDetails, String>;

    /// Create a page in a space by `space_name` (ADF paragraph content).
    async fn create_page_by_space_name(
        &self,
        space_name: String,
        title: String,
        content: String,
    ) -> Result<CreatePageResponse, String>;

    /// Create a page in a space by ID under a **parent page id**.
    async fn create_page_by_space_id_with_parent_page_id(
        &self,
        space_id: u64,
        title: String,
        parent_id: u64,
        content: String,
    ) -> Result<CreatePageResponse, String>;

    /// Create a page in a space by name under a **parent page name**.
    async fn create_page_by_space_name_with_parent_page_name(
        &self,
        space_name: String,
        title: String,
        parent_page_name: String,
        content: String,
    ) -> Result<CreatePageResponse, String>;

    /// Get page details by numeric ID. Body format can be requested via query.
    async fn get_page_by_id(&self, page_id: u64) -> Result<ContentDetails, String>;

    /// Get page details by page title and space name.
    async fn get_page_by_name(
        &self,
        page_name: String,
        space_name: String,
    ) -> Result<ContentDetails, String>;

    /// Update a page by ID with new title/content. Version is auto-incremented.
    async fn update_page_by_id(
        &self,
        page_id: u64,
        space_id: Option<u64>,
        new_title: String,
        new_content: String,
    ) -> Result<ContentDetails, String>;

    /// Update a page by name/space. Resolves IDs and forwards to `update_page_by_id`.
    async fn update_page_by_name(
        &self,
        page_name: String,
        space_name: String,
        new_title: String,
        new_content: String,
    ) -> Result<ContentDetails, String>;

    /// Delete a page by ID.
    async fn delete_page(&self, page_id: u64) -> Result<DeleteResult, String>;

    /// Create a blog post by space ID (ADF paragraph content).
    async fn create_blog_post_by_space_id(
        &self,
        space_id: u64,
        title: String,
        content: String,
    ) -> Result<CreateBlogPostResponse, String>;

    /// Create a blog post by space name.
    async fn create_blog_post_by_space_name(
        &self,
        space_name: String,
        title: String,
        content: String,
    ) -> Result<CreateBlogPostResponse, String>;

    /// Get a blog post by numeric ID (storage body).
    async fn get_blog_post_by_id(&self, blog_post_id: u64) -> Result<ContentDetails, String>;

    /// Get a blog post by name and space.
    async fn get_blog_post_by_name(
        &self,
        blog_post_name: String,
        space_name: String,
    ) -> Result<ContentDetails, String>;

    /// Update a blog post by ID. If `new_version_number` is `None`, it auto-increments.
    async fn update_blog_post_by_id(
        &self,
        blog_post_id: u64,
        space_id: Option<u64>,
        new_title: String,
        new_content: String,
        new_version_number: Option<u32>,
    ) -> Result<BlogPostDetails, String>;

    /// Update a blog post by name and space, forwarding to the ID-based updater.
    async fn update_blog_post_by_name(
        &self,
        blog_post_name: String,
        space_name: String,
        new_title: String,
        new_content: String,
        new_version_number: Option<u32>,
    ) -> Result<BlogPostDetails, String>;

    /// Delete a blog post by ID.
    async fn delete_blog_post(&self, blog_post_id: u64) -> Result<DeleteResult, String>;

    /// List blog posts in a space by ID (de-paginated).
    async fn list_blog_posts_in_space_by_id(
        &self,
        space_id: u64,
    ) -> Result<BlogPostHierarchyResponse, String>;

    /// List blog posts in a space by name.
    async fn list_blog_posts_in_space_by_name(
        &self,
        space_name: String,
    ) -> Result<BlogPostHierarchyResponse, String>;

    /// List labels on a page by page ID.
    async fn list_page_labels_by_id(&self, page_id: u64) -> Result<Vec<Label>, String>;

    /// List labels on a page by page name and space name.
    async fn list_page_labels_by_name(
        &self,
        page_name: String,
        space_name: String,
    ) -> Result<Vec<Label>, String>;

    /// List labels in a space by ID.
    async fn list_space_labels_by_id(&self, space_id: u64) -> Result<Vec<Label>, String>;

    /// List labels in a space by name.
    async fn list_space_labels_by_name(&self, space_name: String) -> Result<Vec<Label>, String>;

    /// List labels on a blog post by ID.
    async fn list_blog_post_labels_by_id(&self, blog_post_id: u64) -> Result<Vec<Label>, String>;

    /// List labels on a blog post by name and space.
    async fn list_blog_post_labels_by_name(
        &self,
        blog_post_name: String,
        space_name: String,
    ) -> Result<Vec<Label>, String>;

    /// Create a **footer** comment on a page by page ID (string form).
    async fn create_footer_comment_on_page_by_page_id(
        &self,
        page_id: String,
        content: String,
    ) -> Result<CommentDetails, String>;

    /// Create a **footer** comment on a page by page name and space name.
    async fn create_footer_comment_on_page_by_page_name(
        &self,
        page_name: String,
        space_name: String,
        content: String,
    ) -> Result<CommentDetails, String>;

    /// Create a **footer** comment on a blog post by ID.
    async fn create_footer_comment_on_blog_post_by_id(
        &self,
        blog_post_id: u64,
        content: String,
    ) -> Result<CommentDetails, String>;

    /// Create a **footer** comment on a blog post by name and space.
    async fn create_footer_comment_on_blog_post_by_name(
        &self,
        blog_post_name: String,
        space_name: String,
        content: String,
    ) -> Result<CommentDetails, String>;

    /// Reply to a **footer** comment by parent comment ID.
    async fn reply_footer_comment(
        &self,
        parent_comment_id: u64,
        content: String,
    ) -> Result<CommentDetails, String>;

    /// Get a **footer** comment by ID.
    async fn get_footer_comment_by_id(&self, comment_id: u64) -> Result<CommentDetails, String>;

    /// List **direct** footer comments under a page by ID (not recursive).
    async fn list_page_direct_footer_comments_by_id(
        &self,
        page_id: u64,
    ) -> Result<Vec<CommentDetails>, String>;

    /// List **direct** footer comments under a page by name/space.
    async fn list_page_direct_footer_comments_by_name(
        &self,
        page_name: String,
        space_name: String,
    ) -> Result<Vec<CommentDetails>, String>;

    /// Update a comment by ID (storage representation).
    async fn update_comment(
        &self,
        comment_id: u64,
        new_content: String,
    ) -> Result<CommentDetails, String>;

    /// Delete a comment by ID.
    async fn delete_comment(&self, comment_id: u64) -> Result<DeleteResult, String>;

    /// List **direct** footer comments under a blog post by ID (not recursive).
    async fn list_blog_post_direct_footer_comments_by_id(
        &self,
        blog_post_id: u64,
    ) -> Result<Vec<CommentDetails>, String>;

    /// List **direct** footer comments under a blog post by name/space.
    async fn list_blog_post_direct_footer_comments_by_name(
        &self,
        blog_post_name: String,
        space_name: String,
    ) -> Result<Vec<CommentDetails>, String>;

    /// Get the children of a comment (thread replies).
    async fn get_comment_children(&self, comment_id: u64) -> Result<Vec<CommentDetails>, String>;

    /// List permissions configured on a given space.
    async fn list_space_permissions(&self, space_id: u64) -> Result<Vec<SpacePermission>, String>;

    /// Get direct child pages of a given page ID.
    async fn get_page_children(&self, page_id: u64) -> Result<PageHierarchyResponse, String>;

    /// Get descendants of a page by ID (walks multiple depths internally).
    async fn get_page_descendants_by_page_id(
        &self,
        page_id: u64,
    ) -> Result<PageHierarchyResponse, String>;

    /// Get ancestors of a page by ID.
    async fn get_page_ancestors_by_page_id(
        &self,
        page_id: u64,
    ) -> Result<Vec<PageAncestor>, String>;

    /// Get descendants of a page by resolving name/space.
    async fn get_page_descendants_by_page_name(
        &self,
        page_name: String,
        space_name: String,
    ) -> Result<PageHierarchyResponse, String>;

    /// Get ancestors of a page by resolving name/space.
    async fn get_page_ancestors_by_page_name(
        &self,
        page_name: String,
        space_name: String,
    ) -> Result<Vec<PageAncestor>, String>;

    /// Get all pages in a space by ID.
    async fn get_pages_in_space_by_id(&self, space_id: u64) -> Result<Vec<ContentDetails>, String>;

    /// Get all pages in a space by name.
    async fn get_pages_in_space_by_name(
        &self,
        space_name: String,
    ) -> Result<Vec<ContentDetails>, String>;

    /// JSON schema of callable tools for LLM function-calling.
    fn tools(&self) -> String;

    /// Optional prompt pack (currently empty placeholder).
    fn prompts(&self) -> String;
}

/// Parse a `SpaceSummary` from a raw JSON value returned by Confluence.
///
/// This helper is used when the REST payload isn't directly mapped by Serde.
fn parse_space_summary(space_json: &serde_json::Value) -> types::SpaceSummary {
    types::SpaceSummary {
        id: space_json["id"].as_str().unwrap_or("").to_string(),
        key: space_json["key"].as_str().unwrap_or("").to_string(),
        name: space_json["name"].as_str().unwrap_or("").to_string(),
        space_type: space_json["type"].as_str().unwrap_or("").to_string(),
        status: space_json["status"].as_str().unwrap_or("").to_string(),
    }
}

/// Fetch file content from the IMFS contract using its well-known name (`imfs`).
///
/// The `file_descriptor` is an encoded handle the IMFS contract understands. This function
/// performs a cross-contract call and returns the file's textual content as a `String`.
async fn get_imfs_file_content(file_descriptor: String) -> Result<String, String> {
    // parameter definition for the cross contract call to imfs
    #[derive(Serialize, Deserialize)]
    struct Args {
        file_descriptor: String,
    }

    let args = Args { file_descriptor };

    // this will get the address of the imfs deployed on this pod
    let contract_addr = Runtime::contract_id_for_name("imfs");

    let file_content = Runtime::call_contract::<String>(
        contract_addr,                               //address of imfs
        "read".to_string(),                          //method to call
        Some(serde_json::to_string(&args).unwrap()), //serialized args
    )
    .map_err(|err| err.to_string())?;

    Ok(file_content)
}

/// Persistent contract state for the Confluence MCP server.
///
/// Holds `Secrets<ConfluenceConfig>` which encapsulate credentials and base URL.
#[derive(Serialize, Deserialize, WeilType)]
pub struct ConfluenceContractState {
    secrets: Secrets<ConfluenceConfig>,
}

/// Convert a Confluence timestamp string (`TIMESTAMP_NTX`) to a shorter, readable form.
///
/// This truncates the fractional seconds and anything following the first dot.
fn parse_date(date_str: String) -> String {
    let index_middle = date_str.find(".").unwrap();
    date_str.split_at(index_middle).0.to_owned()
}

/// Convenience to produce a paragraph node with plain text content in ADF.
fn create_paragraph_node(content: String) -> Node {
    Node::Paragraph {
        content: Some(vec![Content {
            text: Some(content),
            r#type: TEXT.to_string(),
        }]),
    }
}

impl ConfluenceContractState {
    /// Make an authenticated HTTP request to Confluence REST v2 with optional query/body.
    ///
    /// Returns `(status_code, body_text)` or an error string if the status doesn't match
    /// `expected_status_code`.
    async fn make_request(
        &self,
        method: HttpMethod,
        endpoint: &str,
        query_params: Vec<(String, String)>,
        body: Option<String>,
        expected_status_code: u16,
    ) -> Result<(u16, String), String> {
        let url = format!(
            "{}/wiki/api/v2/{}",
            self.secrets.config().confluence_url,
            endpoint
        );

        let headers = HashMap::from([
            ("Content-Type".to_string(), "application/json".to_string()),
            (
                "Authorization".to_string(),
                format!(
                    "Basic {}",
                    BASE64_STANDARD.encode(format!(
                        "{}:{}",
                        self.secrets.config().email,
                        self.secrets.config().api_key
                    ))
                ),
            ),
        ]);

        let mut request = HttpClient::request(&url, method)
            .headers(headers)
            .query(query_params);
        match body {
            Some(body) => request = request.body(body),
            None => {}
        }

        let response = request.send().map_err(|err| err.to_string())?;
        let status = response.status();
        let text = response.text();

        if status != expected_status_code {
            return Err(format!("HTTP {}: {}", status, text));
        }

        Ok((status, text))
    }

    /// Drain all pages of a paginated `ListResponse<T>` by following `links.next`.
    ///
    /// Returns a single concatenated `Vec<T>`.
    async fn process_complete_response<T>(
        &self,
        list_response: ListResponse<T>,
    ) -> Result<Vec<T>, String>
    where
        T: for<'de> Deserialize<'de>,
    {
        let mut vec_list = list_response.results;
        self.get_complete_response(&mut vec_list, list_response.links.next)?;

        Ok(vec_list)
    }

    /// Internal recursion that walks `links.next` until exhausted, pushing into `vec_items`.
    fn get_complete_response<T>(
        &self,
        vec_items: &mut Vec<T>,
        next_url: Option<String>,
    ) -> Result<(), String>
    where
        T: for<'de> Deserialize<'de>,
    {
        let Some(next_url) = next_url else {
            return Ok(());
        };
        let url = format!("{}/{}", self.secrets.config().confluence_url, next_url);
        let headers = HashMap::from([
            ("Content-Type".to_string(), "application/json".to_string()),
            (
                "Authorization".to_string(),
                format!(
                    "Basic {}",
                    BASE64_STANDARD.encode(format!(
                        "{}:{}",
                        self.secrets.config().email,
                        self.secrets.config().api_key
                    ))
                ),
            ),
        ]);

        let request = HttpClient::request(&url, HttpMethod::Get).headers(headers);
        let response = request.send().map_err(|err| err.to_string())?;
        let response_text = response.text();
        // NOTE: not validating w.r.t. status code because this endpoint is provided in the API
        // response itself, and if the page didn't exist , it would error out before this function
        // is called.

        let list_response: ListResponse<T> =
            serde_json::from_str(&response_text).map_err(|err| err.to_string())?;

        let next_page_url = list_response.links.next;
        list_response
            .results
            .into_iter()
            .for_each(|item| vec_items.push(item));

        if next_page_url.is_some() {
            self.get_complete_response(vec_items, next_page_url)?;
        }
        Ok(())
    }

    /// Resolve a human space name to its numeric `space_id`.
    async fn get_space_id_from_name(&self, space_name: String) -> Result<u64, String> {
        let space_list = self.list_spaces().await?;
        let space = space_list
            .into_iter()
            .find(|s| s.name == space_name)
            .ok_or_else(|| "Space not found".to_string())?;
        Ok(space.id.parse::<u64>().unwrap())
    }

    /// Resolve page ID from `page_name` scoped to `space_name`.
    ///
    /// If multiple pages share the title within a space, this returns one arbitrarily (as per API).
    async fn get_page_id_from_name(
        &self,
        page_name: String,
        space_name: String,
    ) -> Result<u64, String> {
        let space_id = self.get_space_id_from_name(space_name.clone()).await?;
        let query_params = vec![
            ("title".to_string(), page_name.clone()),
            ("space-id".to_string(), space_id.to_string()),
        ];
        let endpoint = format!("spaces/{}/pages", space_id);
        let response = self
            .make_request(HttpMethod::Get, &endpoint, query_params, None, 200)
            .await
            .map_err(|err| err.to_string())?
            .1;

        let page_list: ListResponse<ContentDetails> =
            serde_json::from_str(&response).map_err(|err| err.to_string())?;
        let page = page_list
            .results
            .iter()
            .find(|s| s.title == page_name)
            .ok_or_else(|| {
                format!(
                    "page not found for the given page_name: {page_name} and space : {space_name}"
                )
            })?;

        Ok(page.id.parse::<u64>().unwrap())
    }

    /// Resolve blog post ID from `blog_post_name` scoped to `space_name`.
    async fn get_blog_post_id_from_name(
        &self,
        blog_post_name: String,
        space_name: String,
    ) -> Result<u64, String> {
        let space_id = self.get_space_id_from_name(space_name.clone()).await?;
        let query_params = vec![
            ("title".to_string(), blog_post_name.clone()),
            ("space-id".to_string(), space_id.to_string()),
        ];
        let endpoint = format!("spaces/{}/blogposts", space_id);
        let response = self
            .make_request(HttpMethod::Get, &endpoint, query_params, None, 200)
            .await
            .map_err(|err| err.to_string())?
            .1;

        let blog_post_list_response: types::ListResponse<types::BlogPostHierarchyItem> =
            serde_json::from_str(&response).map_err(|e| e.to_string())?;

        let blog_post = blog_post_list_response.results.iter()
            .find(|s| s.title == blog_post_name)
            .ok_or_else(|| {
                format!(
                    "blog post not found for the given name: {blog_post_name} and space : {space_name}"
                )
            })?;

        Ok(blog_post.id.parse::<u64>().unwrap())
    }

    /// Fetch page details with body returned in **Atlas Doc Format (ADF)**.
    async fn get_page_adf_doc_format(&self, page_id: u64) -> Result<types::ContentDetails, String> {
        let endpoint = format!("pages/{}", page_id);
        let query_params = vec![(BODY_FORMAT.to_string(), ATLAS_DOC_FORMAT.to_string())];
        let response = self
            .make_request(HttpMethod::Get, &endpoint, query_params, None, 200)
            .await?
            .1;
        serde_json::from_str(&response).map_err(|e| e.to_string())
    }

    /// Create a page using a pre-built ADF `Document` (optionally under a parent).
    async fn create_page(
        &self,
        space_id: u64,
        title: String,
        parent_id: Option<u64>,
        document: Document,
    ) -> Result<responses::CreatePageResponse, String> {
        let req_body = types::CreateContentRequest {
            space_id,
            title: &title,
            parent_id,
            body: types::AtlasDocFormatBodyStr {
                value: serde_json::to_string(&document).map_err(|err| err.to_string())?,
                representation: ATLAS_DOC_FORMAT.to_string(),
            },
        };
        let body = serde_json::to_string(&req_body).map_err(|e| e.to_string())?;
        let response = self
            .make_request(HttpMethod::Post, "/pages", vec![], Some(body), 200)
            .await?
            .1;
        let create_content_response: CreateContentDetails =
            serde_json::from_str(&response).map_err(|e| e.to_string())?;

        Ok(responses::CreatePageResponse {
            page_id: create_content_response.id,
            title: create_content_response.title,
            status: create_content_response.status,
            space_id: create_content_response.space_id,
            created_at: create_content_response.created_at,
            parent_type: create_content_response.parent_type,
        })
    }

    /// Convenience to build a simple ADF with a leading paragraph and a generated table,
    /// then create the page (optionally under a parent).
    pub async fn create_table(
        &self,
        space_id: u64,
        title: String,
        parent_id: Option<u64>,
        headers: String,
        rows: String,
    ) -> Result<CreatePageResponse, String> {
        let table = deserialize_and_create_table_node(headers, rows)?;

        let adf_doc = Document {
            version: Some(1),
            r#type: "doc".into(),
            content: vec![
                Node::Paragraph {
                    content: Some(vec![Content {
                        r#type: "text".into(),
                        text: Some("Table generated from Icarus: ".into()),
                    }]),
                },
                table,
            ],
        };

        self.create_page(space_id, title, parent_id, adf_doc).await
    }
}

/// Pull a `storage`-format body value from a `ContentDetails` body, or empty string if missing.
fn get_content_from_body(body: Option<Body>) -> String {
    let body_str = match body {
        None => String::new(),
        Some(body) => match body.storage {
            None => String::new(),
            Some(storage_body) => storage_body.value,
        },
    };
    body_str
}

/// Deserialize headers/rows JSON strings and produce an ADF `Node::Table`.
fn deserialize_and_create_table_node(headers: String, rows: String) -> Result<Node, String> {
    let headers_parsed: Vec<String> = serde_json::from_str(&headers)
        .map_err(|err| format!("invalid header passed , {:?}", err))?;

    let rows_parsed: Vec<Vec<Value>> =
        serde_json::from_str(&rows).map_err(|err| format!("invalid rows passed : {:?}", err))?;

    create_table_node(headers_parsed, rows_parsed)
}

/// Internal bag for parsed table data (headers + rows) coming from IMFS files.
struct TableFields {
    headers: Vec<String>,
    rows: Vec<Vec<Value>>,
}

/// Fetch a file via IMFS and convert it into table headers/rows.
///
/// The file is expected to be a JSON array of serialized maps (one per row). Keys are used as
/// headers; values become cells. Ordering is derived from `BTreeMap`'s sorted key iteration.
async fn get_table_from_imfs_file(file_descriptor: String) -> Result<TableFields, String> {
    let serialized_response = get_imfs_file_content(file_descriptor).await?;
    let row_list: Vec<String> = serde_json::from_str(&serialized_response).map_err(|err| {
        format!(
            "invalid file response , deserialization failed at first try : {}",
            err.to_string()
        )
    })?;
    let mut col_row_map: Vec<BTreeMap<String, Value>> = Vec::new();

    for row in row_list {
        let row_map: BTreeMap<String, Value> = serde_json::from_str(&row).map_err(|err| {
            format!(
                "invalid data, failed to deserialize this map: {}",
                err.to_string()
            )
        })?;
        col_row_map.push(row_map);
    }

    let mut is_first_iteration = true;
    let mut header_vec: Vec<String> = Vec::new();
    let mut row_vec: Vec<Vec<Value>> = Vec::new();

    for row_map in col_row_map {
        let mut row = Vec::new();
        // NOTE: relying on rust BTreeMap's property where this iterator visits keys in sorted
        // order, therefore i can assume that row values will always be in same order w.r.t. the
        // header
        for (i, (key, value)) in row_map.into_iter().enumerate() {
            if is_first_iteration {
                header_vec.push(key.clone());
            } else {
                let Some(header_value) = header_vec.get(i) else {
                    return Err(format!(
                        "invalid data received from file, index out of bound"
                    ));
                };
                if !key.eq(header_value) {
                    return Err(format!(
                        "invalid data received from file, header value different for different row"
                    ));
                }
            }
            row.push(value);
        }
        if is_first_iteration {
            is_first_iteration = false;
        }
        row_vec.push(row);
    }
    Ok(TableFields {
        headers: header_vec,
        rows: row_vec,
    })
}

/// Build an ADF `Node::Table` from string headers and JSON values.
///
/// `headers` must be the same length as each row. Non-string JSON values are stringified.
fn create_table_node(headers: Vec<String>, rows: Vec<Vec<Value>>) -> Result<Node, String> {
    let mut header_columns = Vec::new();
    for header_value in headers {
        let header = Node::TableHeader {
            attrs: CellAttrs {
                colspan: 1,
                rowspan: 1,
            },
            content: vec![Node::Paragraph {
                content: Some(vec![Content {
                    r#type: "text".into(),
                    text: Some(header_value),
                }]),
            }],
        };
        header_columns.push(header);
    }

    let header_row = Node::TableRow {
        content: header_columns,
    };

    let mut table_rows = Vec::new();
    table_rows.push(header_row);

    for row in rows {
        let mut row_cells = Vec::new();
        for col in row {
            let text = match col {
                Value::String(s) => s,
                _ => col.to_string(),
            };
            let cell = Node::TableCell {
                attrs: CellAttrs {
                    colspan: 1,
                    rowspan: 1,
                },
                content: vec![Node::Paragraph {
                    content: Some(vec![Content {
                        r#type: "text".into(),
                        text: Some(text),
                    }]),
                }],
            };
            row_cells.push(cell);
        }
        table_rows.push(Node::TableRow { content: row_cells });
    }

    Ok(Node::Table {
        attrs: TableAttrs {
            layout: "default".into(),
            width: None,
            local_id: None,
            is_number_column_enabled: Some(false),
        },
        content: table_rows,
    })
}

#[smart_contract]
impl Confluence for ConfluenceContractState {
    /// Initialize `ConfluenceContractState` with an empty `Secrets<ConfluenceConfig>`.
    #[constructor]
    fn new() -> Result<Self, String>
    where
        Self: Sized,
    {
        Ok(Self {
            secrets: Secrets::<ConfluenceConfig>::new(),
        })
    }

    /// List all spaces (internally traverses pagination up to Confluence max page size).
    #[query]
    async fn list_spaces(&self) -> Result<Vec<SpaceSummary>, String> {
        let endpoint = "/spaces";
        let response: ListResponse<SpaceSummary> = serde_json::from_str(
            &self
                .make_request(
                    HttpMethod::Get,
                    endpoint,
                    vec![(LIMIT.to_string(), 250.to_string())],
                    None,
                    200,
                )
                .await?
                .1,
        )
        .map_err(|err| err.to_string())?;
        self.process_complete_response(response).await
    }

    // --- Page CRUD ---

    /// Create a page by `space_id`, wrapping the provided `content` in a simple ADF paragraph.
    #[query]
    async fn create_page_by_space_id(
        &self,
        space_id: u64,
        title: String,
        content: String,
    ) -> Result<responses::CreatePageResponse, String> {
        let document = Document {
            r#type: DOC.to_string(),
            content: vec![create_paragraph_node(content)],
            version: None,
        };
        self.create_page(space_id, title, None, document).await
    }

    /// Create a page **with table** under a named parent page within a space.
    #[query]
    async fn create_page_with_table_by_space_name_with_parent_page(
        &self,
        space_name: String,
        title: String,
        parent_page_name: String,
        headers: String,
        rows: String,
    ) -> Result<CreatePageResponse, String> {
        let space_id = self.get_space_id_from_name(space_name.clone()).await?;
        let parent_page_id = self
            .get_page_id_from_name(parent_page_name, space_name)
            .await?;
        self.create_table(space_id, title, Some(parent_page_id), headers, rows)
            .await
    }

    /// Create a page **with table** in a space by name (no parent).
    #[query]
    async fn create_page_with_table_by_space_name_without_parent_page(
        &self,
        space_name: String,
        title: String,
        headers: String,
        rows: String,
    ) -> Result<CreatePageResponse, String> {
        let space_id = self.get_space_id_from_name(space_name).await?;
        self.create_table(space_id, title, None, headers, rows)
            .await
    }

    /// Import an IMFS file and create a **table page** under a parent page.
    #[query]
    async fn import_file_and_create_page_with_table_with_parent_page(
        &self,
        space_name: String,
        title: String,
        parent_page_name: String,
        file_descriptor: String,
    ) -> Result<CreatePageResponse, String> {
        let table_fields = get_table_from_imfs_file(file_descriptor).await?;
        let header_serialized = serde_json::to_string(&table_fields.headers).unwrap(); // safe
        // to unwrap since deserializing
        let row_serialized = serde_json::to_string(&table_fields.rows).unwrap();

        self.create_page_with_table_by_space_name_with_parent_page(
            space_name,
            title,
            parent_page_name,
            header_serialized,
            row_serialized,
        )
        .await
    }

    /// Import an IMFS file and create a **table page** (no parent).
    #[query]
    async fn import_file_and_create_page_with_table_without_parent_page(
        &self,
        space_name: String,
        title: String,
        file_descriptor: String,
    ) -> Result<CreatePageResponse, String> {
        let table_fields = get_table_from_imfs_file(file_descriptor).await?;
        let header_serialized = serde_json::to_string(&table_fields.headers).unwrap(); // safe
        // to unwrap since deserializing
        let row_serialized = serde_json::to_string(&table_fields.rows).unwrap();

        self.create_page_with_table_by_space_name_without_parent_page(
            space_name,
            title,
            header_serialized,
            row_serialized,
        )
        .await
    }

    /// Append a **table** to an existing page resolved by name/space, preserving existing content.
    #[query]
    async fn append_table_to_page_by_page_name(
        &self,
        page_name: String,
        space_name: String,
        headers: String,
        rows: String,
    ) -> Result<ContentDetails, String> {
        let page_id = self.get_page_id_from_name(page_name, space_name).await?;
        let page_details = self.get_page_adf_doc_format(page_id).await?;
        let Some(page_body) = page_details.body else {
            return Err("page body none".to_string());
        };

        let Some(document_str) = page_body.atlas_doc_format else {
            return Err("page body received isn't of atlas doc format".to_string());
        };
        let mut document: Document = serde_json::from_str(&document_str.value).map_err(|err| {
            format!(
                "Unable to deserialize Document json to Document Struct {}",
                err.to_string()
            )
        })?;

        let table = deserialize_and_create_table_node(headers, rows)?;

        document.content.push(Node::Paragraph {
            content: Some(vec![Content {
                r#type: "text".into(),
                text: Some("Table generated from Icarus: ".into()),
            }]),
        });
        document.content.push(table);

        let version_number = page_details.version.number + 1;
        let req_body = types::UpdateContentRequestAtlasDocFormat {
            id: page_id,
            status: "current",
            title: &page_details.title,
            space_id: None,
            body: AtlasDocFormatBodyStr {
                value: serde_json::to_string(&document).map_err(|err| err.to_string())?,
                representation: ATLAS_DOC_FORMAT.to_string(),
            },
            version: types::PageVersion {
                number: version_number,
                message: None,
            },
        };
        let body = serde_json::to_string(&req_body).map_err(|e| e.to_string())?;
        let endpoint = format!("/pages/{}", page_id);
        let response = self
            .make_request(HttpMethod::Put, &endpoint, vec![], Some(body), 200)
            .await?
            .1;
        serde_json::from_str(&response).map_err(|e| e.to_string())
    }

    /// Create a page by `space_name` using ADF paragraph content.
    #[query]
    async fn create_page_by_space_name(
        &self,
        space_name: String,
        title: String,
        content: String,
    ) -> Result<CreatePageResponse, String> {
        let space_id = self.get_space_id_from_name(space_name).await?;
        self.create_page_by_space_id(space_id, title, content).await
    }

    /// Append text to an existing page by ID (storage representation).
    #[query]
    async fn append_to_page_by_id(
        &self,
        page_id: u64,
        content: String,
    ) -> Result<ContentDetails, String> {
        // get the page content for this id
        let page = self.get_page_by_id(page_id.clone()).await?;
        let original_body = get_content_from_body(page.body);

        // append to the content , and then send the update
        let new_content = format!("{original_body}\n{content}");
        self.update_page_by_id(page_id, None, page.title, new_content)
            .await
    }

    /// Import an IMFS file and append a **table** to a page resolved by name/space.
    #[query]
    async fn import_file_and_append_table_to_page_by_page_name(
        &self,
        file_descriptor: String,
        page_name: String,
        space_name: String,
    ) -> Result<ContentDetails, String> {
        let table_fields = get_table_from_imfs_file(file_descriptor).await?;
        let serialized_header_vec =
            serde_json::to_string(&table_fields.headers).map_err(|err| err.to_string())?;
        let serialized_row_vec =
            serde_json::to_string(&table_fields.rows).map_err(|err| err.to_string())?;
        self.append_table_to_page_by_page_name(
            page_name,
            space_name,
            serialized_header_vec,
            serialized_row_vec,
        )
        .await
    }

    /// Append text to a page by name/space (storage representation).
    #[query]
    async fn append_to_page_by_page_name(
        &self,
        page_name: String,
        space_name: String,
        content: String,
    ) -> Result<ContentDetails, String> {
        let page_id = self.get_page_id_from_name(page_name, space_name).await?;
        self.append_to_page_by_id(page_id, content).await
    }

    /// Get page details by numeric ID, requesting `storage` body format.
    #[query]
    async fn get_page_by_id(&self, page_id: u64) -> Result<types::ContentDetails, String> {
        let endpoint = format!("pages/{}", page_id);
        let query_params = vec![(BODY_FORMAT.to_string(), STORAGE.to_string())];
        let response = self
            .make_request(HttpMethod::Get, &endpoint, query_params, None, 200)
            .await?
            .1;
        serde_json::from_str(&response).map_err(|e| e.to_string())
    }

    /// Create a page by `space_id` and **parent page id**, using ADF paragraph content.
    #[query]
    async fn create_page_by_space_id_with_parent_page_id(
        &self,
        space_id: u64,
        title: String,
        parent_id: u64,
        content: String,
    ) -> Result<CreatePageResponse, String> {
        let document = Document {
            r#type: DOC.to_string(),
            content: vec![create_paragraph_node(content)],
            version: None,
        };
        self.create_page(space_id, title, Some(parent_id), document)
            .await
    }

    /// Create a page by `space_name` and **parent page name**.
    #[query]
    async fn create_page_by_space_name_with_parent_page_name(
        &self,
        space_name: String,
        title: String,
        parent_page_name: String,
        content: String,
    ) -> Result<CreatePageResponse, String> {
        let space_id = self.get_space_id_from_name(space_name.clone()).await?;
        let parent_page_id = self
            .get_page_id_from_name(parent_page_name, space_name)
            .await?;
        self.create_page_by_space_id_with_parent_page_id(space_id, title, parent_page_id, content)
            .await
    }

    /// Get page details by `page_name` and `space_name` (storage body format).
    #[query]
    async fn get_page_by_name(
        &self,
        page_name: String,
        space_name: String,
    ) -> Result<ContentDetails, String> {
        let page_id = self.get_page_id_from_name(page_name, space_name).await?;
        let endpoint = format!("pages/{}?body-format=storage", page_id);
        let response = self
            .make_request(HttpMethod::Get, &endpoint, vec![], None, 200)
            .await?
            .1;
        serde_json::from_str(&response).map_err(|e| e.to_string())
    }

    /// Update page content/title by ID. If `space_id` is `None`, it is not changed.
    #[query]
    async fn update_page_by_id(
        &self,
        page_id: u64,
        space_id: Option<u64>,
        new_title: String,
        new_content: String,
    ) -> Result<types::ContentDetails, String> {
        let version_number = {
            // Fetch the current version number if not provided
            let page_details = self.get_page_by_id(page_id).await?;
            page_details.version.number + 1
        };

        let req_body = types::UpdateContentRequest {
            id: page_id,
            status: "current",
            title: &new_title,
            space_id,
            body: types::StorageBody {
                value: new_content,
                representation: "storage".to_string(),
            },
            version: types::PageVersion {
                number: version_number,
                message: None,
            },
        };
        let body = serde_json::to_string(&req_body).map_err(|e| e.to_string())?;
        let endpoint = format!("pages/{}", page_id);
        let response = self
            .make_request(HttpMethod::Put, &endpoint, vec![], Some(body), 200)
            .await?
            .1;
        serde_json::from_str(&response).map_err(|e| e.to_string())
    }

    /// Update page by name/space (resolves IDs) with new title/content.
    #[query]
    async fn update_page_by_name(
        &self,
        page_name: String,
        space_name: String,
        new_title: String,
        new_content: String,
    ) -> Result<ContentDetails, String> {
        let space_id = self.get_space_id_from_name(space_name.clone()).await?;
        let page_id = self.get_page_id_from_name(page_name, space_name).await?;
        self.update_page_by_id(page_id, Some(space_id), new_title, new_content)
            .await
    }

    /// Delete a page by ID. Returns success flag and message.
    #[query]
    async fn delete_page(&self, page_id: u64) -> Result<types::DeleteResult, String> {
        let endpoint = format!("pages/{}", page_id);
        let (status, response_text) = self
            .make_request(HttpMethod::Delete, &endpoint, vec![], None, 204)
            .await?;
        if status == 204 {
            return Ok(DeleteResult {
                success: true,
                message: format!("Page with {page_id} Deleted"),
            });
        } else {
            return Ok(DeleteResult {
                success: false,
                message: response_text,
            });
        }
    }

    // --- Blog Post CRUD ---

    /// Create a blog post by space ID using a simple ADF paragraph.
    #[query]
    async fn create_blog_post_by_space_id(
        &self,
        space_id: u64,
        title: String,
        content: String,
    ) -> Result<CreateBlogPostResponse, String> {
        let document = Document {
            r#type: DOC.to_string(),
            content: vec![create_paragraph_node(content)],
            version: None,
        };

        let req_body = types::CreateContentRequest {
            space_id, // safe to unwrap
            title: &title,
            parent_id: None,
            body: types::AtlasDocFormatBodyStr {
                value: serde_json::to_string(&document).map_err(|err| err.to_string())?,
                representation: ATLAS_DOC_FORMAT.to_string(),
            },
        };
        let body = serde_json::to_string(&req_body).map_err(|e| e.to_string())?;
        let response = self
            .make_request(HttpMethod::Post, "blogposts", vec![], Some(body), 200)
            .await?
            .1;
        let blogpost_details: types::BlogPostDetails =
            serde_json::from_str(&response).map_err(|e| e.to_string())?;
        let body_str = get_content_from_body(blogpost_details.body);

        Ok(CreateBlogPostResponse {
            blog_post_id: blogpost_details.id,
            title: blogpost_details.title,
            status: blogpost_details.status,
            space_id: blogpost_details.space_id,
            version: blogpost_details.version,
            body: body_str,
            created_at: blogpost_details.created_at,
            author_id: blogpost_details.author_id,
        })
    }

    /// Create a blog post by space name.
    #[query]
    async fn create_blog_post_by_space_name(
        &self,
        space_name: String,
        title: String,
        content: String,
    ) -> Result<CreateBlogPostResponse, String> {
        let space_id = self.get_space_id_from_name(space_name).await?;
        self.create_blog_post_by_space_id(space_id, title, content)
            .await
    }

    /// Get blog post by name/space.
    #[query]
    async fn get_blog_post_by_name(
        &self,
        blog_post_name: String,
        space_name: String,
    ) -> Result<ContentDetails, String> {
        let blog_post_id = self
            .get_blog_post_id_from_name(blog_post_name, space_name)
            .await?;
        self.get_blog_post_by_id(blog_post_id).await
    }

    /// Get blog post by ID (storage format).
    #[query]
    async fn get_blog_post_by_id(
        &self,
        blog_post_id: u64,
    ) -> Result<types::ContentDetails, String> {
        let endpoint = format!("blogposts/{}", blog_post_id);
        let query_params = vec![("body-format".to_string(), "storage".to_string())];
        let response = self
            .make_request(HttpMethod::Get, &endpoint, query_params, None, 200)
            .await?
            .1;
        serde_json::from_str(&response).map_err(|e| e.to_string())
    }

    /// Update a blog post by ID. If `new_version_number` is `None`, the current version is fetched and incremented.
    #[query]
    async fn update_blog_post_by_id(
        &self,
        blog_post_id: u64,
        space_id: Option<u64>,
        new_title: String,
        new_content: String,
        new_version_number: Option<u32>,
    ) -> Result<types::BlogPostDetails, String> {
        let version_number = match new_version_number {
            Some(num) => num,
            None => {
                // Fetch the current version number if not provided
                let blog_post_details = self.get_blog_post_by_id(blog_post_id).await?;
                blog_post_details.version.number + 1
            }
        };
        let req_body = types::UpdateContentRequest {
            id: blog_post_id,
            status: "current",
            title: &new_title,
            space_id,
            body: types::StorageBody {
                value: new_content,
                representation: "storage".to_string(),
            },
            version: types::PageVersion {
                number: version_number,
                message: None,
            },
        };
        let body = serde_json::to_string(&req_body).map_err(|e| e.to_string())?;
        let endpoint = format!("blogposts/{}", blog_post_id);
        let response = self
            .make_request(HttpMethod::Put, &endpoint, vec![], Some(body), 200)
            .await?
            .1;
        serde_json::from_str(&response).map_err(|e| e.to_string())
    }

    /// Update a blog post by name/space, forwarding to the ID-based updater.
    #[query]
    async fn update_blog_post_by_name(
        &self,
        blog_post_name: String,
        space_name: String,
        new_title: String,
        new_content: String,
        new_version_number: Option<u32>,
    ) -> Result<BlogPostDetails, String> {
        let blog_post_id = self
            .get_blog_post_id_from_name(blog_post_name, space_name)
            .await?;
        self.update_blog_post_by_id(
            blog_post_id,
            None,
            new_title,
            new_content,
            new_version_number,
        )
        .await
    }

    /// Delete a blog post by ID. Returns success flag and message.
    #[query]
    async fn delete_blog_post(&self, blog_post_id: u64) -> Result<types::DeleteResult, String> {
        let endpoint = format!("blogposts/{}", blog_post_id);
        let (status, response_text) = self
            .make_request(HttpMethod::Delete, &endpoint, vec![], None, 204)
            .await?;
        if status == 204 {
            return Ok(DeleteResult {
                success: true,
                message: format!("Blog post with {blog_post_id} Deleted"),
            });
        } else {
            return Ok(DeleteResult {
                success: false,
                message: response_text,
            });
        }
    }

    /// List blog posts in a space by **ID**, with pagination drained into a single result.
    #[query]
    async fn list_blog_posts_in_space_by_id(
        &self,
        space_id: u64,
    ) -> Result<types::BlogPostHierarchyResponse, String> {
        let endpoint = format!("spaces/{}/blogposts", space_id);
        let response = self
            .make_request(
                HttpMethod::Get,
                &endpoint,
                vec![(LIMIT.to_string(), 250.to_string())],
                None,
                200,
            )
            .await?
            .1;
        let list: types::ListResponse<types::BlogPostHierarchyItem> =
            serde_json::from_str(&response).map_err(|e| e.to_string())?;
        let vec_list = self.process_complete_response(list).await?;
        Ok(types::BlogPostHierarchyResponse {
            total_count: vec_list.len(),
            results: vec_list,
        })
    }

    /// List blog posts in a space by **name**.
    #[query]
    async fn list_blog_posts_in_space_by_name(
        &self,
        space_name: String,
    ) -> Result<BlogPostHierarchyResponse, String> {
        let space_id = self.get_space_id_from_name(space_name).await?;
        self.list_blog_posts_in_space_by_id(space_id).await
    }

    // --- Label Management ---

    /// List labels on a page by ID (de-paginated).
    #[query]
    async fn list_page_labels_by_id(&self, page_id: u64) -> Result<Vec<types::Label>, String> {
        let endpoint = format!("pages/{}/labels", page_id);
        let response = self
            .make_request(
                HttpMethod::Get,
                &endpoint,
                vec![(LIMIT.to_string(), 250.to_string())],
                None,
                200,
            )
            .await?
            .1;
        let list: ListResponse<types::Label> =
            serde_json::from_str(&response).map_err(|e| e.to_string())?;

        self.process_complete_response(list).await
    }

    /// List labels on a page by page name/space.
    #[query]
    async fn list_page_labels_by_name(
        &self,
        page_name: String,
        space_name: String,
    ) -> Result<Vec<Label>, String> {
        let page_id = self.get_page_id_from_name(page_name, space_name).await?;
        self.list_page_labels_by_id(page_id).await
    }

    /// List labels in a space by ID.
    #[query]
    async fn list_space_labels_by_id(&self, space_id: u64) -> Result<Vec<types::Label>, String> {
        let endpoint = format!("spaces/{}/labels", space_id);
        let response = self
            .make_request(
                HttpMethod::Get,
                &endpoint,
                vec![(LIMIT.to_string(), 250.to_string())],
                None,
                200,
            )
            .await?
            .1;
        let list: ListResponse<types::Label> =
            serde_json::from_str(&response).map_err(|e| e.to_string())?;
        self.process_complete_response(list).await
    }

    /// List labels in a space by name.
    #[query]
    async fn list_space_labels_by_name(&self, space_name: String) -> Result<Vec<Label>, String> {
        let space_id = self.get_space_id_from_name(space_name).await?;
        self.list_space_labels_by_id(space_id).await
    }

    /// List labels on a blog post by ID.
    #[query]
    async fn list_blog_post_labels_by_id(
        &self,
        blog_post_id: u64,
    ) -> Result<Vec<types::Label>, String> {
        let endpoint = format!("blogposts/{}/labels", blog_post_id);
        let response = self
            .make_request(
                HttpMethod::Get,
                &endpoint,
                vec![(LIMIT.to_string(), 250.to_string())],
                None,
                200,
            )
            .await?
            .1;
        let list: ListResponse<types::Label> =
            serde_json::from_str(&response).map_err(|e| e.to_string())?;
        self.process_complete_response(list).await
    }

    /// List labels on a blog post by name/space.
    #[query]
    async fn list_blog_post_labels_by_name(
        &self,
        blog_post_name: String,
        space_name: String,
    ) -> Result<Vec<Label>, String> {
        let blog_post_id = self
            .get_blog_post_id_from_name(blog_post_name, space_name)
            .await?;
        self.list_blog_post_labels_by_id(blog_post_id).await
    }

    // --- Comment Management ---

    /// Create a **footer** comment on a page by page ID (string-typed ID for compatibility).
    #[query]
    async fn create_footer_comment_on_page_by_page_id(
        &self,
        page_id: String,
        content: String,
    ) -> Result<types::CommentDetails, String> {
        let req_body = types::CreateCommentRequest {
            page_id: Some(page_id.parse().unwrap()),
            blog_post_id: None,
            parent_comment_id: None,
            body: types::StorageBody {
                value: content,
                representation: "storage".to_string(),
            },
            space_id: None,
        };
        let body = serde_json::to_string(&req_body).map_err(|e| e.to_string())?;
        let response = self
            .make_request(HttpMethod::Post, "footer-comments", vec![], Some(body), 201)
            .await?
            .1;
        serde_json::from_str(&response).map_err(|e| e.to_string())
    }

    /// Create a **footer** comment on a page by resolving page name/space.
    #[query]
    async fn create_footer_comment_on_page_by_page_name(
        &self,
        page_name: String,
        space_name: String,
        content: String,
    ) -> Result<CommentDetails, String> {
        let page_id = self.get_page_id_from_name(page_name, space_name).await?;
        self.create_footer_comment_on_page_by_page_id(page_id.to_string(), content)
            .await
    }

    /// Reply to a **footer** comment by parent ID.
    #[query]
    async fn reply_footer_comment(
        &self,
        parent_comment_id: u64,
        content: String,
    ) -> Result<types::CommentDetails, String> {
        let req_body = types::CreateCommentRequest {
            page_id: None,
            blog_post_id: None,
            parent_comment_id: Some(parent_comment_id),
            body: types::StorageBody {
                value: content,
                representation: "storage".to_string(),
            },
            space_id: None,
        };
        let body = serde_json::to_string(&req_body).map_err(|e| e.to_string())?;
        let response = self
            .make_request(HttpMethod::Post, "footer-comments", vec![], Some(body), 201)
            .await?
            .1;
        serde_json::from_str(&response).map_err(|e| e.to_string())
    }

    /// List **direct** footer comments on a page by ID (body in storage format).
    #[query]
    async fn list_page_direct_footer_comments_by_id(
        &self,
        page_id: u64,
    ) -> Result<Vec<types::CommentDetails>, String> {
        let endpoint = format!("pages/{}/footer-comments", page_id);
        let query_params = vec![
            ("body-format".to_string(), "storage".to_string()),
            (LIMIT.to_string(), 250.to_string()),
        ];
        let response = self
            .make_request(HttpMethod::Get, &endpoint, query_params, None, 200)
            .await?
            .1;
        let list: ListResponse<types::CommentDetails> =
            serde_json::from_str(&response).map_err(|e| e.to_string())?;
        self.process_complete_response(list).await
    }

    /// List **direct** footer comments on a page by name/space.
    #[query]
    async fn list_page_direct_footer_comments_by_name(
        &self,
        page_name: String,
        space_name: String,
    ) -> Result<Vec<CommentDetails>, String> {
        let page_id = self.get_page_id_from_name(page_name, space_name).await?;
        self.list_page_direct_footer_comments_by_id(page_id).await
    }

    /// Update a comment by ID (increments version automatically).
    #[query]
    async fn update_comment(
        &self,
        comment_id: u64,
        new_content: String,
    ) -> Result<types::CommentDetails, String> {
        let comment_details = self.get_footer_comment_by_id(comment_id).await?;
        let req_body = types::UpdateCommentRequest {
            id: comment_id,
            version: types::PageVersion {
                number: comment_details.version.number + 1,
                message: None,
            },
            body: types::StorageBody {
                value: new_content,
                representation: "storage".to_string(),
            },
        };
        let body = serde_json::to_string(&req_body).map_err(|e| e.to_string())?;
        let endpoint = format!("footer-comments/{}", comment_id);
        let response = self
            .make_request(HttpMethod::Put, &endpoint, vec![], Some(body), 200)
            .await?
            .1;
        serde_json::from_str(&response).map_err(|e| e.to_string())
    }

    /// Delete a comment by ID. Returns success flag and message.
    #[query]
    async fn delete_comment(&self, comment_id: u64) -> Result<types::DeleteResult, String> {
        let endpoint = format!("footer-comments/{}", comment_id);
        let response = self
            .make_request(HttpMethod::Delete, &endpoint, vec![], None, 204)
            .await?;
        if response.0 == 204 {
            return Ok(DeleteResult {
                success: true,
                message: format!("Comment with {comment_id} Deleted"),
            });
        } else {
            return Ok(DeleteResult {
                success: false,
                message: response.1,
            });
        }
    }

    /// Create a **footer** comment on a blog post by ID.
    #[query]
    async fn create_footer_comment_on_blog_post_by_id(
        &self,
        blog_post_id: u64,
        content: String,
    ) -> Result<types::CommentDetails, String> {
        let req_body = types::CreateCommentRequest {
            page_id: None,
            blog_post_id: Some(blog_post_id),
            parent_comment_id: None,
            body: types::StorageBody {
                value: content,
                representation: "storage".to_string(),
            },
            space_id: None,
        };
        let body = serde_json::to_string(&req_body).map_err(|e| e.to_string())?;
        let response = self
            .make_request(HttpMethod::Post, "footer-comments", vec![], Some(body), 201)
            .await?
            .1;
        serde_json::from_str(&response).map_err(|e| e.to_string())
    }

    /// Create a **footer** comment on a blog post by name/space.
    #[query]
    async fn create_footer_comment_on_blog_post_by_name(
        &self,
        blog_post_name: String,
        space_name: String,
        content: String,
    ) -> Result<CommentDetails, String> {
        let blog_post_id = self
            .get_blog_post_id_from_name(blog_post_name, space_name)
            .await?;
        self.create_footer_comment_on_blog_post_by_id(blog_post_id, content)
            .await
    }

    /// Get a **footer** comment by ID (storage body format).
    #[query]
    async fn get_footer_comment_by_id(
        &self,
        comment_id: u64,
    ) -> Result<types::CommentDetails, String> {
        let endpoint = format!("footer-comments/{}", comment_id);
        let query_params = vec![("body-format".to_string(), "storage".to_string())];
        let response = self
            .make_request(HttpMethod::Get, &endpoint, query_params, None, 200)
            .await?
            .1;
        serde_json::from_str(&response).map_err(|e| e.to_string())
    }

    /// List **direct** footer comments on a blog post by ID (body in storage format).
    #[query]
    async fn list_blog_post_direct_footer_comments_by_id(
        &self,
        blog_post_id: u64,
    ) -> Result<Vec<types::CommentDetails>, String> {
        let endpoint = format!("blogposts/{}/footer-comments", blog_post_id);
        let query_params = vec![
            ("body-format".to_string(), "storage".to_string()),
            (LIMIT.to_string(), 250.to_string()),
        ];
        let response = self
            .make_request(HttpMethod::Get, &endpoint, query_params, None, 200)
            .await?
            .1;
        let list: ListResponse<types::CommentDetails> =
            serde_json::from_str(&response).map_err(|e| e.to_string())?;
        self.process_complete_response(list).await
    }

    /// List **direct** footer comments on a blog post by name/space.
    #[query]
    async fn list_blog_post_direct_footer_comments_by_name(
        &self,
        blog_post_name: String,
        space_name: String,
    ) -> Result<Vec<CommentDetails>, String> {
        let blog_post_id = self
            .get_blog_post_id_from_name(blog_post_name, space_name)
            .await?;
        self.list_blog_post_direct_footer_comments_by_id(blog_post_id)
            .await
    }

    /// Get children (replies) of a comment by ID.
    #[query]
    async fn get_comment_children(&self, comment_id: u64) -> Result<Vec<CommentDetails>, String> {
        let endpoint = format!("footer-comments/{}/children", comment_id);
        let v = vec![
            (BODY_FORMAT.to_string(), STORAGE.to_string()),
            (LIMIT.to_string(), 250.to_string()),
        ];
        let response = self
            .make_request(HttpMethod::Get, &endpoint, v, None, 200)
            .await?
            .1;

        let list: ListResponse<types::CommentDetails> =
            serde_json::from_str(&response).map_err(|err| err.to_string())?;
        self.process_complete_response(list).await
    }

    // --- Space Permissions ---

    /// List permissions on a space by ID.
    #[query]
    async fn list_space_permissions(
        &self,
        space_id: u64,
    ) -> Result<Vec<types::SpacePermission>, String> {
        let endpoint = format!("spaces/{}/permissions", space_id);
        let response = self
            .make_request(
                HttpMethod::Get,
                &endpoint,
                vec![(LIMIT.to_string(), 250.to_string())],
                None,
                200,
            )
            .await?
            .1;
        let list: ListResponse<types::SpacePermission> =
            serde_json::from_str(&response).map_err(|e| e.to_string())?;
        self.process_complete_response(list).await
    }

    // --- Hierarchy ---

    /// Get direct children of a page by ID.
    #[query]
    async fn get_page_children(
        &self,
        page_id: u64,
    ) -> Result<types::PageHierarchyResponse, String> {
        let endpoint = format!("pages/{}/direct-children", page_id);
        let response = self
            .make_request(HttpMethod::Get, &endpoint, vec![], None, 200)
            .await?
            .1;
        let list: types::ListResponse<types::PageHierarchyItem> =
            serde_json::from_str(&response).map_err(|e| e.to_string())?;
        Ok(types::PageHierarchyResponse {
            total_count: list.results.len(),
            results: list.results,
        })
    }

    /// Get descendants of a page by ID (limited depth; internally de-paginates).
    #[query]
    async fn get_page_descendants_by_page_id(
        &self,
        page_id: u64,
    ) -> Result<types::PageHierarchyResponse, String> {
        let endpoint = format!("pages/{}/descendants", page_id);
        let query_params = vec![
            (LIMIT.to_string(), "250".to_string()),
            (DEPTH.to_string(), "5".to_string()),
        ];
        let response = self
            .make_request(HttpMethod::Get, &endpoint, query_params, None, 200)
            .await?
            .1;
        let list: types::ListResponse<types::PageHierarchyItem> =
            serde_json::from_str(&response).map_err(|e| e.to_string())?;
        let results = self.process_complete_response(list).await?;
        Ok(types::PageHierarchyResponse {
            total_count: results.len(),
            results,
        })
    }

    /// Get ancestors of a page by ID.
    #[query]
    async fn get_page_ancestors_by_page_id(
        &self,
        page_id: u64,
    ) -> Result<Vec<PageAncestor>, String> {
        let endpoint = format!("pages/{}/ancestors", page_id);
        let query_params = vec![(LIMIT.to_string(), "250".to_string())];

        let response = self
            .make_request(HttpMethod::Get, &endpoint, query_params, None, 200)
            .await?
            .1;
        let list: ListResponse<PageAncestor> =
            serde_json::from_str(&response).map_err(|e| e.to_string())?;

        self.process_complete_response(list).await
    }

    /// Get descendants of a page by resolving name/space.
    #[query]
    async fn get_page_descendants_by_page_name(
        &self,
        page_name: String,
        space_name: String,
    ) -> Result<types::PageHierarchyResponse, String> {
        let page_id = self.get_page_id_from_name(page_name, space_name).await?;
        self.get_page_descendants_by_page_id(page_id).await
    }

    /// Get ancestors of a page by resolving name/space.
    #[query]
    async fn get_page_ancestors_by_page_name(
        &self,
        page_name: String,
        space_name: String,
    ) -> Result<Vec<PageAncestor>, String> {
        let page_id = self.get_page_id_from_name(page_name, space_name).await?;
        self.get_page_ancestors_by_page_id(page_id).await
    }

    /// Get all pages in a space by ID (de-paginated).
    #[query]
    async fn get_pages_in_space_by_id(&self, space_id: u64) -> Result<Vec<ContentDetails>, String> {
        let endpoint = format!("spaces/{}/pages", space_id);

        let response: ListResponse<ContentDetails> = serde_json::from_str(
            &self
                .make_request(
                    HttpMethod::Get,
                    &endpoint,
                    vec![(LIMIT.to_string(), 250.to_string())],
                    None,
                    200,
                )
                .await?
                .1,
        )
        .map_err(|e| e.to_string())?;

        self.process_complete_response(response).await
    }

    /// Get all pages in a space by name.
    #[query]
    async fn get_pages_in_space_by_name(
        &self,
        space_name: String,
    ) -> Result<Vec<ContentDetails>, String> {
        let space_id = self.get_space_id_from_name(space_name.clone()).await?;
        self.get_pages_in_space_by_id(space_id)
            .await
            .map_err(|e| e.to_string())
    }

    /// JSON schema describing exposed tools for LLM function-calling.
    #[query]
    fn tools(&self) -> String {
        r#"[
  {
    "type": "function",
    "function": {
      "name": "list_spaces",
      "description": "list spaces in confluence\n",
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
      "name": "create_page_by_space_id",
      "description": "create a page in confluence, prividing the space id. The content should be in Confluence Native Markup Language.\n",
      "parameters": {
        "type": "object",
        "properties": {
          "space_id": {
            "type": "integer",
            "description": "id of the space, integer\n"
          },
          "title": {
            "type": "string",
            "description": "\n"
          },
          "content": {
            "type": "string",
            "description": "\n"
          }
        },
        "required": [
          "space_id",
          "title",
          "content"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "create_page_with_table_by_space_name_with_parent_page",
      "description": "create page with table structured input as content, with a specified parent page\n",
      "parameters": {
        "type": "object",
        "properties": {
          "space_name": {
            "type": "string",
            "description": "space name\n"
          },
          "title": {
            "type": "string",
            "description": "new page's title\n"
          },
          "parent_page_name": {
            "type": "string",
            "description": "name of the parent page\n"
          },
          "headers": {
            "type": "string",
            "description": "headers or column names of the table. format : [header1, header2, header3, ...] , i.e. a serialized version of a list. Use double quotes, not single quotes.\n"
          },
          "rows": {
            "type": "string",
            "description": "Complete row data as a single JSON string. Pass the entire 2D array structure as one stringified JSON value. Example value: \\\"[[\\\\\\\"row11\\\\\\\", \\\\\\\"row12\\\\\\\"], [\\\\\\\"row21\\\\\\\", \\\\\\\"row22\\\\\\\"]]\\\" (note this is ONE string, not an array). Do not pass multiple array elements - serialize everything into a single string. Use double quotes, not single quotes.\n"
          }
        },
        "required": [
          "space_name",
          "title",
          "parent_page_name",
          "headers",
          "rows"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "import_file_and_create_page_with_table_with_parent_page",
      "description": "Reads a file from the encoded filedescriptor (like ey9320... ), and then create page with table structured input as content, with a specified parent page.\n",
      "parameters": {
        "type": "object",
        "properties": {
          "space_name": {
            "type": "string",
            "description": ""
          },
          "title": {
            "type": "string",
            "description": ""
          },
          "parent_page_name": {
            "type": "string",
            "description": ""
          },
          "file_descriptor": {
            "type": "string",
            "description": "The base64 encoded file descriptor to the file to read and upload\n"
          }
        },
        "required": [
          "space_name",
          "title",
          "parent_page_name",
          "file_descriptor"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "import_file_and_create_page_with_table_without_parent_page",
      "description": "Reads a file from the encoded filedescriptor (like ey9320... ), and then create page with table structured input as content, without a specified parent page.\n",
      "parameters": {
        "type": "object",
        "properties": {
          "space_name": {
            "type": "string",
            "description": ""
          },
          "title": {
            "type": "string",
            "description": ""
          },
          "file_descriptor": {
            "type": "string",
            "description": "The base64 encoded file descriptor to the file to read and upload\n"
          }
        },
        "required": [
          "space_name",
          "title",
          "file_descriptor"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "create_page_with_table_by_space_name_without_parent_page",
      "description": "create page with table structured input as content, without a specified parent page\n",
      "parameters": {
        "type": "object",
        "properties": {
          "space_name": {
            "type": "string",
            "description": ""
          },
          "title": {
            "type": "string",
            "description": "new page's title\n"
          },
          "headers": {
            "type": "string",
            "description": "headers or column names of the table. format : [header1, header2, header3, ...] , i.e. a serialized version of a list. Use double quotes, not single quotes.\n"
          },
          "rows": {
            "type": "string",
            "description": "Complete row data as a single JSON string. Pass the entire 2D array structure as one stringified JSON value. Example value: \\\"[[\\\\\\\"row11\\\\\\\", \\\\\\\"row12\\\\\\\"], [\\\\\\\"row21\\\\\\\", \\\\\\\"row22\\\\\\\"]]\\\" (note this is ONE string, not an array). Do not pass multiple array elements - serialize everything into a single string. Use double quotes, not single quotes.\n"
          }
        },
        "required": [
          "space_name",
          "title",
          "headers",
          "rows"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "append_to_page_by_id",
      "description": "append content to page , in confluence. The content should be in Confluence Native Markup Language.\n",
      "parameters": {
        "type": "object",
        "properties": {
          "page_id": {
            "type": "integer",
            "description": ""
          },
          "content": {
            "type": "string",
            "description": ""
          }
        },
        "required": [
          "page_id",
          "content"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "import_file_and_append_table_to_page_by_page_name",
      "description": "Reads a file from the encoded filedescriptor (like ey9320... ) and appends content in table structured format, to a given page.\n",
      "parameters": {
        "type": "object",
        "properties": {
          "file_descriptor": {
            "type": "string",
            "description": "The base64 encoded file descriptor to the file to read and upload\n"
          },
          "page_name": {
            "type": "string",
            "description": ""
          },
          "space_name": {
            "type": "string",
            "description": ""
          }
        },
        "required": [
          "file_descriptor",
          "page_name",
          "space_name"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "append_to_page_by_page_name",
      "description": "append content to page , in confluence. The content should be in Confluence Native Markup Language.\n",
      "parameters": {
        "type": "object",
        "properties": {
          "page_name": {
            "type": "string",
            "description": ""
          },
          "space_name": {
            "type": "string",
            "description": ""
          },
          "content": {
            "type": "string",
            "description": ""
          }
        },
        "required": [
          "page_name",
          "space_name",
          "content"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "append_table_to_page_by_page_name",
      "description": "append content in table structured format , to a given page.\n",
      "parameters": {
        "type": "object",
        "properties": {
          "page_name": {
            "type": "string",
            "description": ""
          },
          "space_name": {
            "type": "string",
            "description": ""
          },
          "headers": {
            "type": "string",
            "description": "headers or column names of the table. format : [header1, header2, header3, ...] , i.e. a serialized version of a list. Use double quotes, not single quotes.\n"
          },
          "rows": {
            "type": "string",
            "description": "Complete row data as a single JSON string. Pass the entire 2D array structure as one stringified JSON value. Example value: \\\"[[\\\\\\\"row11\\\\\\\", \\\\\\\"row12\\\\\\\"], [\\\\\\\"row21\\\\\\\", \\\\\\\"row22\\\\\\\"]]\\\" (note this is ONE string, not an array). Do not pass multiple array elements - serialize everything into a single string. Use double quotes, not single quotes.\n"
          }
        },
        "required": [
          "page_name",
          "space_name",
          "headers",
          "rows"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "create_page_by_space_name",
      "description": "create a page in confluence, providing the space name. The content should be in Confluence Native Markup Language.\n",
      "parameters": {
        "type": "object",
        "properties": {
          "space_name": {
            "type": "string",
            "description": "name of the space\n"
          },
          "title": {
            "type": "string",
            "description": "\n"
          },
          "content": {
            "type": "string",
            "description": "\n"
          }
        },
        "required": [
          "space_name",
          "title",
          "content"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "create_page_by_space_id_with_parent_page_id",
      "description": "create a page in confluence, prividing the space id and parent page id. The content should be in Confluence Native Markup Language.\n",
      "parameters": {
        "type": "object",
        "properties": {
          "space_id": {
            "type": "integer",
            "description": "id of the space, integer\n"
          },
          "title": {
            "type": "string",
            "description": "\n"
          },
          "parent_id": {
            "type": "integer",
            "description": "id of the parent page\n"
          },
          "content": {
            "type": "string",
            "description": "\n"
          }
        },
        "required": [
          "space_id",
          "title",
          "parent_id",
          "content"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "create_page_by_space_name_with_parent_page_name",
      "description": "create a page in confluence, providing the space name and parent page name. The content should be in Confluence Native Markup Language.\n",
      "parameters": {
        "type": "object",
        "properties": {
          "space_name": {
            "type": "string",
            "description": "name of the space\n"
          },
          "title": {
            "type": "string",
            "description": "\n"
          },
          "parent_page_name": {
            "type": "string",
            "description": "id of the parent page\n"
          },
          "content": {
            "type": "string",
            "description": "\n"
          }
        },
        "required": [
          "space_name",
          "title",
          "parent_page_name",
          "content"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "get_page_by_id",
      "description": "get page information by id\n",
      "parameters": {
        "type": "object",
        "properties": {
          "page_id": {
            "type": "integer",
            "description": "id of the page. get it from the get_pages_in_space or create_page function. this is an integer\n"
          }
        },
        "required": [
          "page_id"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "get_page_by_name",
      "description": "get page information by page title\n",
      "parameters": {
        "type": "object",
        "properties": {
          "page_name": {
            "type": "string",
            "description": "name of the page\n"
          },
          "space_name": {
            "type": "string",
            "description": "name of the space\n"
          }
        },
        "required": [
          "page_name",
          "space_name"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "update_page_by_id",
      "description": "update an existing page by page id. The content should be in Confluence Native Markup Language.\n",
      "parameters": {
        "type": "object",
        "properties": {
          "page_id": {
            "type": "integer",
            "description": "id of the page. passed as integer\n"
          },
          "space_id": {
            "type": "integer",
            "description": "id of the space. passed as integer\n"
          },
          "new_title": {
            "type": "string",
            "description": "\n"
          },
          "new_content": {
            "type": "string",
            "description": "\n"
          }
        },
        "required": [
          "page_id",
          "new_title",
          "new_content"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "update_page_by_name",
      "description": "updata an existing page by page name. The content should be in Confluence Native Markup Language.\n",
      "parameters": {
        "type": "object",
        "properties": {
          "page_name": {
            "type": "string",
            "description": "name of the page.\n"
          },
          "space_name": {
            "type": "string",
            "description": "name of the space\n"
          },
          "new_title": {
            "type": "string",
            "description": "\n"
          },
          "new_content": {
            "type": "string",
            "description": "\n"
          }
        },
        "required": [
          "page_name",
          "space_name",
          "new_title",
          "new_content"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "delete_page",
      "description": "delete a page by id\n",
      "parameters": {
        "type": "object",
        "properties": {
          "page_id": {
            "type": "integer",
            "description": "id of the page. get it from the get_pages_in_space or create_page function, passed as integer\n"
          }
        },
        "required": [
          "page_id"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "create_blog_post_by_space_id",
      "description": "create a blog post in a space by providing the space_id. The content should be in Confluence Native Markup Language.\n",
      "parameters": {
        "type": "object",
        "properties": {
          "space_id": {
            "type": "integer",
            "description": "id of the space, passed as integer\n"
          },
          "title": {
            "type": "string",
            "description": "\n"
          },
          "content": {
            "type": "string",
            "description": "\n"
          }
        },
        "required": [
          "space_id",
          "title",
          "content"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "create_blog_post_by_space_name",
      "description": "create a blog post in a space , providing the space_name. The content should be in Confluence Native Markup Language.\n",
      "parameters": {
        "type": "object",
        "properties": {
          "space_name": {
            "type": "string",
            "description": "name of the space.\n"
          },
          "title": {
            "type": "string",
            "description": "\n"
          },
          "content": {
            "type": "string",
            "description": "\n"
          }
        },
        "required": [
          "space_name",
          "title",
          "content"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "get_blog_post_by_id",
      "description": "get blog post information by id.\n",
      "parameters": {
        "type": "object",
        "properties": {
          "blog_post_id": {
            "type": "integer",
            "description": "id of the blog post, passed as integer\n"
          }
        },
        "required": [
          "blog_post_id"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "get_blog_post_by_name",
      "description": "get blog post information by name\n",
      "parameters": {
        "type": "object",
        "properties": {
          "blog_post_name": {
            "type": "string",
            "description": "name of the blog post\n"
          },
          "space_name": {
            "type": "string",
            "description": ""
          }
        },
        "required": [
          "blog_post_name",
          "space_name"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "update_blog_post_by_id",
      "description": "update an existing blog post by id. The content should be in Confluence Native Markup Language.\n",
      "parameters": {
        "type": "object",
        "properties": {
          "blog_post_id": {
            "type": "integer",
            "description": "id of the blog post, passed as integer\n"
          },
          "space_id": {
            "type": "integer",
            "description": "id of the space, passed as integer\n"
          },
          "new_title": {
            "type": "string",
            "description": "\n"
          },
          "new_content": {
            "type": "string",
            "description": "\n"
          },
          "new_version_number": {
            "type": "integer",
            "description": "\n"
          }
        },
        "required": [
          "blog_post_id",
          "new_title",
          "new_content"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "update_blog_post_by_name",
      "description": "update an existing blog post by name. The content should be in Confluence Native Markup Language.\n",
      "parameters": {
        "type": "object",
        "properties": {
          "blog_post_name": {
            "type": "string",
            "description": "name of the blog post\n"
          },
          "space_name": {
            "type": "string",
            "description": "name of the space\n"
          },
          "new_title": {
            "type": "string",
            "description": "\n"
          },
          "new_content": {
            "type": "string",
            "description": "\n"
          },
          "new_version_number": {
            "type": "integer",
            "description": "\n"
          }
        },
        "required": [
          "blog_post_name",
          "space_name",
          "new_title",
          "new_content"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "delete_blog_post",
      "description": "delete a blog post by id\n",
      "parameters": {
        "type": "object",
        "properties": {
          "blog_post_id": {
            "type": "integer",
            "description": "id of the blog post. get it from the list_blog_posts_in_space or create_blog_post function, passed as integer\n"
          }
        },
        "required": [
          "blog_post_id"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "list_blog_posts_in_space_by_id",
      "description": "list all blog posts in a space, with space_id passed in argument.\n",
      "parameters": {
        "type": "object",
        "properties": {
          "space_id": {
            "type": "integer",
            "description": "space id, passed as integer\n"
          }
        },
        "required": [
          "space_id"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "list_blog_posts_in_space_by_name",
      "description": "list all blog posts in a space, with space_name passed in argument\n",
      "parameters": {
        "type": "object",
        "properties": {
          "space_name": {
            "type": "string",
            "description": "\n"
          }
        },
        "required": [
          "space_name"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "list_page_labels_by_id",
      "description": "list all labels on a page, providing page id\n",
      "parameters": {
        "type": "object",
        "properties": {
          "page_id": {
            "type": "integer",
            "description": "page id , passed as integer\n"
          }
        },
        "required": [
          "page_id"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "list_page_labels_by_name",
      "description": "list all labels on a page, providing page name\n",
      "parameters": {
        "type": "object",
        "properties": {
          "page_name": {
            "type": "string",
            "description": "\n"
          },
          "space_name": {
            "type": "string",
            "description": ""
          }
        },
        "required": [
          "page_name",
          "space_name"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "list_space_labels_by_id",
      "description": "list all labels in a space , providing space id\n",
      "parameters": {
        "type": "object",
        "properties": {
          "space_id": {
            "type": "integer",
            "description": "space id , passed as integer\n"
          }
        },
        "required": [
          "space_id"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "list_space_labels_by_name",
      "description": "list all labels in a space , providing space name\n",
      "parameters": {
        "type": "object",
        "properties": {
          "space_name": {
            "type": "string",
            "description": "\n"
          }
        },
        "required": [
          "space_name"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "list_blog_post_labels_by_id",
      "description": "list all labels on a blog post, providing blog post id\n",
      "parameters": {
        "type": "object",
        "properties": {
          "blog_post_id": {
            "type": "integer",
            "description": "blog post id , passed as integer\n"
          }
        },
        "required": [
          "blog_post_id"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "list_blog_post_labels_by_name",
      "description": "list all labels on a blog post, providing blog post name\n",
      "parameters": {
        "type": "object",
        "properties": {
          "blog_post_name": {
            "type": "string",
            "description": "\n"
          },
          "space_name": {
            "type": "string",
            "description": ""
          }
        },
        "required": [
          "blog_post_name",
          "space_name"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "create_footer_comment_on_page_by_page_id",
      "description": "create a footer comment on a page, by providing the page id\n",
      "parameters": {
        "type": "object",
        "properties": {
          "page_id": {
            "type": "string",
            "description": "page id, passed as integer value\n"
          },
          "content": {
            "type": "string",
            "description": "\n"
          }
        },
        "required": [
          "page_id",
          "content"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "create_footer_comment_on_page_by_page_name",
      "description": "create a footer comment on a page, by providing the page name\n",
      "parameters": {
        "type": "object",
        "properties": {
          "page_name": {
            "type": "string",
            "description": ""
          },
          "space_name": {
            "type": "string",
            "description": ""
          },
          "content": {
            "type": "string",
            "description": ""
          }
        },
        "required": [
          "page_name",
          "space_name",
          "content"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "create_footer_comment_on_blog_post_by_id",
      "description": "create a footer comment on a blog post, by providing the blog post id\n",
      "parameters": {
        "type": "object",
        "properties": {
          "blog_post_id": {
            "type": "integer",
            "description": "blog post id, passed as integer\n"
          },
          "content": {
            "type": "string",
            "description": "\n"
          }
        },
        "required": [
          "blog_post_id",
          "content"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "create_footer_comment_on_blog_post_by_name",
      "description": "create a footer comment on a blog post, by providing the blog post name\n",
      "parameters": {
        "type": "object",
        "properties": {
          "blog_post_name": {
            "type": "string",
            "description": "\n"
          },
          "space_name": {
            "type": "string",
            "description": "\n"
          },
          "content": {
            "type": "string",
            "description": "\n"
          }
        },
        "required": [
          "blog_post_name",
          "space_name",
          "content"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "reply_footer_comment",
      "description": "reply to a footer comment\n",
      "parameters": {
        "type": "object",
        "properties": {
          "parent_comment_id": {
            "type": "integer",
            "description": "parent commit id, paseed as integer\n"
          },
          "content": {
            "type": "string",
            "description": "\n"
          }
        },
        "required": [
          "parent_comment_id",
          "content"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "get_footer_comment_by_id",
      "description": "get a footer comment by id\n",
      "parameters": {
        "type": "object",
        "properties": {
          "comment_id": {
            "type": "integer",
            "description": "comment id, passed as integer\n"
          }
        },
        "required": [
          "comment_id"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "list_page_direct_footer_comments_by_id",
      "description": "list direct footer comments on a page, providing the page id\n",
      "parameters": {
        "type": "object",
        "properties": {
          "page_id": {
            "type": "integer",
            "description": "page id, passed as integer\n"
          }
        },
        "required": [
          "page_id"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "list_page_direct_footer_comments_by_name",
      "description": "list direct footer comments on a page, providing the page name\n",
      "parameters": {
        "type": "object",
        "properties": {
          "page_name": {
            "type": "string",
            "description": "\n"
          },
          "space_name": {
            "type": "string",
            "description": ""
          }
        },
        "required": [
          "page_name",
          "space_name"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "update_comment",
      "description": "update a comment\n",
      "parameters": {
        "type": "object",
        "properties": {
          "comment_id": {
            "type": "integer",
            "description": "comment id, passed as integer\n"
          },
          "new_content": {
            "type": "string",
            "description": "\n"
          }
        },
        "required": [
          "comment_id",
          "new_content"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "delete_comment",
      "description": "delete a comment by id\n",
      "parameters": {
        "type": "object",
        "properties": {
          "comment_id": {
            "type": "integer",
            "description": "comment id, passed as integer\n"
          }
        },
        "required": [
          "comment_id"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "list_blog_post_direct_footer_comments_by_id",
      "description": "list direct footer comments on a blog post, providing the blog post id\n",
      "parameters": {
        "type": "object",
        "properties": {
          "blog_post_id": {
            "type": "integer",
            "description": "blog post id, passed as integer\n"
          }
        },
        "required": [
          "blog_post_id"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "list_blog_post_direct_footer_comments_by_name",
      "description": "list direct footer comments on a blog post, providing the blog post name\n",
      "parameters": {
        "type": "object",
        "properties": {
          "blog_post_name": {
            "type": "string",
            "description": "\n"
          },
          "space_name": {
            "type": "string",
            "description": "\n"
          }
        },
        "required": [
          "blog_post_name",
          "space_name"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "get_comment_children",
      "description": "get children of a comment, providing the comment id\n",
      "parameters": {
        "type": "object",
        "properties": {
          "comment_id": {
            "type": "integer",
            "description": "\n"
          }
        },
        "required": [
          "comment_id"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "list_space_permissions",
      "description": "list all permissions for a space\n",
      "parameters": {
        "type": "object",
        "properties": {
          "space_id": {
            "type": "integer",
            "description": "space id, passed as integer\n"
          }
        },
        "required": [
          "space_id"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "get_page_children",
      "description": "get the direct children of a page\n",
      "parameters": {
        "type": "object",
        "properties": {
          "page_id": {
            "type": "integer",
            "description": "page id , passed as integer\n"
          }
        },
        "required": [
          "page_id"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "get_page_descendants_by_page_id",
      "description": "get all descendants of a page, providing the page id\n",
      "parameters": {
        "type": "object",
        "properties": {
          "page_id": {
            "type": "integer",
            "description": "page id, passed as integer\n"
          }
        },
        "required": [
          "page_id"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "get_page_ancestors_by_page_id",
      "description": "get all ancestors of a page, providing the page id\n",
      "parameters": {
        "type": "object",
        "properties": {
          "page_id": {
            "type": "integer",
            "description": "page id , passed as integer\n"
          }
        },
        "required": [
          "page_id"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "get_page_descendants_by_page_name",
      "description": "get all descendants of a page, providing the page name\n",
      "parameters": {
        "type": "object",
        "properties": {
          "page_name": {
            "type": "string",
            "description": "\n"
          },
          "space_name": {
            "type": "string",
            "description": "\n"
          }
        },
        "required": [
          "page_name",
          "space_name"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "get_page_ancestors_by_page_name",
      "description": "get all ancestors of a page, providing the page name\n",
      "parameters": {
        "type": "object",
        "properties": {
          "page_name": {
            "type": "string",
            "description": "\n"
          },
          "space_name": {
            "type": "string",
            "description": "\n"
          }
        },
        "required": [
          "page_name",
          "space_name"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "get_pages_in_space_by_id",
      "description": "get all pages in a space with the given id\n",
      "parameters": {
        "type": "object",
        "properties": {
          "space_id": {
            "type": "integer",
            "description": "space id, passed as integer\n"
          }
        },
        "required": [
          "space_id"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "get_pages_in_space_by_name",
      "description": "get all pages in a space with the given space name\n",
      "parameters": {
        "type": "object",
        "properties": {
          "space_name": {
            "type": "string",
            "description": "\n"
          }
        },
        "required": [
          "space_name"
        ]
      }
    }
  }
]"#.to_string()
    }

    /// Placeholder for prompt packs used by agentic flows.
    #[query]
    fn prompts(&self) -> String {
        "{\"prompts\": []}".to_string()
    }
    // Add more tools here following the same pattern for all operations (blog posts, labels, comments, properties, etc.)
}
