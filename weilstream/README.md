# WeilStream

A Rust implementation of the WeilStream Technical Whitepaper (Draft v0.1) as WeilChain
applets: a Byzantine-fault-tolerant, **partitionless** event streaming service.

A topic is one canonical ordered sequence, not a collection of application-visible
partition logs. Publishers reach horizontally scaled Sentinels, WeilChain consensus fixes
the order, a deterministic Publisher applet assigns sequence numbers, and Consumer applets
hold each subscriber's progress on chain.

```
Client -> Sentinel (scales out) -> WeilChain consensus (orders) -> Publisher applet
                                                                    |
                                                            seq = next_seq
                                                            stream[seq] = record
                                                            next_seq = seq + 1
```

**One Publisher deployment is one topic.** The topic name is a constructor argument, so no
method takes a topic parameter and there is no topic registry. This is the topic-level
sharding the whitepaper anticipates in section 5: the sequence counter is still a
serialization point, but only for its own topic — nothing serializes one topic against
another, because they do not share an applet, a storage row, or a state root.

## Crates

| Crate | Kind | What it holds |
|---|---|---|
| `weilstream_core` | library | Wire types, the per-topic hash chain, and modulo assignment for parallel consumers. No host runtime, fully unit tested. |
| `weilstream_publisher` | applet (`cdylib`) | One topic: publishes, assigns sequences, certifies history. Deploy one per topic. Declares no read methods. |
| `weilstream_consumer` | applet (`cdylib`) | Query reads plus durable checkpoints keyed by `(topic, checkpoint_id)`. One deployment serves every topic. |
| `loadgen` | binary (`bin/loadgen`) | Load generator, with an optional live web dashboard (`--ui`). Drives N topics through both applets and reports p50/p90/p99. Native, not WASM — an applet has no clock and cannot time itself. |
| `console` | binary (`bin/console`) | A local web UI over one topic: a publish pane and a live subscriber feed, side by side. Native for the same reason `loadgen` is — an applet cannot hold a socket or poll. |

The two applets deploy separately. The Publisher owns the write and certification side and
maintains the next sequence; the Consumer owns the read and checkpoint side, reading in
order from a starting sequence and tracking how far each consumer got. `weilstream_core` links into
the Publisher *and* into an off-chain driver: it defines the record shape once, so a driver
deserializing `Publisher.read` cannot drift from what the applet writes, and it carries the
cursor arithmetic the Consumer Architecture puts in the SDK rather than on chain.

## Build

```sh
# from weil-applets/
cargo build --release --target wasm32-unknown-unknown \
  -p weilstream_publisher -p weilstream_consumer

widl check weilstream/publisher/publisher.widl
widl check weilstream/consumer/consumer.widl

cargo test -p weilstream_core
```

Artifacts land at `target/wasm32-unknown-unknown/release/weilstream_{publisher,consumer}.wasm`,
paired with the `.widl` file next to each crate for deployment.

`cargo test` at the workspace level fails to link on macOS for every applet here,
including these: an applet is a `cdylib` whose host `env` imports have no native
definitions. Use `cargo test -p weilstream_core`, which is a plain library.

## Creating a topic

Creating a topic *is* deploying a Publisher. `weilstream-create-topic` does it in one step
— build if needed, deploy with the topic as a constructor argument, register under WNS:

```sh
./weilstream-create-topic --topic orders --org acme
```

That registers the Publisher as `<topic>::<org>` — here `orders::acme` — so consumers
address the topic by name rather than by contract address. The org is your WNS domain and
may carry subdomains (`--org engg.acme` gives `orders::engg.acme`). `--name` overrides the
WNS name outright, `--inline-max-bytes` sets the inline payload ceiling, and `--dry-run`
prints the CLI command block instead of running it:

```
connect --host sentinel.weilliptic.ai
wallet select-account --derived 0
deploy --file-path .../weilstream_publisher.wasm --widl-file .../publisher.widl \
       --name orders::acme --init-args '{"topic":"orders","inline_max_bytes":0}'
```

The Weilliptic CLI is an interactive REPL, so the script drives it by feeding that block
on stdin. It needs the `cli` binary on PATH (`--cli` points at it elsewhere) and a wallet
under `ACCOUNT_PATH`.

The Publisher also needs an `identity::weil` applet to be resolvable — see
[Authorization](#authorization).

## Deploying the checkpoint store

Deploy `weilstream_consumer.wasm` with `consumer.widl` once. It takes no constructor
arguments and holds no per-topic configuration: `topic` and `checkpoint_id` are opaque
strings, so one deployment serves every topic an organization runs.

## Authorization

`#[secured("weil")]` gates everything that changes a topic — `publish`, `publish_ref` and
`trim`. The macro resolves the `identity::weil` applet, reads its key manager, and requires
the transaction instantiator to hold an Execution or Management key there. So the
whitepaper's open question about topic-level authorization is answered at the **realm**
level: the identity realm is the only authority, and the Publisher keeps no allow-list, no
owner and no admin of its own.

Reads are ungated. A committed entry is already replicated state, and the point of a
certified history is that anyone can verify it.

Deploying is the privileged act. Whoever runs `weilstream-create-topic` chooses the topic
name and the realm that will govern it; after that, what changes is the realm's key
manager, not applet state. Handing a topic over means moving the key in the realm — which
is why the Publisher has no `transfer_ownership`.

The checkpoint store has its own, unrelated rule: the first commit for a
`(topic, checkpoint_id)` claims it, and only that address may advance it afterwards. That
gives consumer isolation without a registration call — which the architecture deliberately
puts outside WeilChain.

Every identity check across both applets uses `Runtime::origin()`, which is what
`#[secured]` itself checks. A worker keeps its identity when relayed through a helper
contract.

## Publishing

Calls go to the topic's own Publisher, so nothing takes a topic argument:

```
publish("{\"id\":1}")                      -> PublishReceipt { seq: 0, entry_hash, ... }
```

Payloads too large for consensus state go to object storage and are referenced:

```
publish_ref(PayloadRef {
    payload_hash:    <sha3-256 of the object's bytes>,
    object_location: "s3://bucket/key",
    content_len:     1048576,
    content_type:    Some("application/json"),
})
```

Sequence provides order, `object_location` provides retrieval, and `payload_hash` binds
the external bytes to the certified entry. After fetching the object, `verify(seq,
payload_hash)` turns "the store gave me these bytes" into "these are the bytes consensus
committed at this position".

## Consuming

A consumer is an external service, agent or process. WeilStream never creates, schedules,
polls for, or runs one — a stateless applet does not wake itself. The Consumer applet is a
**checkpoint store**, and nothing more:

```
WEILCHAIN                          | SDK + customer runtime (external)
-----------------------------------+----------------------------------
read_batch / read_seqs  QUERY      | read loop, batching, retry, backoff
read_seq                QUERY      | local volatile cursor
get_committed           QUERY      | modulo assignment across N consumers
commit                  MUTATE     | commit cadence, resizing, scaling
```

Every call is a query except `commit` — reading an already committed event is a lookup
against replicated state, not a transaction, so only durable progress costs consensus.

WeilChain owns durable truth, the SDK owns consumption policy, the customer's runtime owns
liveness.

### Local cursor versus durable checkpoint

A consumer holds two notions of progress:

```
durable committed sequence on WeilChain = 10,240
local processed sequence                = 10,917
```

The local cursor advances on every processed record and is volatile. The durable checkpoint
advances periodically. Only `commit` is a mutation, so a consumer processes thousands of
events without a consensus write per record:

```
checkpoint = get_committed("orders::acme", "shipping-2")     // query
next_seq   = checkpoint + stride_or_one

loop:
    records = read_batch("orders::acme", next_seq, 1024)     // query
    for record in records:
        process(record)
        update_local_progress(record.seq)
    if commit_due():
        commit("orders::acme", "shipping-2", progress)       // mutate -> CommitReceipt
```

`commit_due()` is the SDK's: after a number of records, after an interval, or whichever
comes first. A count threshold keeps consensus-write overhead low at high throughput; a
time threshold stops a low-volume consumer sitting uncheckpointed.

### Reads

Three query reads, all passthroughs to the topic's Publisher:

| | |
|---|---|
| `read_batch(topic, from_seq, max_count)` | a contiguous run — the ordered consumer's read |
| `read_seqs(topic, seqs)` | a sparse, strided set in one call — the parallel consumer's read |
| `read_seq(topic, seq)` | a single record |

`read_batch` errors if `from_seq` is below the topic's low-water mark, so a consumer that
has fallen behind retention fails loudly rather than silently skipping events. `read_seqs`
instead *drops* sequences past the head or below the mark, so a parallel driver can stride
optimistically without first asking where the head is.

**The read API lives here and only here.** `publisher.widl` declares no read methods: the
Publisher publishes and certifies, the Consumer reads and checkpoints.

Entries do live in the Publisher's state, though, and a cross-contract call can only target
a WASM export — so the Publisher's Rust impl exports one storage accessor, `read_seqs`,
which is deliberately absent from `publisher.widl`. All three shapes above are built on that
one call. It is not part of the Publisher's API and no client should bind to it.

`read_batch` needs the topic's retention window to tell "trimmed" apart from "caught up",
so it fetches that window only when the batch comes back without the record it asked for
first — one cross-contract call in the steady state, two in the case that needs
explaining.

### Checkpoint identity

`checkpoint_id` is deliberately opaque. An SDK may derive it from a group name and a
consumer index — `shipping-2` — while other applications use agent identities or service
names. The applet does not interpret it; it guarantees durable progress for that identity
and nothing else.

There is no registration call: consumers are created outside WeilChain, so a checkpoint
comes into existence by being committed. The first commit for a `(topic, checkpoint_id)`
claims it for the caller, and only that address may advance it afterwards — which is what
keeps one consumer's progress from moving another's. Progress may not move backwards;
re-committing the sequence already recorded is accepted and idempotent, so a restarting
consumer need not special-case its first commit.

`topic` carries one meaning throughout: the topic's WNS name (`orders::acme`) or the raw
contract id of its Publisher. It is the namespace a checkpoint is filed under *and* how a
read resolves the Publisher to call, so an SDK passes the same string to every method.

Reads resolve that name per call. Nothing can be corrupted if WNS is later re-pointed — a
query keeps no cursor — so the stored-binding hazard that applies to progress does not
apply here.

## Crash and replay: at-least-once

Only the checkpoint is provable.

```
committed:  10,240
processed:  10,917
                    x crash

restart -> resume from 10,241
possible replay: 10,241 ... 10,917
```

Periodic checkpointing yields **at-least-once** processing, with a worst-case replay window
of one commit interval — roughly 1,023 records at a 1,024-record cadence. Use idempotent
handlers, or `(topic, sequence)` as a downstream idempotency key, wherever a duplicate side
effect would be harmful.

A committed read position does not make an external side effect exactly-once. Committing
before the action risks losing it on a crash; committing after risks repeating it. If the
business effect is itself a WeilChain mutation, perform it and the checkpoint update in one
transaction. For external systems, use their transactional or idempotency facilities rather
than claiming exactly-once from checkpointing.

## Parallel consumers

A consumer group is a processing policy, not an on-chain object. `N` external consumers
take ids `0..N-1` and divide the canonical sequence by modulo:

```
seq = consumer_id + N * k

consumer 0: 0, 4,  8, 12, ...
consumer 1: 1, 5,  9, 13, ...
consumer 2: 2, 6, 10, 14, ...
consumer 3: 3, 7, 11, 15, ...
```

The physical stream is never partitioned to obtain this. The SDK computes the assignment —
`weilstream_core::owned_seqs` is the reference implementation — and each consumer keeps its
own checkpoint under its own id.

**This is only valid when the assigned events are independent.** If event `n+1` depends on
the completed effects of event `n`, modulo assignment is unsafe: consumers start in order
but completion order is not guaranteed. Use a single ordered consumer for those workloads.

### Resizing

Changing `N` changes ownership — sequence 12 belongs to consumer 0 under `N = 4` and to
consumer 4 under `N = 8` — so a live resize cannot reinterpret history under the new width.
This is a deployment concern, handled by the SDK or a management layer defining a cutover
boundary so old work finishes under the old width. `weilstream_core::Generation` and
`owned_seqs` implement that cutover for a Rust driver; the configuration epochs are an
optional management mechanism, not an on-chain artifact.

## Certified history

Each entry carries `prev_entry_hash` and `entry_hash`, forming a per-topic chain rooted at
a genesis hash derived from the contract id and topic name. Verification checks two things
per record: that the stored `entry_hash` is what the record's own fields hash to, and that
`prev_entry_hash` matches the previous record's `entry_hash`. The first catches an edited
field, the second catches an inserted, dropped or reordered record.

```
verify_range(1000, 1499, expected_range_head)   -> bool
proof_for_range(1000, 1499)                     -> RangeProof
```

One call covers a whole range, so a consumer or auditor verifies a range rather than every
event individually. `RangeProof` carries the same evidence for checking off chain.

This is Byzantine-resistant, cryptographically verifiable tamper evidence **under
WeilChain's fault assumptions** — not absolute physical immutability. Compromise of the
relevant BFT threshold, or destruction of all available data, remains outside the
guarantee.

## Retention

`trim(up_to_seq)` drops records below `up_to_seq` and raises the topic's low-water
mark, bounded to 512 deletions per call so one transaction's work stays bounded. It is
realm-gated like publishing — retention decides what history survives. The chain
survives it: the record at the new low-water mark still carries the `prev_entry_hash` that
anchors everything above it.

`read` **errors** rather than skipping forward when asked for a trimmed sequence, so a
consumer that has fallen behind retention fails loudly instead of silently dropping events.

## Invariants

The whitepaper's section 16 invariants, and where each is enforced:

| Invariant | Where |
|---|---|
| Sequence uniqueness | `next_seq` is the sole assignment point, and one applet is one topic; assignment and increment share one transaction |
| Sequence monotonicity | assignment follows WeilChain's canonical execution order, which is the order the applet runs in |
| Deterministic execution | every input is an argument or a deterministic runtime value; no clock, no randomness |
| Certified binding | `payload_hash` is folded into the per-topic hash chain |
| Consumer isolation | per-checkpoint keys under a per-topic row suffix; first commit claims a checkpoint, and only that owner may advance it |
| Topic authorization | `#[secured("weil")]` on `publish`, `publish_ref` and `trim`, checked against the realm's key manager |
| Topic immutability | the topic name is a constructor argument with no setter; renaming would orphan every consumer's progress and every record's hash |
| Progress monotonicity | a commit may repeat the recorded sequence but never move below it |
| Slot exclusivity | **not enforced on chain** — the customer's runtime must run one process per consumer id |
| Generation stability | driver-side: `weilstream_core::Generation` gives a resize a cutover boundary, so old work finishes under the old width |
| Dependency safety | modulo assignment is only valid for independent events; ordered workloads use a single consumer |

## Scaling

| Dimension | How |
|---|---|
| Ingress | add Sentinels |
| Ordering | WeilChain's existing transaction path |
| Read throughput | queries spread across replicated state holders |
| Subscriber count | independent checkpoint ids; no on-chain object per consumer |
| Parallel processing | add external consumers and widen `N`, where dependencies allow |
| Consensus writes | commit cadence — one mutation per batch, not per record |
| Topic write throughput | bounded by the serialization a total order requires, per topic only — topics do not share an applet |

## Not implemented

Deliberately out of scope for this pass, and named in the whitepaper as open questions:

- Sequence assignment derived directly from a consensus-visible transaction position,
  rather than from a per-topic counter.
- Per-topic publisher policy. Authorization is realm-level via `@secured`; expressing it
  per topic would need the identity realm to carry topic-scoped groups.
- Compaction and archival, and how they interact with proofs. Only `trim` is implemented.
- Dead-letter handling and poison-event routing.
- A Kafka protocol compatibility layer.
- Long-poll reads. `read(topic, from_seq, max_count, wait_ms)` in the Consumer Architecture
  expects the serving **Sentinel** to hold a request until local execution observes new
  events. An applet cannot block, so `wait_ms` is not modelled on `Publisher.read`; adding a
  parameter the applet ignores would be worse than leaving it out.
- The consumer SDK itself — the read loop, batching, retry/backoff, commit cadence and
  cursor calculation. `weilstream_core` has the record types and assignment arithmetic a
  Rust driver needs; the loop around them is not here.

`create_topic` ships as `weilstream-create-topic`, a wrapper over the existing CLI's
`deploy`, rather than as a native CLI subcommand — the Weilliptic CLI is a distributed
binary and is not in any repo checked out here.

These are the applets a consumer SDK and a customer runtime would drive.

## Console

`console` puts both halves of a topic on one page — publish on the left, a live subscriber
on the right:

```sh
cargo run --release -p console -- \
  --publisher <publisher-contract-id> \
  --consumer  <consumer-contract-id> \
  --topic orders
```

Open `http://127.0.0.1:8788`. With no arguments it comes up idle and takes the whole
configuration from the browser.

Nothing is echoed locally. The left pane calls `publish`; the right pane is an ordinary
`WeilStreamConsumer` polling `read_batch`, so a row appears because a poll **read it back
out of committed state** — the gap between the two is the write path end to end. The tiles
show the two cursors this document keeps separating: a volatile position and a durable
checkpoint. Turn auto-commit off and they come apart on screen, which is the replay window
a crash reopens.

Each row is checked against its own `entry_hash` and against the record before it, so a
tampered or reordered history shows up in the feed rather than in a verification call
nobody made.

Like the loadgen dashboard it binds to loopback only: it publishes with the wallet the
process was launched with, and there is no auth on it.

See **[`bin/console/README.md`](bin/console/README.md)** for every flag, what each offset
tile means, and troubleshooting.

## Load testing

`loadgen` drives real traffic through both applets and reports latency percentiles per
topic and in aggregate:

```sh
cargo run --release -p loadgen -- \
  --publisher <publisher-contract-id> \
  --consumer  <consumer-contract-id> \
  --topics 8 --messages 500 --concurrency 32
```

```
PUBLISH — 8 topics · 500 msg/topic · ~256 B payload · concurrency 32 · 61.4s wall

  topic                 ok    err   p50 (ms)   p90 (ms)   p99 (ms)   max (ms)     ops/s
  ─────────────────────────────────────────────────────────────────────────────────────
  loadgen-198f2c-0     500      0     143.10     186.30     236.20     337.10      8.14
  ...
  ─────────────────────────────────────────────────────────────────────────────────────
  ALL                 4000      0     144.50     187.70     239.40     412.80     65.12
```

Add `--ui` for a live dashboard on `http://127.0.0.1:8787` — percentiles and throughput
updating as the run goes, a rolling latency chart, per-topic rows, and Start/Stop. With no
`--publisher` on the command line it comes up idle and takes the whole configuration from
the browser. It binds to loopback only: it drives load with the wallet the process was
launched with, and there is no auth on it.

Topics need no deploys — one Publisher serves all of them, and a topic is created by
publishing to it. Percentiles are nearest-rank rather than interpolated, failures are
counted but kept out of the latency distribution, and the read phase checks every record
back against the sequences the Publisher assigned.

See **[`bin/loadgen/README.md`](bin/loadgen/README.md)** for deploy steps, every flag, how
to read the output, and troubleshooting.
