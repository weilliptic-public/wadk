//! ServiceNow Data Structures and Custom Deserializers
//! 
//! This module contains all the data structures used to interact with ServiceNow's REST API,
//! along with custom deserializers to handle ServiceNow's unique JSON response formats.
//! 
//! ## Key Features:
//! - **Custom Boolean Deserializer**: Handles ServiceNow's string-based boolean values
//! - **Reference Field Deserializer**: Handles ServiceNow's reference objects with link/value structure
//! - **Comprehensive Struct Coverage**: All major ServiceNow tables and entities
//! - **Proper Error Handling**: Graceful handling of missing or malformed fields

use serde::{Deserialize, Serialize, Deserializer};
use weil_macros::WeilType;

/// Custom deserializer for ServiceNow boolean fields that are returned as strings
/// 
/// **Why we need this:**
/// ServiceNow's REST API often returns boolean values as strings ("true"/"false") instead of 
/// actual JSON boolean values. This is common in fields like `active`, `mandatory`, etc.
/// 
/// **What it does:**
/// - Accepts either a string or null from the JSON response
/// - Converts string "true" → Some(true), string "false" → Some(false)
/// - Returns None for null values or invalid strings
/// - Handles the Option<bool> type that ServiceNow uses for nullable boolean fields
/// 
/// **Example ServiceNow response:**
/// ```json
/// {
///   "active": "true",     // String instead of boolean
///   "mandatory": "false"  // String instead of boolean
/// }
/// ```
fn deserialize_string_to_bool<'de, D>(deserializer: D) -> Result<Option<bool>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: Option<String> = Option::deserialize(deserializer)?;
    match s {
        Some(s) => {
            match s.as_str() {
                "true" => Ok(Some(true)),
                "false" => Ok(Some(false)),
                _ => Ok(None), // Invalid string values become None
            }
        }
        None => Ok(None), // Null values become None
    }
}

/// Custom deserializer for ServiceNow reference fields that can be strings or objects
/// 
/// **Why we need this:**
/// ServiceNow's REST API returns reference fields (like `assigned_to`, `requested_by`, `department`)
/// in two different formats depending on the context:
/// 1. As simple strings (just the sys_id)
/// 2. As reference objects with `link` and `value` properties
/// 
/// **What it does:**
/// - Handles both string and object formats seamlessly
/// - For strings: extracts the value directly
/// - For objects: extracts the `value` field from `{"link": "...", "value": "sys_id"}`
/// - Returns None for null values or malformed objects
/// - Uses Serde's Visitor pattern to handle multiple data types
/// 
/// **Example ServiceNow responses:**
/// ```json
/// // Simple string format:
/// {
///   "assigned_to": "1234567890abcdef"
/// }
/// 
/// // Reference object format:
/// {
///   "assigned_to": {
///     "link": "https://instance.service-now.com/api/now/table/sys_user/1234567890abcdef",
///     "value": "1234567890abcdef"
///   }
/// }
/// ```
fn deserialize_reference_field<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::{self, Visitor};
    use std::fmt;
    
    struct ReferenceFieldVisitor;
    
    impl<'de> Visitor<'de> for ReferenceFieldVisitor {
        type Value = Option<String>;
        
        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a string or a reference object")
        }
        
        // Handle string values directly
        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(Some(value.to_string()))
        }
        
        // Handle owned string values
        fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(Some(value))
        }
        
        // Handle null values
        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }
        
        // Handle Some(string) values
        fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: Deserializer<'de>,
        {
            // Try to deserialize as a string first
            match String::deserialize(deserializer) {
                Ok(s) => Ok(Some(s)),
                Err(_) => {
                    // If that fails, return None for now
                    Ok(None)
                }
            }
        }
        
        // Handle reference objects with link/value structure
        fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
        where
            M: de::MapAccess<'de>,
        {
            // Extract the "value" field from the reference object
            let mut value = None;
            while let Some(key) = map.next_key::<String>()? {
                if key == "value" {
                    value = map.next_value()?;
                } else {
                    // Ignore other fields like "link", "display_value", etc.
                    map.next_value::<de::IgnoredAny>()?;
                }
            }
            Ok(value)
        }
    }
    
    deserializer.deserialize_any(ReferenceFieldVisitor)
}

// ============================================================================
// CONFIGURATION STRUCTURES
// ============================================================================

/// Configuration structure for ServiceNow API connection
#[derive(Debug, Serialize, Deserialize, WeilType, Default)]
pub struct ServicenowConfig {
    pub base_url: String,
    pub username: String,
    pub password: String,
}

// ============================================================================
// INCIDENT MANAGEMENT STRUCTURES
// ============================================================================

/// ServiceNow Incident record structure
#[derive(Debug, Serialize, Deserialize)]
pub struct Incident {
    sys_id: Option<String>,
    number: Option<String>,
    short_description: Option<String>,
    description: Option<String>,
    priority: Option<String>,
    state: Option<String>,
}

/// ServiceNow Comment record structure
#[derive(Debug, Serialize, Deserialize)]
pub struct Comment {
    sys_id: Option<String>,
    comments: Option<String>,
    sys_created_on: Option<String>,
    #[serde(deserialize_with = "deserialize_reference_field")]
    sys_created_by: Option<String>,
}

// ============================================================================
// SERVICE CATALOG STRUCTURES
// ============================================================================

/// ServiceNow Catalog Item record structure
#[derive(Debug, Serialize, Deserialize)]
pub struct CatalogItem {
    sys_id: Option<String>,
    name: Option<String>,
    short_description: Option<String>,
    description: Option<String>,
    category: Option<String>,
    #[serde(deserialize_with = "deserialize_string_to_bool")]
    active: Option<bool>,
    sys_created_on: Option<String>,
}

/// ServiceNow Catalog Category record structure
#[derive(Debug, Serialize, Deserialize)]
pub struct CatalogCategory {
    sys_id: Option<String>,
    title: Option<String>,
    description: Option<String>,
    parent: Option<String>,
    #[serde(deserialize_with = "deserialize_string_to_bool")]
    active: Option<bool>,
}

/// ServiceNow Catalog Variable record structure
#[derive(Debug, Serialize, Deserialize, WeilType, Default)]
pub struct CatalogVariable {
    sys_id: Option<String>,
    name: Option<String>,
    question_text: Option<String>,
    r#type: Option<String>,
    #[serde(deserialize_with = "deserialize_string_to_bool")]
    mandatory: Option<bool>,
    catalog_item: Option<String>,
}

// ============================================================================
// CHANGE MANAGEMENT STRUCTURES
// ============================================================================

/// ServiceNow Change Request record structure
#[derive(Debug, Serialize, Deserialize)]
pub struct ChangeRequest {
    sys_id: Option<String>,
    number: Option<String>,
    short_description: Option<String>,
    description: Option<String>,
    state: Option<String>,
    priority: Option<String>,
    risk: Option<String>,
    impact: Option<String>,
    #[serde(deserialize_with = "deserialize_reference_field")]
    requested_by: Option<String>,
    requested_date: Option<String>,
}

/// ServiceNow Change Task record structure
#[derive(Debug, Serialize, Deserialize)]
pub struct ChangeTask {
    sys_id: Option<String>,
    number: Option<String>,
    short_description: Option<String>,
    description: Option<String>,
    state: Option<String>,
    change_request: Option<String>,
    #[serde(deserialize_with = "deserialize_reference_field")]
    assigned_to: Option<String>,
}

// ============================================================================
// AGILE/SCRUM STRUCTURES
// ============================================================================

/// ServiceNow Story record structure (Agile Development)
#[derive(Debug, Serialize, Deserialize)]
pub struct Story {
    sys_id: Option<String>,
    number: Option<String>,
    short_description: Option<String>,
    description: Option<String>,
    state: Option<String>,
    priority: Option<String>,
    story_points: Option<String>,
    #[serde(deserialize_with = "deserialize_reference_field")]
    epic: Option<String>,
}

/// ServiceNow Epic record structure (Agile Development)
#[derive(Debug, Serialize, Deserialize)]
pub struct Epic {
    sys_id: Option<String>,
    number: Option<String>,
    short_description: Option<String>,
    description: Option<String>,
    state: Option<String>,
    priority: Option<String>,
    #[serde(deserialize_with = "deserialize_reference_field")]
    assigned_to: Option<String>,
}

/// ServiceNow Scrum Task record structure (Agile Development)
#[derive(Debug, Serialize, Deserialize)]
pub struct ScrumTask {
    sys_id: Option<String>,
    number: Option<String>,
    short_description: Option<String>,
    description: Option<String>,
    state: Option<String>,
    priority: Option<String>,
    #[serde(deserialize_with = "deserialize_reference_field")]
    story: Option<String>,
    #[serde(deserialize_with = "deserialize_reference_field")]
    assigned_to: Option<String>,
}

// ============================================================================
// PROJECT MANAGEMENT STRUCTURES
// ============================================================================

/// ServiceNow Project record structure (Project Management)
#[derive(Debug, Serialize, Deserialize)]
pub struct Project {
    sys_id: Option<String>,
    name: Option<String>,
    short_description: Option<String>,
    state: Option<String>,
    number: Option<String>,
    status: Option<String>,
    #[serde(deserialize_with = "deserialize_reference_field")]
    goal: Option<String>,
}

// ============================================================================
// WORKFLOW STRUCTURES
// ============================================================================

/// ServiceNow Workflow record structure
#[derive(Debug, Serialize, Deserialize)]
pub struct Workflow {
    sys_id: Option<String>,
    name: Option<String>,
    description: Option<String>,
    #[serde(deserialize_with = "deserialize_string_to_bool", default)]
    active: Option<bool>,
    table: Option<String>,
}

/// ServiceNow Script Include record structure
#[derive(Debug, Serialize, Deserialize)]
pub struct ScriptInclude {
    sys_id: Option<String>,
    name: Option<String>,
    description: Option<String>,
    script: Option<String>,
    #[serde(deserialize_with = "deserialize_string_to_bool")]
    active: Option<bool>,
    api_name: Option<String>,
}

// ============================================================================
// UPDATE SET/CHANGESET STRUCTURES
// ============================================================================

/// ServiceNow Changeset (Update Set) record structure
#[derive(Debug, Serialize, Deserialize)]
pub struct Changeset {
    sys_id: Option<String>,
    name: Option<String>,
    description: Option<String>,
    state: Option<String>,
    #[serde(rename = "sys_created_by", deserialize_with = "deserialize_reference_field")]
    created_by: Option<String>,
    #[serde(rename = "sys_created_on")]
    created_on: Option<String>,
    #[serde(rename = "completed_by", deserialize_with = "deserialize_reference_field")]
    completed_by: Option<String>,
    #[serde(rename = "completed_on")]
    completed_on: Option<String>,
}

// ============================================================================
// KNOWLEDGE MANAGEMENT STRUCTURES
// ============================================================================

/// ServiceNow Knowledge Base record structure
#[derive(Debug, Serialize, Deserialize)]
pub struct KnowledgeBase {
    sys_id: Option<String>,
    title: Option<String>,
    description: Option<String>,
    #[serde(deserialize_with = "deserialize_string_to_bool")]
    active: Option<bool>,
    #[serde(deserialize_with = "deserialize_reference_field")]
    owner: Option<String>,
}

/// ServiceNow Knowledge Article record structure
#[derive(Debug, Serialize, Deserialize)]
pub struct KnowledgeArticle {
    sys_id: Option<String>,
    number: Option<String>,
    short_description: Option<String>,
    text: Option<String>,
    state: Option<String>,
    knowledge_base: Option<String>,
    category: Option<String>,
    #[serde(deserialize_with = "deserialize_reference_field")]
    author: Option<String>,
}

// ============================================================================
// USER MANAGEMENT STRUCTURES
// ============================================================================

/// ServiceNow User record structure
#[derive(Debug, Serialize, Deserialize)]
pub struct User {
    sys_id: Option<String>,
    user_name: Option<String>,
    first_name: Option<String>,
    last_name: Option<String>,
    email: Option<String>,
    #[serde(deserialize_with = "deserialize_string_to_bool")]
    active: Option<bool>,
    #[serde(deserialize_with = "deserialize_reference_field")]
    department: Option<String>,
}

/// ServiceNow Group record structure
#[derive(Debug, Serialize, Deserialize)]
pub struct Group {
    sys_id: Option<String>,
    name: Option<String>,
    description: Option<String>,
    #[serde(deserialize_with = "deserialize_string_to_bool")]
    active: Option<bool>,
    #[serde(deserialize_with = "deserialize_reference_field")]
    manager: Option<String>,
}

// ============================================================================
// UI POLICY STRUCTURES
// ============================================================================

/// ServiceNow UI Policy record structure
#[derive(Debug, Serialize, Deserialize)]
pub struct UIPolicy {
    sys_id: Option<String>,
    #[serde(default)]
    name: Option<String>,
    description: Option<String>,
    #[serde(deserialize_with = "deserialize_string_to_bool")]
    active: Option<bool>,
    table: Option<String>,
    #[serde(deserialize_with = "deserialize_reference_field", default)]
    catalog_item: Option<String>,
}

/// ServiceNow UI Policy Action record structure
#[derive(Debug, Serialize, Deserialize)]
pub struct UIPolicyAction {
    sys_id: Option<String>,
    name: Option<String>,
    description: Option<String>,
    #[serde(deserialize_with = "deserialize_string_to_bool")]
    active: Option<bool>,
    #[serde(deserialize_with = "deserialize_reference_field")]
    ui_policy: Option<String>,
    field_name: Option<String>,
    action: Option<String>,
}
