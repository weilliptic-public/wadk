use super::{WeilCollection, WeilId};
use crate::runtime::Memory;
use crate::traits::WeilType;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::borrow::Borrow;
use std::{hash::Hash, marker::PhantomData};

/// A shallow light weight lazy data-structure mimicking `std::collections::HashMap`.
#[derive(Debug, Serialize, Deserialize)]
pub struct WeilMap<K, V> {
    state_id: WeilId,
    phantom: PhantomData<(K, V)>,
}

impl<K, V> WeilMap<K, V> {
    /// Constructs a new empty `WeilMap<K, V>`.
    pub fn new(id: WeilId) -> Self {
        WeilMap {
            state_id: id,
            phantom: PhantomData,
        }
    }
}

impl<K, V> WeilMap<K, V>
where
    K: Serialize + DeserializeOwned + Hash + Eq,
    V: WeilType,
{
    /// Inserts a key-value pair into the map.
    /// If the map did have this key present, the value is updated.
    pub fn insert(&mut self, key: K, val: V) {
        Memory::write_collection(self.state_tree_key(&key), val)
    }

    /// Returns the owned value corresponding to the key.
    /// The method is slightly different from `std::collections::HashMap` `get`, where it returns
    /// the owned element rather than a reference. This makes the returned value a
    /// completely different owned object whose mutations won't reflect in the `WeilMap<K, V>`
    /// value corrosponding to the key.
    /// So if one wants to mutate the returned value and get it reflected in `WeilMap<K, V>`,
    /// call `insert` after the mutation of the returned owned value.
    ///
    /// <br>
    ///
    /// # Example
    ///
    /// ```
    /// let mut map: WeilMap<String, Vec<usize>> = WeilMap::new(WeilId(0));
    ///
    /// map.insert("key1".to_string(), vec![1, 2, 3]);
    /// map.insert("key2".to_string(), vec![10, 3, 4]);
    ///
    /// let mut v = map.get(&"key1".to_string()).unwrap(); // this would be: [1, 2, 3]
    /// v.push(4); // this change won't reflect in the value for the key `key1`
    ///
    /// map.insert("key1".to_string(), v); // now it will set the updated value of `v` inside `map`
    /// ```
    ///
    /// <br>
    ///
    /// # Note
    /// Even though the API returns an owned value, it does not mean that value will not be there
    /// inside the `WeilVec<T>` at the index after calling the method. This is due to how values of
    /// `Weil Collections` in general are persisted on the platform side.
    pub fn get<Q>(&self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + Serialize,
    {
        Memory::read_collection(self.state_tree_key(key))
    }

    /// Removes a key from the map, returning the value at the key if the key was previously in the map.
    pub fn remove<Q>(&self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + Serialize,
    {
        Memory::delete_collection(self.state_tree_key(key))
    }
}

impl<K, V> WeilType for WeilMap<K, V>
where
    K: Serialize + DeserializeOwned,
    V: WeilType,
{
}

impl<'a, K, V, Q: 'a> WeilCollection<'a, Q> for WeilMap<K, V>
where
    K: Serialize + DeserializeOwned,
    V: WeilType,
{
    type Key = K;

    fn base_state_path(&self) -> WeilId {
        self.state_id
    }

    fn state_tree_key(&'a self, suffix: &'a Q) -> String
    where
        Q: Serialize,
        <Self as WeilCollection<'a, Q>>::Key: Borrow<Q>,
    {
        format!(
            "{}_{}",
            <WeilMap<K, V> as WeilCollection<'_, Q>>::base_state_path(self),
            serde_json::to_string(suffix).unwrap()
        )
    }
}

impl<K, V> Clone for WeilMap<K, V> {
    fn clone(&self) -> Self {
        WeilMap {
            state_id: self.state_id,
            phantom: PhantomData,
        }
    }
}
