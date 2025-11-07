//! # Arithmetic MCP Server (WeilChain Applet)
//!
//! A minimal Model Context Protocol (MCP) applet exposing simple
//! arithmetic utilities (`add`, `multiply`) for agentic workflows on WeilChain.
//!
//! ## Overview
//! - **Surface**: Two pure functions: integer addition and multiplication.
//! - **MCP Integration**: `tools()` describes callable functions for agents; `prompts()`
//!   is a placeholder for future prompt templates.
//! - **State**: This contract keeps no runtime state.
//!
//! ## Return & Error Semantics
//! - `add(x, y)` → returns `x + y` as `i32`.
//! - `multiply(x, y)` → returns `x * y` as `i32`.
//! - Constructor returns an empty state; no secrets or configuration needed.
//!
//! ## Typical Usage (pseudocode)
//! ```text
//! // 1) Deploy the contract
//! // 2) From an agent/runtime:
//! call tools()       -> discover 'add' and 'multiply'
//! call add{x, y}     -> receive integer sum
//! call multiply{x,y} -> receive integer product
//! ```

use serde::{Deserialize, Serialize};
use weil_macros::{WeilType, constructor, query, smart_contract};

/// Public MCP trait surface for basic integer arithmetic.
///
/// All methods are intended to be side-effect free:
/// - [`add`] and [`multiply`] compute results deterministically for given inputs.
/// - [`tools`] provides a JSON schema describing callable functions for MCP agents.
/// - [`prompts`] is reserved for future prompt templates (currently empty).
trait Arithmetic {
    /// Construct a new contract state.
    ///
    /// Returns an empty [`ArithmeticContractState`]. No configuration or secrets required.
    fn new() -> Result<Self, String>
    where
        Self: Sized;

    /// Compute the sum of two `i32` integers.
    ///
    /// # Parameters
    /// - `x`: Left operand.
    /// - `y`: Right operand.
    ///
    /// # Returns
    /// `x + y` as `i32`.
    async fn add(&self, x: i32, y: i32) -> i32;

    /// Compute the product of two `i32` integers.
    ///
    /// # Parameters
    /// - `x`: Left operand.
    /// - `y`: Right operand.
    ///
    /// # Returns
    /// `x * y` as `i32`.
    async fn multiply(&self, x: i32, y: i32) -> i32;

    /// JSON schema describing callable tools (for MCP/agent orchestration).
    ///
    /// The schema enumerates `add` and `multiply` with their parameter shapes.
    fn tools(&self) -> String;

    /// Placeholder for prompt templates used by agentic flows (currently empty).
    fn prompts(&self) -> String;
}

/// Contract state for the Arithmetic server.
///
/// This contract is stateless; the struct exists to satisfy the smart-contract framework.
#[derive(Serialize, Deserialize, WeilType)]
pub struct ArithmeticContractState {
    // define your contract state here!
}

#[smart_contract]
impl Arithmetic for ArithmeticContractState {
    /// Initialize an empty arithmetic contract.
    ///
    /// Returns a fresh [`ArithmeticContractState`]. No external configuration is required.
    #[constructor]
    fn new() -> Result<Self, String>
    where
        Self: Sized,
    {
        Ok(ArithmeticContractState {})
    }

    /// Add two integers and return the sum.
    ///
    /// # Examples
    /// ```
    /// // 2 + 3 = 5
    /// ```
    #[query]
    async fn add(&self, x: i32, y: i32) -> i32 {
        x + y
    }

    /// Multiply two integers and return the product.
    ///
    /// # Examples
    /// ```
    /// // 4 * 6 = 24
    /// ```
    #[query]
    async fn multiply(&self, x: i32, y: i32) -> i32 {
        x * y
    }

    /// Machine-readable tool specifications for MCP/agent runtimes.
    ///
    /// Exposes two functions: `add` and `multiply`, each taking integer `x` and `y`.
    #[query]
    fn tools(&self) -> String {
        r#"[
  {
    "type": "function",
    "function": {
      "name": "add",
      "description": "adds two numbers\n",
      "parameters": {
        "type": "object",
        "properties": {
          "x": {
            "type": "integer",
            "description": null
          },
          "y": {
            "type": "integer",
            "description": null
          }
        },
        "required": [
          "x",
          "y"
        ]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "multiply",
      "description": "multiply two numbers\n",
      "parameters": {
        "type": "object",
        "properties": {
          "x": {
            "type": "integer",
            "description": null
          },
          "y": {
            "type": "integer",
            "description": null
          }
        },
        "required": [
          "x",
          "y"
        ]
      }
    }
  }
]"#
        .to_string()
    }

    /// Placeholder for prompt templates. Currently returns an empty `prompts` array.
    #[query]
    fn prompts(&self) -> String {
        r#"{
  "prompts": []
}"#
        .to_string()
    }
}
