//! Consumer client — the Kafka-shaped half of the SDK.
//!
//! ## Two cursors, as in Kafka
//!
//! * **position** — local and volatile: the next sequence a poll will read. Advances the
//!   moment [`WeilStreamConsumer::poll`] hands records back, exactly like Kafka's
//!   `KafkaConsumer::position`. It is *not* a record of what the caller processed.
//! * **committed** — durable on WeilChain: the highest sequence acknowledged. Survives the
//!   client and is what a restart resumes from.
//!
//! Because position advances on delivery rather than on processing, committing is what
//! turns delivery into acknowledgement. Calling [`WeilStreamConsumer::commit`] *after* the
//! caller has finished with a batch gives at-least-once. Calling it before — or never
//! processing what a poll returned — gives at-most-once, and that is the caller's choice
//! to make, the same way it is in Kafka.
//!
//! ## Offsets are off by one against checkpoints
//!
//! A checkpoint stores the highest sequence *acknowledged*; a position is the next
//! sequence to *read*. A client that has consumed `0..=2` sits at position `3` and commits
//! sequence `2`. The conversion happens in one place each way: [`Self::commit`] writes
//! `position - 1`, and a fresh client resumes at `checkpoint + 1`.
//!
//! ## Prefetch: hide the next `read_block` behind the current buffer's tail
//!
//! Once the local buffer drops below [`PREFETCH_WATERMARK`] of the last block's size, the
//! consumer spawns the *next* `read_block` in the background so it overlaps with the
//! caller draining what's still cached. When the buffer runs out, the refill awaits the
//! spawned task -- if it already finished (the common case, since the network fetch runs
//! in parallel with cheap in-process polls), the refill is effectively free. If it hasn't
//! finished yet, the await pays the same latency the caller would have paid synchronously,
//! so prefetch never makes things worse.

use crate::proxy::{Checkpoint, CommitReceipt, S3Credentials, WeilStreamConsumerClient};
use rustc_hash::FxHashMap;
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::mpsc::{Receiver, Sender, channel};
use tokio::task::JoinHandle;
use weil_wallet::{contract::ContractId, wallet::Wallet};
use weilstream_core::{
    ERR_OFFSET_OUT_OF_RANGE, ERR_UNKNOWN_TOPIC, StreamBatch, StreamRecord, low_water_from_error,
};

pub const MAX_CHANNEL_CAPACITY: u32 = 1000;

/// When the local buffer drops to this fraction of the last block's size,
/// kick off the next `read_block` in the background so it overlaps with
/// the tail of the current buffer's local drain. 0.20 = 80% consumed.
const PREFETCH_WATERMARK: f32 = 0.20;

/// What to do when a consumer's position is not servable — Kafka's `auto.offset.reset`.
///
/// This applies in the same two situations Kafka applies it: the position has fallen below
/// the topic's low-water mark because retention passed it, and there is no committed
/// checkpoint to resume from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OffsetReset {
    /// Resume at the lowest sequence still retained. Replays whatever survived, and is the
    /// right choice when reprocessing is cheaper than a gap.
    Earliest,
    /// Jump to the head and take only what arrives next. Skips the gap; correct when stale
    /// records have no value.
    Latest,
    /// Refuse, and surface the error to the caller. Correct when a gap is a business
    /// incident that a human should see rather than something to paper over.
    None,
}

impl Default for OffsetReset {
    /// `Earliest`, which is **not** Kafka's default of `latest`.
    ///
    /// A WeilStream topic is a certified history that a consumer is usually expected to
    /// process in full, and every driver in this workspace publishes and then reads — under
    /// `Latest` they would silently read nothing, which is the single most confusing
    /// failure Kafka's default produces. Set it explicitly with
    /// [`WeilStreamConsumer::with_offset_reset`] where that reasoning does not hold.
    fn default() -> Self {
        OffsetReset::Earliest
    }
}

/// True when an applet error carries `marker`.
///
/// The markers are `const`s in `weilstream_core`, shared with the applet, so this is not
/// matching on prose that either side is free to reword.
fn is(err: &anyhow::Error, marker: &str) -> bool {
    err.to_string().contains(marker)
}

/// A pending background fetch. `offset` is the `from_seq` the task was
/// spawned with -- if a `seek`/`reset_offset` moves the consumer past it
/// before the task lands, the result is stale and gets dropped.
struct Prefetch {
    offset: u64,
    handle: JoinHandle<Result<StreamBatch, anyhow::Error>>,
}

pub struct WeilStreamConsumer {
    processed_offsets: FxHashMap<String, Option<u64>>,
    /// Per-topic block cache: whatever records the last `read_block` call
    /// returned, minus what `poll` has already handed out. `poll` drains
    /// this before issuing another network round trip, so at steady
    /// state a caller sees one host call per segment block instead of
    /// per poll.
    ///
    /// Also tracks each topic's next offset to feed into `read_block`
    /// when the buffer runs dry -- that's what the block reader carries
    /// forward instead of a per-record cursor.
    buffered_records: FxHashMap<String, VecDeque<StreamRecord>>,
    next_read_offset: FxHashMap<String, u64>,
    /// Record count of the block that filled the current buffer. Lets
    /// `poll_one` compute the 80%-drained watermark against a real block
    /// size rather than a hard-coded threshold.
    last_block_size: FxHashMap<String, usize>,
    /// Background fetch for the next block, spawned once the current
    /// buffer crosses `PREFETCH_WATERMARK`. Consumed on the next refill.
    prefetch: FxHashMap<String, Prefetch>,
    topics: Vec<String>,
    /// `Arc` so the client can be cloned into a spawned prefetch task
    /// without moving it out of `self` (which the sync `poll` would
    /// otherwise reject).
    weil_client: Arc<WeilStreamConsumerClient>,
    /// Ceiling on how many records `poll` hands back per invocation.
    /// Kept even under the block-buffered path so callers that size
    /// downstream work on a bounded batch keep the same guarantee.
    max_count: u32,
    offset_reset: OffsetReset,
}

impl WeilStreamConsumer {
    pub fn new(
        applet_id: ContractId,
        max_count: u32,
        wallet: Wallet,
    ) -> Result<Self, anyhow::Error> {
        Ok(WeilStreamConsumer {
            processed_offsets: FxHashMap::default(),
            buffered_records: FxHashMap::default(),
            next_read_offset: FxHashMap::default(),
            last_block_size: FxHashMap::default(),
            prefetch: FxHashMap::default(),
            topics: vec![],
            weil_client: Arc::new(WeilStreamConsumerClient::new(applet_id, wallet)?),
            max_count,
            offset_reset: OffsetReset::default(),
        })
    }

    pub async fn with_api_key(
        applet_id: ContractId,
        max_count: u32,
        api_key: String,
        creds: Option<S3Credentials>,
    ) -> Result<Self, anyhow::Error> {
        Ok(WeilStreamConsumer {
            processed_offsets: FxHashMap::default(),
            buffered_records: FxHashMap::default(),
            next_read_offset: FxHashMap::default(),
            last_block_size: FxHashMap::default(),
            prefetch: FxHashMap::default(),
            topics: vec![],
            weil_client: Arc::new(
                WeilStreamConsumerClient::with_api_key(applet_id, api_key, creds).await?,
            ),
            max_count,
            offset_reset: OffsetReset::default(),
        })
    }

    /// Override the reset policy. See [`OffsetReset`].
    pub fn with_offset_reset(mut self, offset_reset: OffsetReset) -> Self {
        self.offset_reset = offset_reset;
        self
    }

    pub fn offset_reset(&self) -> OffsetReset {
        self.offset_reset
    }

    /// Subscribe to `topics`.
    ///
    /// Idempotent per topic. Subscribing to something already subscribed neither duplicates
    /// it in the poll set — which would read it twice per `poll` and drop the first batch
    /// when the second overwrote it in the result map — nor resets a position the client
    /// has already established.
    pub fn subscribe(&mut self, topics: &[String]) {
        for topic in topics {
            if self.topics.iter().any(|known| known == topic) {
                continue;
            }

            self.processed_offsets.insert(topic.clone(), None);
            self.buffered_records.insert(topic.clone(), VecDeque::new());
            self.topics.push(topic.clone());
        }
    }

    pub fn subscriptions(&self) -> &[String] {
        &self.topics
    }

    /// Move this client's local position, as Kafka's `seek` does.
    ///
    /// Purely local: it does not touch the durable checkpoint, so the next `commit` writes
    /// from wherever the client ends up. This is the replay primitive — the checkpoint
    /// itself may never move backwards.
    ///
    /// A `seek` invalidates any block already cached from a different
    /// starting offset -- otherwise the next `poll` would hand back
    /// pre-seek records first and only then hit the new position. Any
    /// pending prefetch is aborted for the same reason.
    pub fn seek(&mut self, topic: &str, offset: u64) {
        self.processed_offsets
            .insert(topic.to_string(), Some(offset));
        self.buffered_records
            .entry(topic.to_string())
            .or_insert_with(VecDeque::new)
            .clear();
        self.next_read_offset.remove(topic);
        self.last_block_size.remove(topic);
        if let Some(pending) = self.prefetch.remove(topic) {
            pending.handle.abort();
        }
    }

    async fn read_current_checkpoint(
        topic: &str,
        weil_client: &WeilStreamConsumerClient,
    ) -> Result<Option<Checkpoint>, anyhow::Error> {
        weil_client.checkpoint(topic).await
    }

    /// The durable checkpoint for `topic`, or `None` if it has never committed.
    ///
    /// Kafka's `committed()`. Always a round trip — unlike [`Self::get_curr_offset`] this
    /// never reports a local cache.
    ///
    /// **One call is not proof.** Reads are eventually consistent and successive queries
    /// may be answered by different replicas, so this can return a value older than a
    /// commit that has already succeeded — and a later call can return an *older* value
    /// than an earlier one. Acceptance testing against this observed a wait succeed at
    /// checkpoint 5, the next query return 3, and the one after return 5 again. Poll until
    /// the value settles rather than treating a single read as the answer.
    ///
    /// This is why a restart can replay more than the commit interval suggests: a client
    /// that commits and immediately restarts may resume from a stale checkpoint. That is
    /// within the at-least-once contract, not a defect, but size a replay budget for it.
    pub async fn committed(&self, topic: &str) -> Result<Option<u64>, anyhow::Error> {
        Ok(Self::read_current_checkpoint(topic, &self.weil_client)
            .await?
            .map(|checkpoint| checkpoint.sequence))
    }

    /// The topic's beginning offset — Kafka's `beginningOffsets`. Always a round trip.
    pub async fn beginning_offset(&self, topic: &str) -> Result<u64, anyhow::Error> {
        self.weil_client.beginning_offset(topic).await
    }

    /// The topic's end offset — Kafka's `endOffsets`. Always a round trip.
    pub async fn end_offset(&self, topic: &str) -> Result<u64, anyhow::Error> {
        self.weil_client.end_offset(topic).await
    }

    /// How many records this client has yet to read: `end_offset - position`.
    ///
    /// Zero means caught up. This is the one number that separates a healthy idle consumer
    /// from a stalled one, which a count of delivered records cannot do.
    pub async fn lag(&mut self, topic: &str) -> Result<u64, anyhow::Error> {
        let end = self.end_offset(topic).await?;
        let position = self.get_curr_offset(topic).await?;

        Ok(end.saturating_sub(position))
    }

    /// This client's local position for `topic` — the next sequence a poll will read.
    ///
    /// Resolves from the durable checkpoint on first use and is cached thereafter, so on a
    /// client that has already polled this is a local read and not a round trip.
    pub async fn get_curr_offset(&mut self, topic: &str) -> Result<u64, anyhow::Error> {
        Self::offset_for_topic(
            topic,
            &mut self.processed_offsets,
            &self.weil_client,
            self.offset_reset,
        )
        .await
    }

    async fn offset_for_topic(
        topic: &str,
        processed_offsets: &mut FxHashMap<String, Option<u64>>,
        weil_client: &WeilStreamConsumerClient,
        offset_reset: OffsetReset,
    ) -> Result<u64, anyhow::Error> {
        if let Some(offset) = processed_offsets.get(topic).copied().flatten() {
            return Ok(offset);
        }

        // A checkpoint that has never been committed reads back as `None`, and that is the
        // normal state of a new consumer rather than a missing topic: a checkpoint comes
        // into existence by being committed. Where to start from in that case is exactly
        // what `auto.offset.reset` governs.
        let curr_offset = match Self::read_current_checkpoint(topic, weil_client).await? {
            // `sequence` is the highest sequence *acknowledged*, so reading resumes after
            // it.
            Some(checkpoint) => checkpoint.sequence + 1,
            None => match offset_reset {
                OffsetReset::Earliest => 0,
                // The end offset, resolved now: a sentinel like `u64::MAX` would read
                // empty forever, because a position only advances off a delivered record.
                OffsetReset::Latest => weil_client.end_offset(topic).await.unwrap_or(0),
                OffsetReset::None => {
                    anyhow::bail!(
                        "no committed checkpoint for topic `{}` and offset_reset is `None`",
                        topic
                    )
                }
            },
        };

        processed_offsets.insert(topic.to_string(), Some(curr_offset));

        Ok(curr_offset)
    }

    /// Read one batch per subscribed topic, or just `topic` when one is named.
    ///
    /// Every subscribed topic gets an entry in the returned map, empty ones included, so a
    /// caller can iterate subscriptions rather than probe for keys.
    ///
    /// A topic nothing has been published to yet polls **empty rather than erroring**, the
    /// way Kafka treats `UNKNOWN_TOPIC_OR_PARTITION` as retriable: a topic here is created
    /// by its first publish, so a subscriber that starts before the producer is a normal
    /// race and not a failure.
    pub async fn poll(
        &mut self,
        topic: Option<&str>,
    ) -> Result<FxHashMap<String, Vec<StreamRecord>>, anyhow::Error> {
        let topics: Vec<String> = match topic {
            Some(topic) => vec![topic.to_string()],
            None => self.topics.clone(),
        };

        let mut result = FxHashMap::default();

        for topic in topics {
            let records = self.poll_one(&topic).await?;
            result.insert(topic, records);
        }

        Ok(result)
    }

    async fn poll_one(&mut self, topic: &str) -> Result<Vec<StreamRecord>, anyhow::Error> {
        let curr_offset = Self::offset_for_topic(
            topic,
            &mut self.processed_offsets,
            &self.weil_client,
            self.offset_reset,
        )
        .await?;

        // The buffer's stored records were fetched at some prior offset;
        // if the current position has moved past what's queued (e.g. via
        // `seek`, or after an offset reset), discard the stale entries
        // rather than hand them out.
        Self::drop_stale_buffer(topic, curr_offset, &mut self.buffered_records);

        let mut out = Vec::with_capacity(self.max_count as usize);
        let mut reached_eof = false;

        loop {
            if let Some(queue) = self.buffered_records.get_mut(topic) {
                while out.len() < self.max_count as usize {
                    match queue.pop_front() {
                        Some(record) => out.push(record),
                        None => break,
                    }
                }
            }

            if out.len() >= self.max_count as usize {
                break;
            }
            if reached_eof {
                break;
            }

            let read_from = self
                .next_read_offset
                .get(topic)
                .copied()
                .unwrap_or_else(|| {
                    out.last()
                        .map(|record| record.seq + 1)
                        .unwrap_or(curr_offset)
                });

            let batch = self.consume_prefetch_or_fetch(topic, read_from).await?;
            let (batch, unknown_topic) = match batch {
                Ok(b) => (Some(b), false),
                Err(is_unknown) => (None, is_unknown),
            };
            // `UNKNOWN_TOPIC` is retriable in the Kafka sense -- the topic
            // has not been published to yet. Hand back whatever was already
            // drained, usually an empty batch, so the caller can loop.
            if unknown_topic {
                break;
            }
            let batch = batch.expect("either a batch or an unknown-topic short-circuit");
            reached_eof = batch.eof;
            let batch_advanced = batch.next_offset > read_from;
            let min_seq = out
                .last()
                .map(|record| record.seq + 1)
                .unwrap_or(curr_offset);
            let mut appended = 0usize;

            let queue = self
                .buffered_records
                .entry(topic.to_string())
                .or_insert_with(VecDeque::new);
            for record in batch.records {
                // A block can legitimately overlap the caller's position
                // when `read_block` fell on a segment boundary; skip
                // anything before the next sequence this poll still needs.
                if record.seq >= min_seq {
                    queue.push_back(record);
                    appended += 1;
                }
            }
            self.next_read_offset
                .insert(topic.to_string(), batch.next_offset);
            // Track what actually made it into the buffer (post-filter),
            // not the raw batch size -- otherwise the prefetch watermark
            // could sit above the buffer's real length forever whenever
            // the block spans the current position.
            self.last_block_size.insert(topic.to_string(), appended);

            if appended == 0 && !batch_advanced {
                break;
            }
        }

        // Advance from the last sequence actually delivered, not by the
        // batch size. The two agree only while the batch starts at
        // `curr_offset`; anchoring on the record makes the cursor
        // correct even if it ever does not.
        if let Some(last) = out.last() {
            self.processed_offsets
                .insert(topic.to_string(), Some(last.seq + 1));
        }

        // If the buffer has dropped past the prefetch watermark and we
        // are not already fetching, spawn the next `read_block` in the
        // background. The task overlaps with subsequent local-only polls,
        // so when the buffer runs out the refill above can consume the
        // result instead of paying for a fresh round trip.
        self.maybe_spawn_prefetch(topic);

        Ok(out)
    }

    /// Take the standing prefetch's result if there is one and its offset
    /// still matches; otherwise fetch inline. Returns
    /// `Ok(Err(true))` when the fetch resolved to `ERR_UNKNOWN_TOPIC`, so
    /// the caller can short-circuit to an empty batch.
    async fn consume_prefetch_or_fetch(
        &mut self,
        topic: &str,
        read_from: u64,
    ) -> Result<Result<StreamBatch, bool>, anyhow::Error> {
        if let Some(pending) = self.prefetch.remove(topic) {
            if pending.offset == read_from {
                match pending.handle.await {
                    Ok(Ok(batch)) => return Ok(Ok(batch)),
                    Ok(Err(err)) => return self.handle_fetch_error(topic, err).await,
                    Err(join_err) => {
                        return Err(anyhow::anyhow!(
                            "prefetch task for topic `{}` panicked: {}",
                            topic,
                            join_err
                        ));
                    }
                }
            }
            pending.handle.abort();
        }

        match self.weil_client.read_block(topic, read_from).await {
            Ok(batch) => Ok(Ok(batch)),
            Err(err) => self.handle_fetch_error(topic, err).await,
        }
    }

    /// Recover from a `read_block` error. Returns `Ok(Ok(batch))` when
    /// recovery produced a usable batch, `Ok(Err(true))` for the
    /// unknown-topic short-circuit, or propagates the error otherwise.
    async fn handle_fetch_error(
        &mut self,
        topic: &str,
        err: anyhow::Error,
    ) -> Result<Result<StreamBatch, bool>, anyhow::Error> {
        if is(&err, ERR_UNKNOWN_TOPIC) {
            return Ok(Err(true));
        }
        if is(&err, ERR_OFFSET_OUT_OF_RANGE) {
            // Retention has passed this client's position. The applet
            // refuses rather than skipping the gap, so the recovery
            // choice lands here. `reset_offset` also clears the local
            // buffer + any prefetch that was pointed at the stale
            // position.
            let recovered = self.reset_offset(topic, err).await?;
            self.next_read_offset.remove(topic);
            return Ok(Ok(self.weil_client.read_block(topic, recovered).await?));
        }
        Err(err)
    }

    /// Spawn a background `read_block` for the next block when the
    /// current buffer is at least 80% drained and no other prefetch is
    /// already in flight. No-op if the last fetch returned an empty
    /// block (nothing left to prefetch until new writes arrive).
    fn maybe_spawn_prefetch(&mut self, topic: &str) {
        if self.prefetch.contains_key(topic) {
            return;
        }
        let last_size = match self.last_block_size.get(topic).copied() {
            Some(size) if size > 0 => size,
            _ => return,
        };
        let remaining = Self::topic_buffer_len(topic, &self.buffered_records);
        let threshold = ((last_size as f32) * PREFETCH_WATERMARK).ceil() as usize;
        if remaining > threshold {
            return;
        }
        let Some(offset) = self.next_read_offset.get(topic).copied() else {
            return;
        };

        let client = Arc::clone(&self.weil_client);
        let topic_owned = topic.to_string();
        let handle = tokio::spawn(async move { client.read_block(&topic_owned, offset).await });
        self.prefetch
            .insert(topic.to_string(), Prefetch { offset, handle });
    }

    /// Records left in the local buffer for `topic`, or 0 when no
    /// buffer exists yet.
    fn topic_buffer_len(
        topic: &str,
        buffered_records: &FxHashMap<String, VecDeque<StreamRecord>>,
    ) -> usize {
        buffered_records
            .get(topic)
            .map(|queue| queue.len())
            .unwrap_or(0)
    }

    /// Drop any buffered records for `topic` whose `seq` is below the
    /// caller's current position. Fires after `seek` moves the cursor
    /// past what was already fetched, and after an `OFFSET_OUT_OF_RANGE`
    /// recovery jumps forward.
    fn drop_stale_buffer(
        topic: &str,
        curr_offset: u64,
        buffered_records: &mut FxHashMap<String, VecDeque<StreamRecord>>,
    ) {
        if let Some(queue) = buffered_records.get_mut(topic) {
            while let Some(front) = queue.front() {
                if front.seq < curr_offset {
                    queue.pop_front();
                } else {
                    break;
                }
            }
        }
    }

    /// Apply [`OffsetReset`] after the applet reported an out-of-range position, and return
    /// the position to retry from.
    async fn reset_offset(
        &mut self,
        topic: &str,
        err: anyhow::Error,
    ) -> Result<u64, anyhow::Error> {
        let recovered = match self.offset_reset {
            // The topic's *current* beginning, not 0. The applet refuses a read below the
            // low-water mark rather than clamping to it, so retrying from 0 would fail
            // again with the same error — which is exactly what it did before this asked.
            //
            // Prefer the mark the refusing node reported over asking for it again. Both
            // `read_batch` and `beginning_offset` are queries answered by whichever node
            // takes them, so a second call can land on a replica still holding the
            // pre-trim snapshot and hand back the *old* mark — and resuming there is
            // refused identically, this time with nothing left to recover into. The
            // refusal itself is the one answer known to agree with the read that failed.
            OffsetReset::Earliest => match low_water_from_error(&format!("{:#}", err)) {
                Some(low_water) => low_water,
                None => self.weil_client.beginning_offset(topic).await?,
            },
            OffsetReset::Latest => self.weil_client.end_offset(topic).await?,
            OffsetReset::None => {
                return Err(err.context(format!(
                    "topic `{}` has been trimmed past this consumer's position and \
                     offset_reset is `None`",
                    topic
                )));
            }
        };

        self.processed_offsets
            .insert(topic.to_string(), Some(recovered));
        // The buffered block, if any, is from the pre-reset position --
        // discard it so the next `read_block` refills from `recovered`
        // rather than serving records the reset jumped past.
        if let Some(queue) = self.buffered_records.get_mut(topic) {
            queue.clear();
        }
        self.next_read_offset.remove(topic);
        self.last_block_size.remove(topic);
        if let Some(pending) = self.prefetch.remove(topic) {
            pending.handle.abort();
        }

        Ok(recovered)
    }

    /// Acknowledge everything delivered so far, durably.
    ///
    /// Writes `position - 1` for each topic, because a checkpoint holds the highest
    /// sequence acknowledged while a position is the next one to read. A topic whose
    /// position is 0 or unresolved has nothing to acknowledge and is skipped, so the
    /// returned receipts cover only the topics that actually committed.
    pub async fn commit(
        &mut self,
        topic: Option<&str>,
    ) -> Result<Vec<CommitReceipt>, anyhow::Error> {
        let topics: Vec<String> = match topic {
            Some(topic) => vec![topic.to_string()],
            None => self.topics.clone(),
        };

        let mut receipts = Vec::new();

        for topic in topics {
            let sequence = match self
                .processed_offsets
                .get(&topic)
                .copied()
                .flatten()
                .and_then(|offset| offset.checked_sub(1))
            {
                Some(sequence) => sequence,
                None => continue,
            };

            receipts.push(self.weil_client.commit(&topic, sequence).await?);
        }

        Ok(receipts)
    }

    pub fn spin_channels() -> (
        Sender<(String, StreamRecord)>,
        Receiver<(String, StreamRecord)>,
    ) {
        channel(MAX_CHANNEL_CAPACITY as usize)
    }

    pub async fn spin_poll(
        &mut self,
        sender: Sender<(String, StreamRecord)>,
    ) -> Result<(), anyhow::Error> {
        loop {
            let records_map = self.poll(None).await?;

            for (topic, records) in records_map {
                for record in records {
                    sender
                        .send((topic.clone(), record))
                        .await
                        .expect("sending of stream record failed");
                }
            }
        }
    }
}
