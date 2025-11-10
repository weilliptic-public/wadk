use serde::{
    de::{DeserializeOwned, Visitor},
    Deserialize, Serialize,
};
use std::marker::PhantomData;

/// A pair which is returned from the `get` method of `WeilTriePrefixMap<T>`.
#[derive(Debug, Serialize, Deserialize)]
pub struct WeilTriePair<T> {
    key: String,
    value: T,
}

impl<T> WeilTriePair<T> {
    fn new(key: String, value: T) -> Self {
        WeilTriePair { key, value }
    }

    /// Returns `key` of the pair.
    pub fn key(&self) -> &str {
        &self.key
    }

    /// Returns `value` of the pair.
    pub fn value(&self) -> &T {
        &self.value
    }
}

/// A iteratable collection of key-value pair returned from the `get_with_prefix` method of `WeilTrieMap<T>`.
/// This means that all the keys present inside the collection share the same prefix.
#[derive(Debug, Serialize)]
pub struct WeilTriePrefixMap<T>(Vec<WeilTriePair<T>>);

impl<T> WeilTriePrefixMap<T> {
    fn new() -> Self {
        WeilTriePrefixMap(Vec::new())
    }

    /// Returns the number of key-value pairs.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns the key-value pair at the index, or None if out of bounds
    pub fn get(&self, index: usize) -> Option<&WeilTriePair<T>> {
        if index >= self.len() {
            return None;
        }

        Some(&self.0[index])
    }
}

struct WeilTriePrefixMapVisitor<T>(PhantomData<T>);

// Below `Deserialize` trait is implemented to keep the format of the serialized sequence received from host simple: Vec<(String, String)>
// and we can deserialize it using below implementation to form the `WeilTriePrefixMap<T>`!
impl<'de, T: DeserializeOwned> Visitor<'de> for WeilTriePrefixMapVisitor<T> {
    type Value = WeilTriePrefixMap<T>;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("struct WeilTriePrefixMap<T>")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::SeqAccess<'de>,
    {
        let mut map_seq: WeilTriePrefixMap<T> = WeilTriePrefixMap::new();
        while let Some((key, value)) = seq.next_element::<(String, String)>()? {
            let pair = WeilTriePair::new(key, serde_json::from_str::<T>(&value).unwrap());
            map_seq.0.push(pair);
        }

        return Ok(map_seq);
    }
}

impl<'de, T: DeserializeOwned> Deserialize<'de> for WeilTriePrefixMap<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_seq(WeilTriePrefixMapVisitor::<T>(PhantomData))
    }
}

impl<'a, T> IntoIterator for &'a WeilTriePrefixMap<T> {
    type Item = &'a WeilTriePair<T>;
    type IntoIter = std::slice::Iter<'a, WeilTriePair<T>>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}
