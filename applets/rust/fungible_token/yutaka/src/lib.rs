
use serde::{Deserialize, Serialize};
use weil_contracts::fungible::FungibleToken;
use weil_macros::{constructor, mutate, query, smart_contract, WeilType};


trait Yutaka {
    fn new() -> Result<Self, String>
    where
        Self: Sized;
    async fn name(&self) -> String;
    async fn symbol(&self) -> String;
    async fn decimals(&self) -> u8;
    async fn details(&self) -> (String, String, u8);
    async fn total_supply(&self) -> u64;
    async fn balance_for(&self, addr: String) -> Result<u64, String>;
    async fn transfer(&mut self, to_addr: String, amount: u64) -> Result<(), String>;
    async fn approve(&mut self, spender: String, amount: u64);
    async fn transfer_from(&mut self, from_addr: String, to_addr: String, amount: u64) -> Result<(), String>;
    async fn allowance(&self, owner: String, spender: String) -> u64;
}

#[derive(Serialize, Deserialize, WeilType)]
pub struct YutakaContractState {
    inner: FungibleToken,
}

#[smart_contract]
impl Yutaka for YutakaContractState {
    #[constructor]
    fn new() -> Result<Self, String>
    where
        Self: Sized,
    {
        let total_supply = 100_000_000_000;
        let mut yutaka_token = YutakaContractState {
            inner: FungibleToken::new("Yutaka".to_string(), "YTK".to_string()),
        };

        yutaka_token.inner.mint(total_supply).map_err(|err| err.to_string())?;

        Ok(yutaka_token)
    }


    #[query]
    async fn name(&self) -> String {
        self.inner.name()
    }

    #[query]
    async fn symbol(&self) -> String {
        self.inner.symbol()
    }

    #[query]
    async fn decimals(&self) -> u8 {
        6
    }

    #[query]
    async fn details(&self) -> (String, String, u8) {
        (self.inner.name(), self.inner.symbol(), self.decimals().await)
    }

    #[query]
    async fn total_supply(&self) -> u64 {
        self.inner.total_supply()
    }

    #[query]
    async fn balance_for(&self, addr: String) -> Result<u64, String> {
        self.inner.balance_for(addr).map_err(|err| err.to_string())
    }

    #[mutate]
    async fn transfer(&mut self, to_addr: String, amount: u64) -> Result<(), String> {
        self.inner
            .transfer(to_addr, amount)
            .map_err(|err| err.to_string())
    }

    #[mutate]
    async fn approve(&mut self, spender: String, amount: u64) {
        self.inner.approve(spender, amount)
    }

    #[mutate]
    async fn transfer_from(&mut self, from_addr: String, to_addr: String, amount: u64) -> Result<(), String> {
        self.inner
            .transfer_from(from_addr, to_addr, amount)
            .map_err(|err| err.to_string())
    }

    #[query]
    async fn allowance(&self, owner: String, spender: String) -> u64 {
        self.inner.allowance(owner, spender)
    }
}
