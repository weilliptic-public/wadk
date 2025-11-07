use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::page_body_types::Document;

/// Represents version information for Confluence content
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PageVersion {
    /// The version number
    pub number: u32,
    /// Optional message describing the changes in this version
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Represents the body content of a Confluence page or blog post
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Body {
    pub storage: Option<StorageBody>,
    pub atlas_doc_format: Option<AtlasDocFormatBodyStr>, // this can then be deserialized to AtlasDocFormatBody.
}

/// Represents content in Atlas Document Format with parsed Document
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AtlasDocFormatBody {
    /// The document content
    pub value: Document,
    /// Format of the content (should be "atlas_doc_format")
    pub representation: String,
}

/// String-serialized version of Atlas Document Format content
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AtlasDocFormatBodyStr {
    /// The document content as a JSON string
    pub value: String,
    /// Format of the content (should be "atlas_doc_format")
    pub representation: String,
}

/// Represents storage-format content (wiki markup)
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StorageBody {
    /// Format of the content (should be "storage")
    pub representation: String,
    /// The actual content in storage format
    pub value: String,
}

// --- Generic & List Responses ---

/// Generic list response with pagination support
#[derive(Serialize, Deserialize, Debug)]
pub struct ListResponse<T> {
    /// The list of items in the current page
    pub results: Vec<T>,
    /// Pagination links
    #[serde(rename = "_links")]
    pub links: NextPageResult,
}

/// Pagination information for list responses
#[derive(Debug, Serialize, Deserialize)]
pub struct NextPageResult {
    /// URL to the next page of results, if available
    pub next: Option<String>,
}

/// Result of a delete operation
#[derive(Serialize, Deserialize, Debug)]
pub struct DeleteResult {
    /// Whether the deletion was successful
    pub success: bool,
    /// Status message about the deletion
    pub message: String,
}

// --- Page & Blog Post Structures ---

/// Summary information about Confluence content
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ContentSummary {
    /// Unique identifier of the content
    pub id: String,
    pub title: String,
    /// Status of the content (e.g., 'current', 'draft')
    pub status: String,
    /// ID of the space this content belongs to
    #[serde(skip_serializing_if = "Option::is_none")]
    pub space_id: Option<String>,
    /// ID of the parent content, if any
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
}

/// Detailed information about Confluence content
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ContentDetails {
    /// Unique identifier of the content
    pub id: String,
    pub title: String,
    /// Status of the content (e.g., 'current', 'draft')
    pub status: String,
    /// ID of the space this content belongs to
    pub space_id: String,
    /// ID of the parent content, if any
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    /// Version information
    pub version: PageVersion,
    /// The content body in various formats
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<Body>,
    /// ID of the content owner, if any
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner_id: Option<String>,
    /// ID of the content author
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author_id: Option<String>,
    /// Creation timestamp (ISO 8601 format)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    /// Type of the parent content, if any
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_type: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CreateContentDetails {
    pub id: String,
    pub title: String,
    pub status: String,
    pub space_id: String,
    // #[serde(skip_serializing_if = "Option::is_none")]
    // pub parent_id: Option<String>,
    // pub version: PageVersion,
    // #[serde(skip_serializing_if = "Option::is_none")]
    // pub body: Option<PageBody>,
    // #[serde(skip_serializing_if = "Option::is_none")]
    // pub owner_id: Option<String>,
    // #[serde(skip_serializing_if = "Option::is_none")]
    // pub author_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>, // TODO: timestamp-z type, to be deserialized later
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_type: Option<String>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CreateContentRequest<'a> {
    /// ID of the space to create the content in
    pub space_id: u64,
    /// Title of the content
    pub title: &'a str,
    /// ID of the parent content, if any
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<u64>,
    /// The content body in ADF format
    pub body: AtlasDocFormatBodyStr,
}

/// Request to update existing content in Confluence
#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct UpdateContentRequest<'a> {
    /// ID of the content to update
    pub id: u64,
    /// New status for the content
    pub status: &'static str,
    /// New title for the content
    pub title: &'a str,
    /// New space ID, if moving the content
    pub space_id: Option<u64>,
    /// Updated content body in storage format
    pub body: StorageBody,
    /// New version information
    pub version: PageVersion,
}

/// Request to update content using ADF format
#[derive(Serialize, Debug)]
pub struct UpdateContentRequestAtlasDocFormat<'a> {
    /// ID of the content to update
    pub id: u64,
    /// New status for the content
    pub status: &'static str,
    /// New title for the content
    pub title: &'a str,
    /// New space ID, if moving the content
    pub space_id: Option<u64>,
    /// Updated content body in ADF format
    pub body: AtlasDocFormatBodyStr,
    /// New version information
    pub version: PageVersion,
}

// --- Space Structures ---

/// Summary information about a Confluence space
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SpaceSummary {
    /// Unique identifier of the space
    pub id: String,
    /// Key of the space
    pub key: String,
    /// Name of the space
    pub name: String,
    /// Type of the space (e.g., 'global', 'personal')
    #[serde(rename = "type")]
    pub space_type: String,
    /// Status of the space (e.g., 'current', 'archived')
    pub status: String,
}

// --- Label Structures ---

/// Represents a label in Confluence
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Label {
    /// Unique identifier of the label
    pub id: String,
    /// Name of the label
    pub name: String,
    /// Prefix of the label
    pub prefix: String,
}

// --- Comment Structures ---

/// Returned for both footer and inline comments.
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CommentDetails {
    /// Unique identifier of the comment
    pub id: String,
    /// Status of the comment (e.g., 'approved', 'pending')
    pub status: String,
    /// ID of the page or blog post this comment belongs to, if any
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_id: Option<String>,
    /// ID of the blog post this comment belongs to, if any
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blog_post_id: Option<String>,
    /// ID of the parent comment, if any
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_comment_id: Option<String>,
    /// Version information
    pub version: PageVersion,
    /// The comment body in various formats
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<Body>,
    /// Title of the comment, if any
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

/// Request to create a new comment on a piece of content
#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CreateCommentRequest {
    /// ID of the page or blog post to comment on, if any
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_id: Option<u64>,
    /// ID of the blog post to comment on, if any
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blog_post_id: Option<u64>,
    /// ID of the parent comment, if any
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_comment_id: Option<u64>,
    /// The comment body in storage format
    pub body: StorageBody,
    /// ID of the space this comment belongs to, if any
    pub space_id: Option<u64>,
}

/// Request to update an existing comment
#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCommentRequest {
    /// ID of the comment to update
    pub id: u64,
    /// New version information
    pub version: PageVersion,
    /// Updated comment body in storage format
    pub body: StorageBody,
}

// --- Space Permission (Role) Structures ---

/// Represents a permission (role) on a space
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SpacePermission {
    /// Unique identifier of the permission
    pub id: String,
    /// Principal (user or group) that holds this permission
    pub principal: PermissionPrincipal,
    /// Operation (e.g., 'view', 'edit') that this permission grants
    pub operation: PermissionOperation,
}

/// Represents a principal (user or group) that holds a permission
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PermissionPrincipal {
    #[serde(rename = "type")]
    pub principal_type: String, // "user" or "group"
    pub id: String,
}

/// Represents an operation (e.g., 'view', 'edit') that a permission grants
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PermissionOperation {
    /// Key of the operation (e.g., 'view', 'edit')
    pub key: String,
    /// Type of the target (e.g., 'space', 'page')
    pub target_type: String,
}

// --- Blog Post Hierarchy Structures ---

/// Represents a blog post in a hierarchy
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BlogPostHierarchyItem {
    /// Unique identifier of the blog post
    pub id: String,
    pub title: String,
    /// Status of the blog post (e.g., 'current', 'draft')
    pub status: String,
    /// ID of the space this blog post belongs to
    pub space_id: String,
    /// Creation timestamp (ISO 8601 format), if any
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author_id: Option<String>,
}

/// Response containing a list of blog posts in a hierarchy
#[derive(Serialize, Deserialize, Debug)]
pub struct BlogPostHierarchyResponse {
    /// List of blog posts in the hierarchy
    pub results: Vec<BlogPostHierarchyItem>,
    /// Total number of blog posts in the hierarchy
    pub total_count: usize,
}

// --- Enhanced Blog Post Details ---

/// Detailed information about a blog post
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BlogPostDetails {
    /// Unique identifier of the blog post
    pub id: String,
    pub title: String,
    /// Status of the blog post (e.g., 'current', 'draft')
    pub status: String,
    pub space_id: String,
    /// Version information
    pub version: PageVersion,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<Body>,
    /// Creation timestamp (ISO 8601 format), if any
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author_id: Option<String>,
}

// --- Page Hierarchy Structures ---

/// Represents a page in a hierarchy
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PageHierarchyItem {
    /// Unique identifier of the page
    pub id: String,
    /// Title of the page
    pub title: String,
    /// Status of the page (e.g., 'current', 'draft')
    pub status: String,
    /// ID of the parent page, if any
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    /// ID of the space this page belongs to, if any
    #[serde(skip_serializing_if = "Option::is_none")]
    pub space_id: Option<String>,
    /// Depth of the page in the hierarchy, if any
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depth: Option<u32>,
}

/// Response containing a list of pages in a hierarchy
#[derive(Serialize, Deserialize, Debug)]
pub struct PageHierarchyResponse {
    /// List of pages in the hierarchy
    pub results: Vec<PageHierarchyItem>,
    /// Total number of pages in the hierarchy
    pub total_count: usize,
}
