use crate::types::*;
use serde::{Deserialize, Serialize};

/// Response for listing spaces
#[derive(Debug, Serialize, Deserialize)]
pub struct ListSpacesResponse {
    /// Total number of spaces available
    pub total_spaces: u32,
    /// List of space keys
    pub space_keys: Vec<String>,
    /// Detailed information about each space
    pub spaces: Vec<SpaceSummary>,
}

/// Response for creating a new page
#[derive(Debug, Serialize, Deserialize)]
pub struct CreatePageResponse {
    /// Details of the created page
    pub page_id: String,
    pub title: String,
    pub status: String,
    pub space_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>, // TODO: timestamp-z type, to be deserialized later
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_type: Option<String>,
}

/// Response for getting a page
#[derive(Debug, Serialize, Deserialize)]
pub struct GetPageResponse {
    /// Full details of the page
    pub page: ContentDetails,
    /// URL of the page
    pub url: String,
    /// Whether the page has any children
    pub has_children: bool,
}

/// Response for updating a page
#[derive(Debug, Serialize, Deserialize)]
pub struct UpdatePageResponse {
    /// Updated page details
    pub page: ContentDetails,
    /// Previous version number
    pub previous_version: u32,
    /// New version number
    pub new_version: u32,
    /// URL of the updated page
    pub url: String,
}

/// Response for deleting a page
#[derive(Debug, Serialize, Deserialize)]
pub struct DeletePageResponse {
    /// Whether the deletion was successful
    pub success: bool,
    /// ID of the deleted page
    pub page_id: u64,
    /// Space ID where the page was located
    pub space_id: u64,
    /// Additional message about the deletion
    pub message: String,
}

/// Response for creating a blog post
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateBlogPostResponse {
    pub blog_post_id: String,
    pub title: String,
    pub status: String,
    pub space_id: String,
    pub version: PageVersion,
    pub body: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author_id: Option<String>,
}

/// Response for getting a blog post
#[derive(Debug, Serialize, Deserialize)]
pub struct GetBlogPostResponse {
    /// Full details of the blog post
    pub blog_post: ContentDetails,
    /// URL of the blog post
    pub url: String,
    /// Number of comments on the blog post
    pub comment_count: u32,
}

/// Response for listing labels
#[derive(Debug, Serialize, Deserialize)]
pub struct ListLabelsResponse {
    /// List of labels
    pub labels: Vec<Label>,
    /// Total number of labels
    pub total_count: u32,
    /// Type of content these labels are associated with
    pub content_type: String,
    /// ID of the content these labels are associated with
    pub content_id: u64,
}

/// Response for creating a comment
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateCommentResponse {
    /// Details of the created comment
    pub comment: CommentDetails,
    /// URL of the comment
    pub url: String,
    /// ID of the parent content
    pub content_id: u64,
    /// Type of the parent content (page, blogpost, etc.)
    pub content_type: String,
}

/// Response for listing comments
#[derive(Debug, Serialize, Deserialize)]
pub struct ListCommentsResponse {
    /// List of comments
    pub comments: Vec<CommentDetails>,
    /// Total number of comments
    pub total_count: u32,
    /// Whether there are more comments available
    pub has_more: bool,
    /// ID of the content these comments belong to
    pub content_id: u64,
    /// Type of the content these comments belong to
    pub content_type: String,
}

/// Response for space permissions
#[derive(Debug, Serialize, Deserialize)]
pub struct SpacePermissionsResponse {
    /// List of permissions
    pub permissions: Vec<SpacePermission>,
    /// Total number of permissions
    pub total_count: u32,
    /// ID of the space these permissions belong to
    pub space_id: u64,
}

/// Response for page hierarchy
#[derive(Debug, Serialize, Deserialize)]
pub struct PageHierarchyResponse {
    /// Root page of the hierarchy
    pub root: PageHierarchyItem,
    /// Total number of pages in the hierarchy
    pub total_pages: u32,
    /// Maximum depth of the hierarchy
    pub max_depth: u32,
}

/// Response for blog post hierarchy
#[derive(Debug, Serialize, Deserialize)]
pub struct BlogPostHierarchyResponse {
    /// List of blog posts in the space
    pub blog_posts: Vec<BlogPostHierarchyItem>,
    /// Total number of blog posts
    pub total_count: u32,
    /// ID of the space these blog posts belong to
    pub space_id: u64,
}

/// Response for listing pages in a space
#[derive(Debug, Serialize, Deserialize)]
pub struct ListPagesResponse {
    /// List of pages in the space
    pub pages: Vec<ContentDetails>,
    /// Total number of pages in the space
    pub total_count: u32,
    /// ID of the space these pages belong to
    pub space_id: u64,
    /// Whether there are more pages available
    pub has_more: bool,
}

/// Generic error response
#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    /// Error message
    pub error: String,
    /// Error code, if available
    pub code: Option<String>,
    /// Additional error details
    pub details: Option<serde_json::Value>,
}
