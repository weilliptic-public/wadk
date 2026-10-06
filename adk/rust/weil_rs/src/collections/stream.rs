//! Append-only, offset-addressed collection backed by the platform's
//! segment log.
//!
//! `WeilStream<V>` is a sibling of [`crate::collections::map::WeilMap`],
//! but a different animal: writes append to a per-`(contract_id, topic)`
//! log at `/mnt/weilliptic/weilstreams/orgs/<contract_id>/<topic>_<marker>-{Data,Index}.db`,
//! reads walk that log in insertion order by global record offset, and
//! retention drops whole segments rather than individual keys.
//!
//! There is no point-key `get` / `remove` -- streams are cursors, not
//! maps. The primitive is [`WeilStream::read_block`], which returns one
//! checksum-verified block starting at the caller's offset; every other
//! read method is a convenience wrapper on top of it.

use super::WeilId;
use crate::runtime::{Memory, StreamBatch, StreamRecord};
use crate::traits::WeilType;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::marker::PhantomData;

/// An append-only log of `V`-typed records, keyed by string.
///
/// Constructed with a `(state_id, topic)` pair. `topic` names the on-disk
/// segment family within this contract's stream directory; use different
/// topics on the same contract to keep unrelated streams from sharing an
/// offset space.
#[derive(Debug, Serialize, Deserialize)]
pub struct WeilStream<V> {
    state_id: WeilId,
    topic: String,
    phantom: PhantomData<V>,
}

impl<V> WeilStream<V> {
    /// Constructs a new empty `WeilStream<V>` writing to `topic` under
    /// this contract's stream directory.
    pub fn new(id: WeilId, topic: String) -> Self {
        Self {
            state_id: id,
            topic,
            phantom: PhantomData,
        }
    }

    /// The topic this stream writes to and reads from.
    pub fn topic(&self) -> &str {
        &self.topic
    }

    fn storage_key(&self, key: &str) -> String {
        stream_storage_key(&self.state_id.to_string(), key)
    }
}

impl<V: WeilType> WeilStream<V> {
    /// Append `(key, value)` to the log. Buffered on the host until the
    /// end of the WASM call, then flushed to the segment writer at the
    /// same commit boundary as the contract's state insert.
    ///
    /// `key` is retained per record (the segment stores full
    /// [`StreamRecord`]s), so downstream readers can pair each value
    /// back with the key that produced it.
    pub fn append(&mut self, key: String, value: V) -> Result<(), String> {
        let storage_key = self.storage_key(&key);
        Memory::write_stream(&self.topic, storage_key, value)
    }
}

impl<V> WeilStream<V> {
    /// Fetch one segment block starting at `offset`. Returns a
    /// [`StreamBatch`] whose `block` field is the raw bytes for every
    /// record in the block from `offset` onward, plus `next_offset`
    /// (== `offset + records_in_block`) to feed into the next call, and
    /// `eof` which goes true once the reader has caught up to the writer.
    ///
    /// The block is length-prefix framed; iterate records with
    /// [`StreamBatch::iter_records`] or use the SDK's own [`Self::iter`]
    /// / [`Self::raw_iter`] to walk records across blocks automatically.
    pub fn read_block(&self, offset: u64) -> StreamBatch {
        let key = self.storage_key(&offset.to_string());
        Memory::read_stream_block(&self.topic, &key, offset).unwrap_or(StreamBatch {
            block: Vec::new(),
            next_offset: offset,
            eof: true,
        })
    }

    /// Whole-record cursor over the stream. Refills its local buffer
    /// with one [`Self::read_block`] call whenever it drains,
    /// terminating once the host reports `eof`.
    pub fn raw_iter(&self) -> RawStreamIter {
        RawStreamIter {
            state_id: self.state_id.to_string(),
            topic: self.topic.clone(),
            offset: 0,
            buffer: VecDeque::new(),
            eof: false,
        }
    }
}

impl<V> WeilStream<V>
where
    V: serde::de::DeserializeOwned,
{
    /// Value-only cursor. Yields each record's payload deserialized as
    /// `V`, dropping the key.
    pub fn iter(&self) -> StreamIter<V> {
        StreamIter {
            inner: self.raw_iter(),
            phantom: PhantomData,
        }
    }
}

impl<V: WeilType> WeilType for WeilStream<V> {}

impl<V> Clone for WeilStream<V> {
    fn clone(&self) -> Self {
        Self {
            state_id: self.state_id,
            topic: self.topic.clone(),
            phantom: PhantomData,
        }
    }
}

/// Whole-record cursor: yields `StreamRecord { key, val }` as the raw
/// on-disk form. Refills one block at a time from the host.
pub struct RawStreamIter {
    state_id: String,
    topic: String,
    offset: u64,
    buffer: VecDeque<StreamRecord>,
    eof: bool,
}

impl RawStreamIter {
    /// Global record offset the iterator will feed into the next
    /// `read_stream_block` call. Exposed so callers can checkpoint
    /// their position and resume later without walking from zero.
    pub fn current_offset(&self) -> u64 {
        self.offset
    }
}

impl Iterator for RawStreamIter {
    type Item = StreamRecord;

    fn next(&mut self) -> Option<StreamRecord> {
        while self.buffer.is_empty() && !self.eof {
            let key = stream_storage_key(&self.state_id, &self.offset.to_string());
            let batch = Memory::read_stream_block(&self.topic, &key, self.offset)?;
            self.eof = batch.eof;
            self.offset = batch.next_offset;
            // Walk the raw block bytes and decode every record locally
            // -- the host handed us framed bytes, not a Vec<StreamRecord>.
            self.buffer.extend(batch.iter_records());
        }
        self.buffer.pop_front()
    }
}

fn stream_storage_key(state_id: &str, key: &str) -> String {
    format!("{}_{}", state_id, serde_json::to_string(key).unwrap())
}

/// Typed cursor: thin wrapper over [`RawStreamIter`] that deserializes
/// each record's value into `V` and drops the key.
pub struct StreamIter<V> {
    inner: RawStreamIter,
    phantom: PhantomData<V>,
}

impl<V> StreamIter<V> {
    /// See [`RawStreamIter::current_offset`].
    pub fn current_offset(&self) -> u64 {
        self.inner.current_offset()
    }
}

impl<V> Iterator for StreamIter<V>
where
    V: serde::de::DeserializeOwned,
{
    type Item = V;

    fn next(&mut self) -> Option<V> {
        let record = self.inner.next()?;
        // A record whose value payload no longer matches V is a schema
        // migration bug -- crash loudly rather than skip silently.
        Some(
            serde_json::from_str(&record.val)
                .expect("WeilStream::iter: failed to deserialize stream record into V"),
        )
    }
}