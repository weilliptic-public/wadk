use crate::traits::WeilType;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum Model {
    GPT_5POINT1,
    CLAUDE_SONNET,
    MISTRAL_LARGE,
}

impl WeilType for Model {}
