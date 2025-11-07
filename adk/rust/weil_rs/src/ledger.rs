//! # Ledger client façade (FT/NFT-style token APIs)
//!
//! This module exposes a thin, type-safe wrapper over the on-chain **Ledger**
//! contract, providing convenience methods for common token operations:
//!
//! - [`Ledger::balances_for`] — fetch all token balances for an address
//! - [`Ledger::balance_for`] — fetch a single token balance by symbol
//! - [`Ledger::transfer`] — transfer a token amount between addresses
//! - [`Ledger::mint`] — mint new tokens to an address
//!
//! Internally, each helper serializes its call arguments as JSON and invokes the
//! appropriate Ledger method via [`Runtime::call_contract`], resolving the
//! Ledger's contract ID using [`Runtime::ledger_contract_id`].

use crate::{collections::trie::map::WeilTriePrefixMap, runtime::Runtime};
use anyhow::Result;
use serde::Serialize;

/// High-level helper for invoking Ledger contract methods.
///
/// All methods here are synchronous wrappers that:
/// 1. Serialize a small argument struct to JSON,
/// 2. Call the Ledger contract by name,
/// 3. Deserialize and return the typed result (or `()` for side-effect calls).
pub struct Ledger;

impl Ledger {
    /// Return the mapping between token **symbols** and their **balances** for `addr`.
    ///
    /// This is typically used by wallets to list all tokens owned by a user along with
    /// current balances.
    ///
    /// # Arguments
    /// * `addr` — the address whose balances to fetch
    ///
    /// # Returns
    /// A trie-like map (`WeilTriePrefixMap<u64>`) where each key is a token symbol and
    /// the value is the balance as `u64`.
    pub fn balances_for(addr: String) -> Result<WeilTriePrefixMap<u64>> {
        #[derive(Debug, Serialize)]
        struct LedgerBalancesMethodArgs {
            addr: String,
        }

        let serialized_args = serde_json::to_string(&LedgerBalancesMethodArgs { addr }).unwrap();
        let balances = Runtime::call_contract::<WeilTriePrefixMap<u64>>(
            Runtime::ledger_contract_id(),
            "balances_for".to_string(),
            Some(serialized_args),
        )?;

        Ok(balances)
    }

    /// Return the balance of a specific token `symbol` for `addr`.
    ///
    /// # Arguments
    /// * `addr` — the address whose balance to fetch
    /// * `symbol` — token symbol (e.g., `"USDC"`)
    ///
    /// # Returns
    /// Balance as `u64`.
    pub fn balance_for(addr: String, symbol: String) -> Result<u64> {
        #[derive(Debug, Serialize)]
        struct LedgerBalanceMethodArgs {
            addr: String,
            symbol: String,
        }

        let serialized_args =
            serde_json::to_string(&LedgerBalanceMethodArgs { addr, symbol }).unwrap();
        let balances = Runtime::call_contract::<u64>(
            Runtime::ledger_contract_id(),
            "balance_for".to_string(),
            Some(serialized_args),
        )?;

        Ok(balances)
    }

    /// Transfer `amount` of token `symbol` from `from_addr` to `to_addr`.
    ///
    /// This is a side-effecting call and returns `Ok(())` on success.
    ///
    /// # Arguments
    /// * `symbol` — token symbol
    /// * `from_addr` — source address
    /// * `to_addr` — destination address
    /// * `amount` — amount to transfer
    pub fn transfer(symbol: String, from_addr: String, to_addr: String, amount: u64) -> Result<()> {
        #[derive(Debug, Serialize)]
        struct LedgerTransferMethodArgs {
            symbol: String,
            from_addr: String,
            to_addr: String,
            amount: u64,
        }

        let serialized_args = serde_json::to_string(&LedgerTransferMethodArgs {
            symbol,
            from_addr,
            to_addr,
            amount,
        })
        .unwrap();

        Runtime::call_contract::<()>(
            Runtime::ledger_contract_id(),
            "transfer".to_string(),
            Some(serialized_args),
        )?;

        Ok(())
    }

    /// Mint `amount` of token `symbol` to `to_addr`.
    ///
    /// This is a side-effecting administrative call; access control is enforced by the
    /// Ledger contract itself.
    ///
    /// # Arguments
    /// * `symbol` — token symbol
    /// * `to_addr` — recipient address
    /// * `amount` — amount to mint
    pub fn mint(symbol: String, to_addr: String, amount: u64) -> Result<()> {
        #[derive(Debug, Serialize)]
        struct LedgerMintMethodArgs {
            symbol: String,
            to_addr: String,
            amount: u64,
        }

        let serialized_args = serde_json::to_string(&LedgerMintMethodArgs {
            symbol,
            to_addr,
            amount,
        })
        .unwrap();

        Runtime::call_contract::<()>(
            Runtime::ledger_contract_id(),
            "mint".to_string(),
            Some(serialized_args),
        )?;

        Ok(())
    }
}
