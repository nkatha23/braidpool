# Job Store Transaction Memory — Measurement

**Status:** temporary instrumentation (revert after measuring)
**Related:** A2b — Lazy Transaction Fetch

## Why this exists

The stratum notifier keeps a `GlobalJobStore` that retains up to 20 recent
templates (`GLOBAL_JOB_STORE_CAPACITY = 20`) so that a late share submission can
be matched back to the exact block it was mined against.

Templates are shared between job IDs by `Arc`, but each retained template still
owns its own `Vec<Transaction>`. So the store holds a full copy of every
retained block's transaction set — up to 20 blocks deep.

**A2b (lazy transaction fetch)** proposes dropping those transactions from the
stored template and re-fetching them from the template cache only when a
full-difficulty block is actually found (~500 KB → ~5 KB per job in theory).
Before building that plumbing, the roadmap gate is:

> *v0.1 data showing residual memory matters.*

This change adds the measurement and nothing else. It does **not** change
job-store behavior. It reports how many bytes of transaction data the store is
holding so we can decide whether A2b is worth the complexity or should be
closed.

## What it does

Two additions in `node/src/stratum.rs`, both on `impl GlobalJobStore`:

1. `GlobalJobStore::transaction_bytes()` — sums `tx.total_size()` (serialized,
   witness-inclusive bytes) over every transaction across every retained
   template. This reflects real memory held, not a vsize/weight approximation.

2. An `info!` log at the end of `insert()`, firing once per job insertion after
   eviction has settled:

   ```
   job store tx memory jobs_held=20 job_store_tx_bytes=4812345
   ```

   - `jobs_held` — distinct templates currently in the store.
   - `job_store_tx_bytes` — the number the gate turns on.

## Measurement A — deterministic test (primary)

Standing up a full IPC node against a mainnet mempool is heavy, and the number
it produces depends on whatever happened to be in the mempool that hour — not
reproducible. Instead, `a2b_job_store_memory_measurement` (in the
`global_job_store_tests` module in `node/src/stratum.rs`) seeds the store with
templates sized like real near-full blocks (~1.8 MB of transactions each),
fills it to the capacity-20 ceiling, and reads back `transaction_bytes()`. It
needs no node, no mempool, and reruns anywhere.

Run it:

```bash
cargo test --lib a2b_job_store_memory_measurement -- --nocapture
```

### Result (measured)

```
per full-block template : 1800324 bytes (1.72 MB)
store capacity          : 20 templates
steady-state tx memory  : 36006480 bytes (34.34 MB)
```

**~34 MB is the worst case** — 20 *distinct* full blocks resident at once.
Templates are shared across job IDs by `Arc`, but their transaction vectors are
not deduplicated, so each distinct retained template contributes a full copy.

This is an upper bound. In a live pool, consecutive templates frequently share a
`TemplateId` (Arc reuse), so `jobs_held` sits below 20 and the real figure is
lower. Measurement B reports that deduped number from a running node.

## Measurement B — live node (secondary, optional)

Confirms the instrumentation fires end-to-end and shows the real, deduped
`jobs_held` under production conditions.

**Prerequisite — you need a real mempool.** On regtest *and* on signet the
mempool is typically near-empty, so templates carry little beyond the coinbase
and `job_store_tx_bytes` reads ~0. For a meaningful figure, point the node at a
**mainnet** `bitcoind`, or generate transaction load yourself on signet/testnet.

1. Run this branch against bitcoind with a populated mempool. Let it run long
   enough for the store to fill — 20+ template refreshes, so `jobs_held` reaches
   its ceiling.

2. Ensure the log is visible. It is at `info!`; default logging shows it. If
   `RUST_LOG` is narrowed, include the stratum module (e.g. `RUST_LOG=info`).

3. Capture stderr to a file and extract the series:

   ```bash
   grep "job store tx memory" node.log \
     | grep -oE 'job_store_tx_bytes=[0-9]+' \
     | cut -d= -f2 > tx_bytes.series
   ```

4. Read the **steady-state plateau** (once `jobs_held` has reached its ceiling),
   not the ramp:

   ```bash
   sort -n tx_bytes.series | tail -1 | awk '{printf "%.1f MB\n", $1/1048576}'
   ```

## How to read the result

| Steady-state tx memory | Verdict |
|---|---|
| ~5–10 MB | A2b saves single-digit MB. Not worth the lazy-fetch plumbing — close A2b; keep this as evidence. |
| Tens of MB or more | A2b is justified; this is the before-number to measure the saving against. |

Record `jobs_held` alongside the bytes. Arc dedup means many job IDs often point
at the same few templates, so `jobs_held` may sit well below 20. If it rarely
climbs past a handful, that itself is a finding: dedup already bounds the
problem and A2b is solving something small.

### Finding (v0.1)

The job store's residual transaction memory is **bounded at ~34 MB worst case**
(20 distinct full blocks), and realistically lower once Arc dedup is accounted
for. A2b would reclaim this down to kilobytes, but ~34 MB is modest next to the
template cache (~45 MB) and the overall node footprint. So at v0.1 scale A2b is
a **nice-to-have, not a blocker** — this measurement is the evidence, and
Measurement B's live `jobs_held`/`job_store_tx_bytes` will confirm the real
(deduped) figure before any decision to build it.

## Cleanup

Measurement A (the test) is cheap and can stay as a regression guard on the
memory bound. Measurement B's `info!` log is temporary: once the live
steady-state number is recorded, revert the log (and `transaction_bytes()` if
unused elsewhere).
