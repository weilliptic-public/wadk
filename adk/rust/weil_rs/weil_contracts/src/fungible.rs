//! # Fungible Token (FT) contract (Ledger-backed)
//!
//! Minimal ERC-20–style interface backed by the platform **Ledger** applet for
//! balances/transfers/mints, with an internal allowance map stored as
//! `"<owner>$<spender>" → amount`.
//!
//! ## Exposed capabilities
//! - Read token metadata: [`FungibleToken::name`], [`FungibleToken::symbol`],
//!   [`FungibleToken::total_supply`]
//! - Balance query via Ledger: [`FungibleToken::balance_for`]
//! - Direct transfer: [`FungibleToken::transfer`]
//! - Allowance workflow: [`FungibleToken::approve`], [`FungibleToken::allowance`],
//!   [`FungibleToken::transfer_from`]
//! - Minting (updates `total_supply` and calls Ledger): [`FungibleToken::mint`]

use anyhow::{Error, Result};
use serde::{Deserialize, Serialize};
use weil_macros::WeilType;
use weil_rs::{
    collections::{map::WeilMap, WeilId},
    ledger::Ledger,
    runtime::Runtime,
};

/// Persistent FT state and allowance table.
///
/// Balances are **not** stored here; they are maintained by the Ledger applet.
/// This contract keeps:
//  - token metadata (`name`, `symbol`)
//  - `total_supply` (mirrors mints)
//  - `allowances` as `"<owner>$<spender>" -> amount`
#[derive(Debug, Serialize, Deserialize, WeilType)]
pub struct FungibleToken {
    name: String,
    symbol: String,
    total_supply: u64,
    /// Allowances keyed by `"<owner>$<spender>"`.
    allowances: WeilMap<String, u64>,
}

impl FungibleToken {
    /// Create a new fungible token with zero `total_supply`.
    pub fn new(name: String, symbol: String) -> Self {
        FungibleToken {
            name,
            symbol,
            total_supply: 0,
            allowances: WeilMap::new(WeilId(0)),
        }
    }

    /// Return the token's human-readable name.
    pub fn name(&self) -> String {
        self.name.clone()
    }

    /// Return the token's ticker/symbol (e.g., `"USDC"`).
    pub fn symbol(&self) -> String {
        self.symbol.clone()
    }

    /// Return the current total supply tracked by this contract.
    ///
    /// Note: This value is updated by [`Self::mint`]; transfers do not change it.
    pub fn total_supply(&self) -> u64 {
        self.total_supply
    }

    /// Return the balance for `addr` by delegating to the Ledger applet.
    pub fn balance_for(&self, addr: String) -> Result<u64> {
        Ledger::balance_for(addr, self.symbol())
    }

    /// Transfer `amount` tokens from the **caller** to `to_addr`.
    ///
    /// Delegates to [`Ledger::transfer`].
    pub fn transfer(&mut self, to_addr: String, amount: u64) -> Result<()> {
        Ledger::transfer(self.symbol(), Runtime::sender(), to_addr, amount)
    }

    /// Set/overwrite allowance for `spender` to `amount` for the **caller**.
    ///
    /// Stored locally in `allowances` as `"<owner>$<spender>" → amount`.
    pub fn approve(&mut self, spender: String, amount: u64) {
        let key = format!("{}${}", Runtime::sender(), spender);
        self.allowances.insert(key, amount);
    }

    /// Mint `amount` new tokens to the **caller**.
    ///
    /// Increments `total_supply` and calls [`Ledger::mint`] for accounting.
    pub fn mint(&mut self, amount: u64) -> Result<()> {
        self.total_supply += amount;

        Ledger::mint(self.symbol(), Runtime::sender(), amount)
    }

    /// Transfer `amount` from `from_addr` to `to_addr` using the caller’s allowance.
    ///
    /// Checks `allowances["<from_addr>$<caller>"] >= amount`, then calls
    /// [`Ledger::transfer`], and finally debits the allowance.
    ///
    /// # Errors
    /// - If allowance is missing or insufficient.
    /// - If the Ledger transfer fails.
    pub fn transfer_from(&mut self, from_addr: String, to_addr: String, amount: u64) -> Result<()> {
        let key = format!("{}${}", from_addr, Runtime::sender());
        let balance = match self.allowances.get(&key) {
            Some(balance) => balance,
            None => 0,
        };

        if balance < amount {
            return Err(Error::msg(format!(
                "allowance balance of sender `{}` is `{}`, which is less than amount transfer request from `{}`",
                Runtime::sender(),
                balance,
                from_addr
            )));
        };

        Ledger::transfer(self.symbol(), from_addr, to_addr, amount)?;

        self.allowances.insert(key, balance - amount);

        Ok(())
    }

    /// Read the current allowance set by `owner` for `spender`.
    ///
    /// Returns `0` if no entry exists.
    pub fn allowance(&self, owner: String, spender: String) -> u64 {
        let key = format!("{}${}", owner, spender);
        match self.allowances.get(&key) {
            Some(allowance) => allowance,
            None => 0,
        }
    }
}
