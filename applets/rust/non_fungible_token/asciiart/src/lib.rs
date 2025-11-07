use serde::{Deserialize, Serialize};
use weil_contracts::non_fungible::{NonFungibleToken, Token};
use weil_macros::{constructor, mutate, query, smart_contract, WeilType};
use weil_rs::{
    collections::{map::WeilMap, WeilId},
    runtime::Runtime,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenDetails {
    title: String,
    name: String,
    description: String,
    payload: String,
}

trait AsciiArt {
    fn new() -> Result<Self, String>
    where
        Self: Sized;
    async fn name(&self) -> String;
    async fn balance_of(&self, addr: String) -> u32;
    async fn owner_of(&self, token_id: String) -> Result<String, String>;
    async fn details(&self, token_id: String) -> Result<TokenDetails, String>;
    async fn approve(&mut self, spender: String, token_id: String) -> Result<(), String>;
    async fn set_approve_for_all(&mut self, spender: String, approval: bool);
    async fn transfer(&mut self, to_addr: String, token_id: String) -> Result<(), String>;
    async fn transfer_from(
        &mut self,
        from_addr: String,
        to_addr: String,
        token_id: String,
    ) -> Result<(), String>;
    async fn get_approved(&self, token_id: String) -> Result<Vec<String>, String>;
    async fn is_approved_for_all(&self, owner: String, spender: String) -> bool;
    async fn mint(
        &mut self,
        token_id: String,
        title: String,
        name: String,
        description: String,
        payload: String,
    ) -> Result<(), String>;
}

#[derive(Serialize, Deserialize, WeilType)]
pub struct AsciiArtContractState {
    /// Controllers allowed to mint new tokens.
    /// TODO: How is the set updated?
    controllers: WeilMap<String, ()>,
    inner: NonFungibleToken,
}

impl AsciiArtContractState {
    fn is_controller(&self, addr: &String) -> bool {
        match self.controllers.get(addr) {
            Some(_) => true,
            None => false,
        }
    }
}

#[smart_contract]
impl AsciiArt for AsciiArtContractState {
    #[constructor]
    fn new() -> Result<Self, String>
    where
        Self: Sized,
    {
        let creator: String = Runtime::sender();
        let mut controllers = WeilMap::new(WeilId(0));
        controllers.insert(creator.clone(), ());

        let mut token = AsciiArtContractState {
            controllers,
            inner: NonFungibleToken::new("AsciiArt".to_string()),
        };

        // This contract mints some tokens at the start, but others might mint later.
        let initial_tokens = vec![
            (
                "0",
                Token::new(
                    "A fish going left!".to_string(),
                    "fish 1".to_string(),
                    "A one line ASCII drawing of a fish".to_string(),
                    "<><".to_string(),
                ),
            ),
            (
                "1",
                Token::new(
                    "A fish going right!".to_string(),
                    "fish 2".to_string(),
                    "A one line ASCII drawing of a fish swimming to the right".to_string(),
                    "><>".to_string(),
                ),
            ),
            (
                "2",
                Token::new(
                    "A big fish going left!".to_string(),
                    "fish 3".to_string(),
                    "A one line ASCII drawing of a fish swimming to the left".to_string(),
                    "<'))><".to_string(),
                ),
            ),
            (
                "3",
                Token::new(
                    "A big fish going right!".to_string(),
                    "fish 4".to_string(),
                    "A one line ASCII drawing of a fish swimming to the right".to_string(),
                    "><(('>".to_string(),
                ),
            ),
            (
                "4",
                Token::new(
                    "A Face".to_string(),
                    "face 1".to_string(),
                    "A one line ASCII drawing of a face".to_string(),
                    "(-_-)".to_string(),
                ),
            ),
            (
                "5",
                Token::new(
                    "Arms raised".to_string(),
                    "arms 1".to_string(),
                    "A one line ASCII drawing of a person with arms raised".to_string(),
                    "\\o/".to_string(),
                ),
            ),
        ];

        for (i, t) in initial_tokens {
            token
                .inner
                .mint(i.to_string(), t)
                .map_err(|err| err.to_string())?;
        }

        Ok(token)
    }

    #[query]
    async fn name(&self) -> String {
        self.inner.name()
    }

    #[query]
    async fn balance_of(&self, addr: String) -> u32 {
        self.inner.balance_of(addr) as u32
    }

    #[query]
    async fn owner_of(&self, token_id: String) -> Result<String, String> {
        self.inner.owner_of(token_id).map_err(|err| err.to_string())
    }

    #[query]
    async fn details(&self, token_id: String) -> Result<TokenDetails, String> {
        let token = self
            .inner
            .details(token_id)
            .map_err(|err| err.to_string())?;

        Ok(TokenDetails {
            name: token.name,
            title: token.title,
            description: token.description,
            payload: token.payload,
        })
    }

    #[mutate]
    async fn approve(&mut self, spender: String, token_id: String) -> Result<(), String> {
        self.inner
            .approve(spender, token_id)
            .map_err(|err| err.to_string())
    }

    #[mutate]
    async fn set_approve_for_all(&mut self, spender: String, approval: bool) {
        self.inner.set_approve_for_all(spender, approval)
    }

    #[mutate]
    async fn transfer(&mut self, to_addr: String, token_id: String) -> Result<(), String> {
        self.inner
            .transfer(to_addr, token_id)
            .map_err(|err| err.to_string())
    }

    #[mutate]
    async fn transfer_from(
        &mut self,
        from_addr: String,
        to_addr: String,
        token_id: String,
    ) -> Result<(), String> {
        self.inner
            .transfer_from(from_addr, to_addr, token_id)
            .map_err(|err| err.to_string())
    }

    #[query]
    async fn get_approved(&self, token_id: String) -> Result<Vec<String>, String> {
        self.inner
            .get_approved(token_id)
            .map_err(|err| err.to_string())
    }

    #[query]
    async fn is_approved_for_all(&self, owner: String, spender: String) -> bool {
        self.inner.is_approved_for_all(owner, spender)
    }

    #[mutate]
    async fn mint(
        &mut self,
        token_id: String,
        title: String,
        name: String,
        description: String,
        payload: String,
    ) -> Result<(), String> {
        let token = Token::new(title, name, description, payload);

        let from_addr = Runtime::sender();

        if !self.is_controller(&from_addr) {
            return Err(format!("Only controllers can mint"));
        }

        self.inner
            .mint(token_id, token)
            .map_err(|err| err.to_string())
    }
}
