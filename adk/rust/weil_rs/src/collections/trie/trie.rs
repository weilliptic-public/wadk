use super::map::WeilTriePrefixMap;
use crate::{
    collections::{WeilCollection, WeilId},
    runtime::Memory,
    traits::WeilType,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::marker::PhantomData;

/// A shallow light weight lazy data-structure similar to `WeilMap<String, T>` with extra constraint
/// of key to be of type `String` in favour of supporting prefixed queries.
/// See the methods for more info.
#[derive(Debug, Serialize, Deserialize)]
pub struct WeilTrieMap<T> {
    state_id: WeilId,
    phantom: PhantomData<T>,
}

impl<T> WeilTrieMap<T> {
    /// Constructs a new empty `WeilTrieMap<T>`.
    pub fn new(id: WeilId) -> Self {
        WeilTrieMap {
            state_id: id,
            phantom: PhantomData,
        }
    }
}

impl<T> WeilTrieMap<T>
where
    T: Serialize + DeserializeOwned,
{
    /// Behave similar to `WeilMap<String, T>`
    pub fn insert(&mut self, key: String, val: T) {
        Memory::write_collection(self.state_tree_key(&key), val)
    }

    /// Behave similar to `WeilMap<String, T>`
    pub fn get(&self, key: &str) -> Option<T> {
        Memory::read_collection(self.state_tree_key(key))
    }

    /// Behave similar to `WeilMap<String, T>`
    pub fn remove(&self, key: &str) -> Option<T> {
        Memory::delete_collection(self.state_tree_key(key))
    }

    /// Returns all the values in pair with their corrosponding keys which share the provided prefix.
    pub fn get_with_prefix(&self, prefix: &str) -> Option<WeilTriePrefixMap<T>> {
        Memory::read_prefix_for_trie(self.state_tree_key(prefix))
    }
}

impl<T> WeilType for WeilTrieMap<T> where T: Serialize + DeserializeOwned {}

impl<'a, T> WeilCollection<'a, str> for WeilTrieMap<T>
where
    T: Serialize + DeserializeOwned,
{
    type Key = String;

    fn base_state_path(&self) -> WeilId {
        self.state_id
    }

    fn state_tree_key(&'a self, suffix: &'a str) -> String {
        format!(
            "{}_{}",
            <WeilTrieMap<T> as WeilCollection<'_, str>>::base_state_path(self),
            suffix,
        )
    }
}
