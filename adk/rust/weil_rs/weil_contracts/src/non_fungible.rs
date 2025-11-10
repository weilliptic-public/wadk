//! # Non-Fungible Token (NFT) contract (Ledger-backed)
//!
//! This module implements a simple NFT collection with ownership, per-token and
//! global (“approve for all”) allowances, plus mint and transfer flows. It is
//! backed by the platform **Ledger** applet for supply/transfer accounting and
//! uses `WeilMap` for persistent state.
//!
//! ## Key pieces
//! - [`Token`]: metadata payload for each NFT
//! - [`NonFungibleToken`]: collection state & methods
//! - [`DetailsFetchError`]: strongly-typed errors for `details`
//!
//! ## Allowance model
//! Allowances are stored as strings of the form `"<owner>$<token_id>" → <spender>`.
//! A special `token_id` of the empty string (`""`) represents **approve for all**.
//! Only one “approve for all” entry is allowed per owner.

use anyhow::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;
use weil_macros::WeilType;
use weil_rs::{
    collections::{map::WeilMap, WeilId},
    ledger::Ledger,
    runtime::Runtime,
};

/// Errors produced when fetching NFT details.
#[derive(Error, Debug, Serialize, Deserialize)]
pub enum DetailsFetchError {
    /// The provided token identifier is malformed or outside policy.
    #[error("invalid token id: `{0}`")]
    InvalidTokenId(TokenId),
    /// The token exists but has not been minted (owner is empty/burned).
    #[error("token `{0}` has not been minted")]
    TokenNotMinted(TokenId),
    /// The token record was not found in storage.
    #[error("token `{0}` not found")]
    TokenNotFound(TokenId),
}

/// Descriptive metadata attached to each NFT.
#[derive(Debug, Serialize, Deserialize, WeilType)]
pub struct Token {
    /// A title for the asset which this NFT represents.
    pub title: String,
    /// Identifies the asset which this NFT represents.
    pub name: String,
    /// Describes the asset which this NFT represents.
    pub description: String,
    /// A URI pointing to a resource with mime type `image/*` representing the asset.
    pub payload: String,
}

impl Token {
    /// Create a new [`Token`] metadata object.
    pub fn new(title: String, name: String, description: String, payload: String) -> Token {
        Token {
            title,
            name,
            description,
            payload,
        }
    }
}

// TODO: validate the token id in the various methods.
/// Token identifier with potential restrictions (to be enforced):
/// - disallowed special characters
/// - min/max length
/// - numeric constraints
pub type TokenId = String;

/// Sentinel for "no token" used in approve-for-all keys.
const EMPTY_TOKEN_ID: &str = "";

/// Address/identifier type for owners/spenders.
pub type Address = String;

/// Sentinel for "empty address" (e.g., burned token owner or revoke).
pub const EMPTY_ADDRESS: &str = "";

/// Persistent state for an NFT collection.
#[derive(Debug, Serialize, Deserialize, WeilType)]
pub struct NonFungibleToken {
    /// The name of this collection of tokens.
    name: String,
    /// Creator: address allowed to change the controllers set.
    creator: Address,
    /// Token metadata by `token_id`.
    tokens: WeilMap<TokenId, Token>,
    /// Current owner by `token_id`. `EMPTY_ADDRESS` means burned.
    owners: WeilMap<TokenId, Address>,
    /// Inverted index: `owner_addr` → set of owned `token_id`s.
    /// (Complementary to the Ledger applet.)
    owned: WeilMap<Address, BTreeSet<TokenId>>, // TODO: complement to the Ledger applet.
    /// `<owner address>$<TokenId Hex encoded>` → `<allowed address>`
    ///
    /// If an entry exists, `<owner>` authorized `<allowed>` to transfer `<token_id>`.
    /// If `<TokenId>` is the empty string, then all transfers from `<owner>` are
    /// authorized to `<allowed>` (“approve for all”). There can be only one such
    /// entry per owner.
    allowances: WeilMap<String, Address>,
}

impl NonFungibleToken {
    /// Create a new NFT collection, setting `creator` to the current sender.
    ///
    /// Initializes internal maps with deterministic IDs.
    pub fn new(name: String) -> Self {
        NonFungibleToken {
            name,
            creator: Runtime::sender(),
            tokens: WeilMap::new(WeilId(1)),
            owners: WeilMap::new(WeilId(2)),
            owned: WeilMap::new(WeilId(3)),
            allowances: WeilMap::new(WeilId(4)),
        }
    }

    /// Return the collection name.
    pub fn name(&self) -> String {
        self.name.clone()
    }

    /// Return the creator address.
    pub fn creator(&self) -> String {
        self.creator.clone()
    }

    /// Validate a token identifier according to length policy.
    fn is_valid_id(token_id: &TokenId) -> bool {
        token_id.len() > 0 && token_id.len() < 256
    }

    /// Whether the token has an owner and is not burned.
    fn has_been_minted(&self, token_id: &TokenId) -> bool {
        match self.owners.get(token_id) {
            Some(owner) => {
                owner != EMPTY_ADDRESS // EMPTY_ADDRESS -> Burned token
            }
            None => false, // Not owned, non-existent.
        }
    }

    /// Count all NFTs assigned to an address (possibly 0).
    pub fn balance_of(&self, addr: Address) -> usize {
        match self.owned.get(&addr) {
            Some(nfts) => nfts.len(),
            None => 0,
        }
    }

    /// Find the owner of an NFT.
    ///
    /// # Errors
    /// - Invalid token id.
    /// - Owner unknown/missing.
    pub fn owner_of(&self, token_id: TokenId) -> Result<Address> {
        if !Self::is_valid_id(&token_id) {
            return Err(Error::msg(format!(
                "`{}` is not a valid token id",
                token_id
            )));
        }

        match self.owners.get(&token_id) {
            Some(owner) => Ok(owner.clone()),
            None => Err(Error::msg(format!(
                "owner of `{}` is not identified",
                token_id
            ))),
        }
    }

    /// Return the metadata for a token.
    ///
    /// # Errors
    /// - [`DetailsFetchError::InvalidTokenId`]
    /// - [`DetailsFetchError::TokenNotMinted`]
    /// - [`DetailsFetchError::TokenNotFound`]
    pub fn details(&self, token_id: TokenId) -> std::result::Result<Token, DetailsFetchError> {
        if !Self::is_valid_id(&token_id) {
            return Err(DetailsFetchError::InvalidTokenId(token_id));
        }

        if !self.has_been_minted(&token_id) {
            return Err(DetailsFetchError::TokenNotMinted(token_id));
        }

        let Some(token) = self.tokens.get(&token_id) else {
            return Err(DetailsFetchError::TokenNotFound(token_id));
        };

        Ok(token)
    }

    /// Internal helper: perform the state changes for a transfer and clear per-token allowance.
    ///
    /// Updates:
    /// - Calls [`Ledger::transfer`] for supply/transfer accounting,
    /// - Moves ownership,
    /// - Updates `owned` index for both parties,
    /// - Clears `<from>$<token_id>` allowance.
    ///
    /// # Errors
    /// - If the Ledger applet rejects the transfer.
    /// - If the sender’s owned-set is missing (inconsistent state).
    fn do_transfer(&mut self, token_id: String, from_addr: String, to_addr: String) -> Result<()> {
        // Update the Ledger
        let Ok(()) = Ledger::transfer(token_id.clone(), from_addr.clone(), to_addr.clone(), 1)
        else {
            return Err(Error::msg(format!(
                "`{}` could not be transferred by the Ledger",
                token_id
            )));
        };

        // Update the token
        self.owners.insert(token_id.clone(), to_addr.clone());

        // Update the owners
        let Some(mut old_owners_tokens) = self.owned.get(&from_addr) else {
            return Err(Error::msg(format!("owned tokens is missing")));
        };
        old_owners_tokens.remove(&token_id);
        self.owned.insert(from_addr.clone(), old_owners_tokens);

        let new_owners_tokens = match self.owned.get(&to_addr.clone()) {
            Some(mut current_tokens) => {
                current_tokens.insert(token_id.clone());
                current_tokens
            }
            None => BTreeSet::from([token_id.clone()]),
        };
        self.owned.insert(to_addr.clone(), new_owners_tokens);

        // Update old owner's individual allowances
        let key = format!("{}${}", from_addr, token_id.clone());
        self.allowances.remove(&key);

        Ok(())
    }

    /// Transfer a token you own to `to_addr`.
    ///
    /// # Errors
    /// - Invalid token id.
    /// - Token missing or not owned by the sender.
    /// - Ledger transfer failure.
    pub fn transfer(&mut self, to_addr: Address, token_id: TokenId) -> Result<()> {
        let from_addr = Runtime::sender();

        if !Self::is_valid_id(&token_id) {
            return Err(Error::msg(format!(
                "`{}` is not a valid token id",
                &token_id
            )));
        }

        // Check the owner
        let Some(old_owner) = self.owners.get(&token_id) else {
            return Err(Error::msg(format!(
                "token `{}` is missing an owner",
                &token_id
            )));
        };

        if old_owner != from_addr {
            return Err(Error::msg(format!(
                "token `{}` not owned by {}",
                &token_id, &from_addr
            )));
        }

        self.do_transfer(token_id, from_addr, to_addr)
    }

    /// Transfer a token from `from_addr` to `to_addr` using an **allowance**.
    ///
    /// The current sender must be explicitly approved either:
    /// - per-token (`<owner>$<token_id>`), or
    /// - globally for all tokens owned by `from_addr` (`<owner>$""`).
    ///
    /// # Errors
    /// - Invalid token id.
    /// - Token missing or not owned by `from_addr`.
    /// - No matching allowance for the caller.
    /// - Ledger transfer failure.
    pub fn transfer_from(
        &mut self,
        from_addr: Address,
        to_addr: Address,
        token_id: TokenId,
    ) -> Result<()> {
        let spender = Runtime::sender();

        if !Self::is_valid_id(&token_id) {
            return Err(Error::msg(format!(
                "`{}` is not a valid token id",
                &token_id
            )));
        }

        // Check the owner
        let Some(old_owner) = self.owners.get(&token_id) else {
            return Err(Error::msg(format!(
                "token `{}` is missing an owner",
                &token_id
            )));
        };

        if old_owner != from_addr {
            return Err(Error::msg(format!(
                "token `{}` not owned by {}",
                &token_id, &from_addr
            )));
        }

        // Check the allowances.
        let key = format!("{}${}", old_owner, token_id);
        let mut allowed = match self.allowances.get(&key) {
            Some(allowed) => allowed == spender,
            None => false,
        };

        if !allowed {
            let key = format!("{}${}", old_owner, EMPTY_TOKEN_ID);
            allowed = match self.allowances.get(&key) {
                Some(allowed) => allowed == spender,
                None => false,
            };
            if !allowed {
                return Err(Error::msg(format!(
                    "transfer of token `{}` not authorized",
                    &token_id
                )));
            }
        };

        self.do_transfer(token_id, from_addr, to_addr)
    }

    /// Set or clear an **individual** (per-token) allowance.
    ///
    /// If `spender == EMPTY_ADDRESS`, the allowance is removed.
    ///
    /// # Errors
    /// - Invalid token id.
    /// - Caller is not the current owner.
    pub fn approve(&mut self, spender: Address, token_id: TokenId) -> Result<()> {
        let from_addr = Runtime::sender();

        if !Self::is_valid_id(&token_id) {
            return Err(Error::msg(format!(
                "`{}` is not a valid token id",
                &token_id
            )));
        }

        // If owner
        let Some(old_owner) = self.owners.get(&token_id) else {
            return Err(Error::msg(format!(
                "token `{}` is missing an owner",
                token_id
            )));
        };

        if old_owner != from_addr {
            return Err(Error::msg(format!(
                "allowance of token `{}` not authorized",
                token_id
            )));
        }

        let key = format!("{}${}", from_addr, token_id);
        if spender == EMPTY_ADDRESS {
            self.allowances.remove(&key);
        } else {
            self.allowances.insert(key, spender);
        }

        Ok(())
    }

    /// Set or clear a **global** (“approve for all”) allowance for the sender.
    ///
    /// Total allowances may overlap with individual allowances; the per-token
    /// entry remains independent.
    pub fn set_approve_for_all(&mut self, spender: String, approval: bool) {
        let from_addr = Runtime::sender();

        let key = format!("{}${}", from_addr, EMPTY_TOKEN_ID);
        if approval {
            self.allowances.insert(key, spender);
        } else {
            self.allowances.remove(&key);
        }
    }

    /// Return the list of approved spenders for a `token_id`.
    ///
    /// The list can include (at most) two entries:
    /// 1. Per-token approval (if present),
    /// 2. Global “approve for all” (if present).
    ///
    /// # Errors
    /// - Invalid token id.
    /// - Token missing an owner.
    pub fn get_approved(&self, token_id: TokenId) -> Result<Vec<Address>> {
        let mut response: Vec<Address> = vec![];

        if !Self::is_valid_id(&token_id) {
            return Err(Error::msg(format!(
                "`{}` is not a valid token id",
                token_id
            )));
        }

        let Some(owner) = self.owners.get(&token_id) else {
            return Err(Error::msg(format!(
                "token `{}` is missing an owner",
                token_id
            )));
        };

        let key = format!("{}${}", owner, token_id);
        if let Some(allowed) = self.allowances.get(&key) {
            response.push(allowed.clone());
        }

        let key = format!("{}${}", owner, EMPTY_TOKEN_ID);
        if let Some(allowed) = self.allowances.get(&key) {
            response.push(allowed.clone());
        }

        Ok(response)
    }

    /// Check whether `spender` has a global (“approve for all”) allowance from `owner`.
    pub fn is_approved_for_all(&self, owner: String, spender: String) -> bool {
        let key = format!("{}${}", owner, EMPTY_TOKEN_ID);
        let Some(allowed) = self.allowances.get(&key) else {
            return false;
        };

        allowed == spender
    }

    /// Mint a new token to the caller and register it in the collection.
    ///
    /// Steps:
    /// 1. Ensure token has not already been minted,
    /// 2. Call [`Ledger::mint`] for accounting,
    /// 3. Persist token metadata, set owner to caller,
    /// 4. Update the owner’s `owned` set.
    ///
    /// # Errors
    /// - Token already minted.
    /// - Ledger mint failure.
    pub fn mint(&mut self, token_id: TokenId, token: Token) -> Result<()> {
        let from_addr = Runtime::sender();

        if self.has_been_minted(&token_id) {
            return Err(Error::msg(format!(
                "Token `{}` has already been minted",
                token_id
            )));
        }

        let Ok(()) = Ledger::mint(token_id.clone(), from_addr.clone(), 1) else {
            return Err(Error::msg(format!(
                "`{}` could not be transferred by the Ledger",
                token_id
            )));
        };

        self.tokens.insert(token_id.clone(), token);
        self.owners.insert(token_id.clone(), from_addr.clone());

        let new_owners_tokens = match self.owned.get(&from_addr) {
            Some(mut current_tokens) => {
                current_tokens.insert(token_id);
                current_tokens
            }
            None => BTreeSet::from([token_id]),
        };
        self.owned.insert(from_addr.clone(), new_owners_tokens);

        Ok(())
    }
}
