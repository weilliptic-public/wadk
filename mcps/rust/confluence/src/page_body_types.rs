use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Document {
    pub r#type: String,
    pub content: Vec<Node>,
    pub version: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
pub enum Node {
    #[serde(rename = "paragraph")]
    Paragraph {
        #[serde(default)]
        content: Option<Vec<Content>>,
    },
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "hardBreak")]
    HardBreak,
    #[serde(rename = "table")]
    Table {
        attrs: TableAttrs,
        content: Vec<Node>, // tableRow
    },
    #[serde(rename = "tableRow")]
    TableRow {
        content: Vec<Node>, // tableHeader | tableCell
    },
    #[serde(rename = "tableHeader")]
    TableHeader {
        attrs: CellAttrs,
        content: Vec<Node>, // paragraph
    },
    #[serde(rename = "tableCell")]
    TableCell {
        attrs: CellAttrs,
        content: Vec<Node>, // paragraph
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Content {
    pub r#type: String,
    pub text: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TableAttrs {
    pub layout: String,
    pub width: Option<f64>,
    pub local_id: Option<String>,
    #[serde(default)]
    pub is_number_column_enabled: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CellAttrs {
    pub colspan: u32,
    pub rowspan: u32,
}
