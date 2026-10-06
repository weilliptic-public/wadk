//! # WeilStream protocol core
//!
//! The wire types and pure protocol arithmetic of WeilStream, per the Technical Whitepaper
//! (Draft v0.1) and the Consumer Architecture summary (September 2026).
//!
//! Nothing here calls into the WeilChain runtime, so this crate links equally into the
//! Publisher applet and into an off-chain consumer driver. It carries two things:
//!
//! * **The record shape** — [`StreamRecord`] and friends. Records cross a contract
//!   boundary, so the JSON field names are an interface. A driver deserializing what
//!   `Publisher.read` returns should use these types rather than mirror them, which is what
//!   turns a field rename into a compile error instead of a runtime surprise.
//! * **The protocol arithmetic** — the per-topic hash chain, and modulo work assignment
//!   across parallel consumers. Both are exercised by ordinary tests, which matters:
//!   this is the logic where a subtle mistake corrupts a stream rather than failing loudly.
//!
//! ## Where the assignment helpers belong
//!
//! [`owned_seqs`], [`owner_of_seq`] and [`Generation`] are **driver-side**. The Consumer
//! Architecture puts modulo work assignment and resizing outside WeilChain: a consumer
//! group is a processing policy the SDK implements, not an on-chain object, and the
//! configuration epochs below are an optional management mechanism rather than a WeilStream
//! artifact. No applet calls them; they are here as the reference implementation a Rust
//! driver links, so every driver divides a stream the same way.
//!
//! Storage, authorization and consensus ordering belong to the applets.

use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256};
use weil_macros::WeilType;

// ---------------------------------------------------------------------------
// Wire-level error markers
// ---------------------------------------------------------------------------

// An applet returns `Result<_, String>`, so the only thing that crosses the contract
// boundary is prose. A driver still has to tell the two *retriable* conditions apart from
// a genuine failure, and matching on a sentence is how that silently breaks the first time
// someone rewords an error. These markers prefix those messages and are declared here
// because the applet and every driver already depend on this crate — one definition, and a
// rename is a compile error on both sides rather than a consumer that stops resuming.
//
// The names are Kafka's, because the conditions are Kafka's and a driver written against
// Kafka's client contract should recognize them.

/// The topic has no state yet — nothing has ever been published to it.
///
/// Kafka's `UNKNOWN_TOPIC_OR_PARTITION`: retriable, and a consumer polling one should see
/// an empty batch rather than an error. A topic here comes into existence on its first
/// publish, so a subscriber that starts before the producer hits this legitimately.
pub const ERR_UNKNOWN_TOPIC: &str = "UNKNOWN_TOPIC";

/// The requested sequence is below the topic's low-water mark — retention has passed it.
///
/// Kafka's `OFFSET_OUT_OF_RANGE`: the broker refuses rather than skipping forward, and the
/// client decides what to do about it via `auto.offset.reset`. Silently serving the next
/// surviving record instead would let a consumer advance past a gap without ever learning
/// there was one.
pub const ERR_OFFSET_OUT_OF_RANGE: &str = "OFFSET_OUT_OF_RANGE";

/// Precedes the low-water mark in [`offset_trimmed`]; what [`low_water_from_error`] seeks.
const LOW_WATER_MARKER: &str = "the lowest retained sequence is `";

/// The refusal a read below the low-water mark gets, carrying the mark that refused it.
///
/// Formatter and parser ([`low_water_from_error`]) live together deliberately: the mark is
/// load-bearing for client recovery, not decoration, so a reworded message that stopped
/// being parseable would silently break `auto.offset.reset` rather than fail a build.
pub fn offset_trimmed(requested: u64, low_water: u64) -> String {
    format!(
        "{}: sequence `{}` has been trimmed; {}{}`",
        ERR_OFFSET_OUT_OF_RANGE, requested, LOW_WATER_MARKER, low_water
    )
}

/// Recover the low-water mark from a rendered [`offset_trimmed`] refusal.
///
/// A client that has just been refused needs the mark to know where to resume. Asking for
/// it again with `beginning_offset` is a *separate* query, which a replica still on the
/// pre-trim snapshot can answer with the old mark — and resuming there fails identically.
/// The refusing node already reported the mark it enforced, so read it from the refusal.
///
/// Tolerates wrapping: the message arrives nested in JSON inside transport context, and
/// the backticks around the number survive JSON escaping.
pub fn low_water_from_error(rendered: &str) -> Option<u64> {
    let start = rendered.find(LOW_WATER_MARKER)? + LOW_WATER_MARKER.len();
    let rest = &rendered[start..];

    rest[..rest.find('`')?].parse().ok()
}

/// Domain separator for the per-topic hash chain.
const ENTRY_DOMAIN: &[u8] = b"weilstream.entry.v1";

/// Domain separator for a topic's genesis hash.
const GENESIS_DOMAIN: &[u8] = b"weilstream.genesis.v1";

// ---------------------------------------------------------------------------
// Hashing
// ---------------------------------------------------------------------------

/// Absorb one length-prefixed field.
///
/// The length prefix is what stops two different field splits from hashing alike —
/// without it `("ab", "c")` and `("a", "bc")` would produce the same digest, which would
/// let a forged record verify at the wrong position.
fn absorb(hasher: &mut Sha3_256, field: &[u8]) {
    hasher.update((field.len() as u64).to_le_bytes());
    hasher.update(field);
}

/// Lowercase hex SHA3-256 of `bytes`.
pub fn sha3_hex(bytes: &[u8]) -> String {
    hex::encode(Sha3_256::digest(bytes))
}

/// Genesis hash for a topic's chain.
///
/// Derived from the contract id as well as the topic name, so the same topic name on two
/// different deployments cannot produce a chain that replays into the other.
pub fn genesis_hash(contract_id: &str, topic: &str) -> String {
    let mut hasher = Sha3_256::new();
    absorb(&mut hasher, GENESIS_DOMAIN);
    absorb(&mut hasher, contract_id.as_bytes());
    absorb(&mut hasher, topic.as_bytes());

    hex::encode(hasher.finalize())
}

/// The link in a topic's hash chain for one committed entry.
///
/// Every field a verifier is asked to trust is folded in, so a record's digest can be
/// re-derived from the stored record alone.
#[allow(clippy::too_many_arguments)]
pub fn entry_hash(
    prev_entry_hash: &str,
    topic: &str,
    seq: u64,
    payload_hash: &str,
    publisher: &str,
    txn_id: &str,
    block_height: u64,
    block_timestamp: &str,
) -> String {
    let mut hasher = Sha3_256::new();
    absorb(&mut hasher, ENTRY_DOMAIN);
    absorb(&mut hasher, prev_entry_hash.as_bytes());
    absorb(&mut hasher, topic.as_bytes());
    absorb(&mut hasher, &seq.to_le_bytes());
    absorb(&mut hasher, payload_hash.as_bytes());
    absorb(&mut hasher, publisher.as_bytes());
    absorb(&mut hasher, txn_id.as_bytes());
    absorb(&mut hasher, &block_height.to_le_bytes());
    absorb(&mut hasher, block_timestamp.as_bytes());

    hex::encode(hasher.finalize())
}

// ---------------------------------------------------------------------------
// Wire types
// ---------------------------------------------------------------------------

/// A content-addressed reference to a payload held outside consensus state.
///
/// Sequence provides order, `object_location` provides retrieval, and `payload_hash`
/// binds the external bytes to the certified stream entry. Availability of the bytes is
/// the external store's problem; WeilChain only commits to *which* payload belongs at
/// that sequence.
#[derive(Debug, Clone, Serialize, Deserialize, WeilType)]
pub struct PayloadRef {
    /// Lowercase hex SHA3-256 of the external object's bytes.
    pub payload_hash: String,
    pub object_location: String,
    pub content_len: u64,
    pub content_type: Option<String>,
}

/// The stored form of an event.
#[derive(Debug, Clone, Serialize, Deserialize, WeilType)]
pub enum Payload {
    Inline(String),
    Reference(PayloadRef),
}

/// One committed entry in a topic's canonical sequence.
#[derive(Debug, Clone, Serialize, Deserialize, WeilType)]
pub struct StreamRecord {
    pub topic: String,
    pub seq: u64,
    pub payload: Payload,
    pub payload_hash: String,
    pub publisher: String,
    pub txn_id: String,
    pub block_height: u64,
    pub block_timestamp: String,
    /// `entry_hash` of sequence `seq - 1`, or the topic's genesis hash for `seq == 0`.
    pub prev_entry_hash: String,
    pub entry_hash: String,
}

impl StreamRecord {
    /// Re-derive this record's chain link from its own stored fields.
    ///
    /// A record is intact exactly when this equals its stored `entry_hash` *and* its
    /// `prev_entry_hash` matches the previous record's `entry_hash`.
    pub fn recompute_entry_hash(&self) -> String {
        entry_hash(
            &self.prev_entry_hash,
            &self.topic,
            self.seq,
            &self.payload_hash,
            &self.publisher,
            &self.txn_id,
            self.block_height,
            &self.block_timestamp,
        )
    }
}

/// One segment block's worth of records served back to a consumer.
#[derive(Debug, Clone, Serialize, Deserialize, WeilType)]
pub struct StreamBatch {
    pub records: Vec<StreamRecord>,
    pub next_offset: u64,
    pub eof: bool,
}

/// Acknowledgement that an event now holds a committed position.
#[derive(Debug, Clone, Serialize, Deserialize, WeilType)]
pub struct PublishReceipt {
    pub topic: String,
    pub seq: u64,
    pub payload_hash: String,
    pub entry_hash: String,
    pub txn_id: String,
    pub block_height: u64,
    pub block_timestamp: String,
}

/// Evidence that a contiguous range belongs to the certified history.
///
/// This is what makes proofs batchable: a consumer or auditor verifies a whole range
/// against one anchor and one head rather than checking every event individually.
#[derive(Debug, Clone, Serialize, Deserialize, WeilType)]
pub struct RangeProof {
    pub topic: String,
    pub from_seq: u64,
    pub to_seq: u64,
    /// `prev_entry_hash` of `from_seq` — the chain position the range hangs off.
    pub anchor_entry_hash: String,
    /// `entry_hash` of `to_seq`.
    pub range_head_entry_hash: String,
    /// The topic's head at the time the proof was produced.
    pub topic_head_entry_hash: String,
    pub entry_hashes: Vec<String>,
}

/// Walk a contiguous run of records and confirm it forms an unbroken chain.
///
/// Two things are checked per record: that its stored `entry_hash` is what its own fields
/// hash to (so a field cannot be edited without detection), and that its `prev_entry_hash`
/// equals the previous record's `entry_hash` (so a record cannot be inserted, dropped or
/// reordered). Returns the hash the range lands on.
///
/// The caller supplies the records; this says nothing about whether they came from the
/// certified history, only that they are self-consistent. Anchoring the result against a
/// head obtained from consensus state is what closes that gap.
pub fn walk_chain(records: &[StreamRecord]) -> Option<String> {
    let first = records.first()?;

    let mut link = first.prev_entry_hash.clone();
    for record in records {
        if record.prev_entry_hash != link || record.recompute_entry_hash() != record.entry_hash {
            return None;
        }
        link = record.entry_hash.clone();
    }

    Some(link)
}

// ---------------------------------------------------------------------------
// Consumer group arithmetic
// ---------------------------------------------------------------------------

/// One (count, effective-from) mapping in a group's history.
///
/// Changing the consumer count changes modulo ownership — sequence 12 belongs to consumer 0
/// under `N = 4` but to consumer 4 under `N = 8` — so a live resize cannot reinterpret
/// history under the new width. Keeping the mappings rather than overwriting a single count
/// is what gives a driver a safe cutover: a sequence's owner is decided by the generation in
/// force *at that sequence*, so old work finishes under the old width while new work begins
/// under the new one.
///
/// Driver-side. This is the management mechanism the Consumer Architecture leaves outside
/// WeilChain, not on-chain state.
#[derive(Debug, Clone, Serialize, Deserialize, WeilType)]
pub struct Generation {
    pub generation: u64,
    /// Lowest sequence this generation governs.
    pub start_seq: u64,
    /// Number of logical slots in this generation.
    pub count: u32,
}

/// Index of the generation in force at `seq`.
///
/// `generations` is non-empty and ordered by `start_seq`, so this is the last entry whose
/// `start_seq` is at or below `seq`.
pub fn generation_index_at(generations: &[Generation], seq: u64) -> usize {
    let mut index = 0;
    for (i, generation) in generations.iter().enumerate() {
        if generation.start_seq <= seq {
            index = i;
        } else {
            break;
        }
    }

    index
}

/// Which consumer owns `seq`: `owner(seq) = seq mod N` under the generation in force.
///
/// Deterministic and purely arithmetic — there is no per-message assignment coordinator,
/// which is what lets slots be added without a rebalance protocol.
pub fn owner_of_seq(generations: &[Generation], seq: u64) -> Result<u32, String> {
    let Some(first) = generations.first() else {
        return Err("consumer group has no generations".to_string());
    };
    if seq < first.start_seq {
        return Err(format!(
            "sequence `{}` is below the group's first generation, which starts at `{}`",
            seq, first.start_seq
        ));
    }

    let generation = &generations[generation_index_at(generations, seq)];
    // A generation with no slots owns nothing and would otherwise divide by zero. The
    // fields are public and driver-supplied, so this is reachable from a configuration
    // mistake rather than from a bug in here — it must not be a panic inside a consumer.
    if generation.count == 0 {
        return Err(format!(
            "generation `{}` has a count of zero, so no slot owns sequence `{}`",
            generation.generation, seq
        ));
    }

    Ok((seq % generation.count as u64) as u32)
}

/// The next `max` sequences owned by `slot`, scanning from `from` up to (not including)
/// `head`.
///
/// This is a driver's cursor calculation: for a simple ordered consumer the stride is one
/// and this is not needed, and for a modulo-based parallel consumer it yields exactly the
/// sequences that consumer owns.
///
/// Within one generation the owned sequences are an arithmetic progression of stride
/// `count`, so this strides rather than testing every sequence — a slot in a group of 64
/// does not walk 63 sequences it does not own to find the one it does. At a generation
/// boundary the stride is recomputed, because a resize changes ownership from that
/// sequence onward.
///
/// A slot index beyond a generation's `count` simply owns nothing in that generation.
/// That is the normal state of the extra slots after a shrink, and of a high slot index
/// scanning a region that predates its own generation after a grow.
pub fn owned_seqs(
    generations: &[Generation],
    slot: u32,
    from: u64,
    head: u64,
    max: usize,
) -> Vec<u64> {
    let mut out = Vec::new();
    let Some(first) = generations.first() else {
        return out;
    };
    if max == 0 {
        return out;
    }

    let mut cursor = from.max(first.start_seq);

    while cursor < head && out.len() < max {
        let index = generation_index_at(generations, cursor);
        let generation = &generations[index];
        let count = generation.count as u64;

        // Ownership only holds up to the next generation's start, or the head.
        let bound = generations
            .get(index + 1)
            .map(|next| next.start_seq)
            .unwrap_or(head)
            .min(head);
        if bound <= cursor {
            break;
        }

        if (slot as u64) < count {
            let offset = (slot as u64 + count - (cursor % count)) % count;
            let mut seq = cursor + offset;
            while seq < bound && out.len() < max {
                out.push(seq);
                seq += count;
            }
        }

        cursor = bound;
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_refusal_round_trips_its_low_water_mark() {
        for (requested, low_water) in [(0, 4), (2, 4), (0, 0), (7, u64::MAX)] {
            assert_eq!(
                low_water_from_error(&offset_trimmed(requested, low_water)),
                Some(low_water),
                "round trip for requested `{requested}`, low water `{low_water}`"
            );
        }
    }

    #[test]
    fn a_refusal_survives_the_transport_wrapping_it() {
        // Shape observed on the wire: the applet's message nested in the JSON body of a
        // settled response, itself inside the client's transport context. Recovery reads
        // the mark through all of it, so the wrapping is part of what is under test.
        let wrapped = format!(
            "failed to submit the transaction: \"{{\\\"message\\\":\\\"method `read_batch` \
             returned with an error: method `read_batch_data` returned with an error: {}\\\",\
             \\\"status\\\":\\\"failure\\\"}}\"",
            offset_trimmed(0, 4)
        );

        assert!(wrapped.contains(ERR_OFFSET_OUT_OF_RANGE));
        assert_eq!(low_water_from_error(&wrapped), Some(4));
    }

    #[test]
    fn an_unrelated_error_yields_no_mark() {
        // Recovery falls back to `beginning_offset` on `None`, so a wrong `Some` is the
        // dangerous answer: it would resume a consumer at a position nothing vouched for.
        for rendered in [
            "",
            "UNKNOWN_TOPIC",
            "topic `t` is empty",
            "sequence `9` has not been published; the next sequence is `6`",
            // The `verify` refusal shares the error code but carries a range, not a mark.
            &format!(
                "{}: sequence `9` is outside the retained range `[4, 6)`",
                ERR_OFFSET_OUT_OF_RANGE
            ),
            // Truncated mid-number: the closing backtick never arrives.
            "the lowest retained sequence is `4",
        ] {
            assert_eq!(low_water_from_error(rendered), None, "for `{rendered}`");
        }
    }

    fn generations(spec: &[(u64, u64, u32)]) -> Vec<Generation> {
        spec.iter()
            .map(|(generation, start_seq, count)| Generation {
                generation: *generation,
                start_seq: *start_seq,
                count: *count,
            })
            .collect()
    }

    fn record(seq: u64, prev: &str, payload_hash: &str) -> StreamRecord {
        StreamRecord {
            topic: "orders".to_string(),
            seq,
            payload: Payload::Inline("A".to_string()),
            payload_hash: payload_hash.to_string(),
            publisher: "wallet_a".to_string(),
            txn_id: "txn_a".to_string(),
            block_height: 100 + seq,
            block_timestamp: "2026-09-05T00:00:00Z".to_string(),
            prev_entry_hash: prev.to_string(),
            entry_hash: entry_hash(
                prev,
                "orders",
                seq,
                payload_hash,
                "wallet_a",
                "txn_a",
                100 + seq,
                "2026-09-05T00:00:00Z",
            ),
        }
    }

    /// Brute-force reference for [`owned_seqs`]: test every sequence rather than stride.
    fn reference_owned(
        gens: &[Generation],
        slot: u32,
        from: u64,
        head: u64,
        max: usize,
    ) -> Vec<u64> {
        (from.max(gens[0].start_seq)..head)
            .filter(|seq| owner_of_seq(gens, *seq) == Ok(slot))
            .take(max)
            .collect()
    }

    #[test]
    fn an_inline_payload_hashes_to_its_bytes() {
        // Known SHA3-256 of the empty input, to pin the digest choice itself.
        assert_eq!(
            sha3_hex(b""),
            "a7ffc6f8bf1ed76651c14756a061d662f580ff4de43b49fa82d80a4b80f8434a"
        );
        assert_ne!(sha3_hex(b"a"), sha3_hex(b"b"));
    }

    #[test]
    fn a_records_hash_is_reproducible_from_its_own_fields() {
        let record = record(0, "genesis", "hash_a");

        assert_eq!(record.recompute_entry_hash(), record.entry_hash);
    }

    #[test]
    fn every_committed_field_is_bound_into_the_hash() {
        let base = record(1, "prev", "hash_a");

        // Changing any committed field must change the digest, or that field could be
        // edited after the fact without detection.
        let mutations: Vec<Box<dyn Fn(&mut StreamRecord)>> = vec![
            Box::new(|r| r.payload_hash = "hash_b".to_string()),
            Box::new(|r| r.seq = 2),
            Box::new(|r| r.topic = "shipments".to_string()),
            Box::new(|r| r.publisher = "wallet_b".to_string()),
            Box::new(|r| r.txn_id = "txn_b".to_string()),
            Box::new(|r| r.block_height = 999),
            Box::new(|r| r.block_timestamp = "2026-09-06T00:00:00Z".to_string()),
            Box::new(|r| r.prev_entry_hash = "other".to_string()),
        ];

        for (index, mutate) in mutations.iter().enumerate() {
            let mut edited = base.clone();
            mutate(&mut edited);
            assert_ne!(
                edited.recompute_entry_hash(),
                base.entry_hash,
                "mutation {} left the digest unchanged",
                index
            );
        }
    }

    #[test]
    fn field_boundaries_are_unambiguous() {
        // Without the length prefix these two splits would absorb identical bytes, which
        // would let one record's hash stand in for another's.
        let left = entry_hash("ab", "c", 0, "h", "p", "t", 1, "ts");
        let right = entry_hash("a", "bc", 0, "h", "p", "t", 1, "ts");

        assert_ne!(left, right);
    }

    #[test]
    fn topics_do_not_share_a_chain_origin() {
        let orders = genesis_hash("contract_a", "orders");
        let shipments = genesis_hash("contract_a", "shipments");
        let other_deployment = genesis_hash("contract_b", "orders");

        assert_ne!(orders, shipments);
        assert_ne!(orders, other_deployment);
    }

    #[test]
    fn an_intact_chain_walks_to_its_head() {
        let genesis = genesis_hash("contract_a", "orders");
        let first = record(0, &genesis, "hash_a");
        let second = record(1, &first.entry_hash, "hash_b");
        let third = record(2, &second.entry_hash, "hash_c");

        assert_eq!(
            walk_chain(&[first, second, third.clone()]),
            Some(third.entry_hash)
        );
    }

    #[test]
    fn the_chain_detects_an_edited_record() {
        let genesis = genesis_hash("contract_a", "orders");
        let first = record(0, &genesis, "hash_a");
        let second = record(1, &first.entry_hash, "hash_b");
        let third = record(2, &second.entry_hash, "hash_c");

        // Rewriting a payload without rewriting the chain fails the self-check.
        let mut tampered = second.clone();
        tampered.payload_hash = "hash_x".to_string();
        assert_eq!(
            walk_chain(&[first.clone(), tampered.clone(), third.clone()]),
            None
        );

        // Rewriting the chain too repairs the self-check but breaks the link to the
        // record that follows, so the range still fails.
        tampered.entry_hash = tampered.recompute_entry_hash();
        assert_eq!(walk_chain(&[first.clone(), tampered, third.clone()]), None);

        // Dropping a record breaks the link as well.
        assert_eq!(walk_chain(&[first, third]), None);
    }

    #[test]
    fn owner_is_seq_mod_count_within_a_generation() {
        let gens = generations(&[(0, 0, 4)]);

        // The whitepaper's example: consumer_0 owns 0, 4, 8, 12, ...
        for (seq, slot) in [(0u64, 0u32), (1, 1), (2, 2), (3, 3), (4, 0), (13, 1)] {
            assert_eq!(owner_of_seq(&gens, seq), Ok(slot));
        }
    }

    #[test]
    fn a_resize_does_not_reassign_historical_ownership() {
        // Whitepaper section 10: generation 7 covers [0, 10000) with 4 slots,
        // generation 8 covers [10000, ..) with 6.
        let gens = generations(&[(7, 0, 4), (8, 10_000, 6)]);

        assert_eq!(owner_of_seq(&gens, 9_999), Ok((9_999 % 4) as u32));
        assert_eq!(owner_of_seq(&gens, 10_000), Ok((10_000 % 6) as u32));
    }

    #[test]
    fn sequences_below_the_first_generation_belong_to_no_slot() {
        let gens = generations(&[(0, 100, 4)]);

        assert!(owner_of_seq(&gens, 99).is_err());
        assert!(owner_of_seq(&gens, 100).is_ok());
    }

    #[test]
    fn owned_seqs_matches_brute_force_across_two_resizes() {
        let gens = generations(&[(0, 0, 4), (1, 37, 6), (2, 80, 3)]);

        for slot in 0..6u32 {
            for from in [0u64, 1, 36, 37, 40, 79, 80, 95] {
                assert_eq!(
                    owned_seqs(&gens, slot, from, 100, 12),
                    reference_owned(&gens, slot, from, 100, 12),
                    "slot {} from {}",
                    slot,
                    from
                );
            }
        }
    }

    #[test]
    fn slots_of_a_group_partition_the_stream_exactly() {
        let gens = generations(&[(0, 0, 4), (1, 37, 6)]);

        let mut seen: Vec<u64> = (0..6u32)
            .flat_map(|slot| owned_seqs(&gens, slot, 0, 100, 100))
            .collect();
        seen.sort_unstable();

        // Every sequence is covered exactly once: no duplicates, no gaps.
        assert_eq!(seen, (0..100).collect::<Vec<u64>>());
    }

    #[test]
    fn a_slot_beyond_a_generations_count_owns_nothing_in_it() {
        // Slot 5 exists only from generation 1 onward; it must not claim anything below 37.
        let gens = generations(&[(0, 0, 4), (1, 37, 6)]);

        assert!(owned_seqs(&gens, 5, 0, 37, 10).is_empty());
        assert_eq!(owned_seqs(&gens, 5, 0, 100, 3), vec![41, 47, 53]);
    }

    #[test]
    fn a_shrink_leaves_the_retired_slots_their_historical_work() {
        // Generation 1 halves the group to 2 slots from sequence 20. Slots 2 and 3 keep
        // the work generation 0 already assigned them, and take nothing after.
        let gens = generations(&[(0, 0, 4), (1, 20, 2)]);

        assert_eq!(owned_seqs(&gens, 3, 0, 40, 10), vec![3, 7, 11, 15, 19]);
        assert_eq!(owned_seqs(&gens, 3, 20, 40, 10), Vec::<u64>::new());
        // A surviving slot switches stride at the boundary: 17 under generation 0's
        // stride of 4, then 21, 23, 25 under generation 1's stride of 2.
        assert_eq!(owned_seqs(&gens, 1, 16, 26, 10), vec![17, 21, 23, 25]);
    }

    #[test]
    fn a_zero_count_generation_is_refused_rather_than_dividing_by_zero() {
        // `Generation`'s fields are public and driver-supplied, so a count of zero is a
        // configuration mistake that reaches this code — it must not panic inside a
        // running consumer.
        let gens = generations(&[(0, 0, 0)]);

        assert!(owner_of_seq(&gens, 0).is_err());
        assert!(owned_seqs(&gens, 0, 0, 10, 5).is_empty());
    }

    #[test]
    fn owned_seqs_honours_the_batch_limit_and_the_head() {
        let gens = generations(&[(0, 0, 4)]);

        assert_eq!(owned_seqs(&gens, 1, 0, 100, 3), vec![1, 5, 9]);
        assert_eq!(owned_seqs(&gens, 1, 0, 6, 10), vec![1, 5]);
        assert!(owned_seqs(&gens, 1, 0, 100, 0).is_empty());
        assert!(owned_seqs(&gens, 1, 100, 100, 10).is_empty());
    }
}
