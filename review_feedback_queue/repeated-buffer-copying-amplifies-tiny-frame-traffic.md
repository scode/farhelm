# repeated buffer copying amplifies tiny-frame traffic

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Tiny supervisor frames could amplify buffer-copying cost.

## Details

F180 — **possible** — `crates/farhelm-proto/src/io.rs:161` — repeated buffer copying amplifies tiny-frame traffic

Removing each decoded frame from the front of the receive buffer repeatedly moves the remaining bytes. Batched valid
tiny frames therefore make the helm copy substantially more data than it receives, through a remotely reachable path.
Practical SSH batching, traffic rate, and degradation of other hosts remain unmeasured. Use a consumed offset with
occasional compaction or inexpensive prefix removal, and establish practical impact before making stronger availability
claims.

## Evidence and triage context

- crates/farhelm-proto/src/io.rs:160 decodes one frame and drains its prefix from a Vec, moving the buffered remainder.
- crates/farhelm-proto/src/lib.rs:571 permits nine-byte empty Data frames.
- crates/farhelm-helm/src/client.rs:1520 reads and dispatches these frames; :1997 silently ignores unknown terminal
  channels.
- crates/farhelm-helm/src/transport.rs:142 exposes SSH stdout as the frame transport.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- SPEC.md:2290 accepts hostile availability degradation only where avoiding it is not reasonably easy. It does not
  specifically accept this buffer behavior. No exact Planned TODO, BUGS, queue, ledger, or FILTER coverage identified.

Caveats:

- No runtime tests or benchmarks were run.
- Actual SSH batching, attainable traffic rates, scheduler effects, and user-visible degradation remain unverified.
- The initial 16 KiB capacity is neither a guaranteed read size nor a maximum buffer capacity.
- No credential disclosure, unauthorized execution, or work loss is established.
- TODO.md:33-43 concerns supervisor creation blocking its connection reader, not helm frame decoding. BUGS.md records
  different tmux and provisioning failures.
- Actual batching, sustainable traffic rate, and user-visible impact remain unmeasured.
- The initial buffer capacity is neither a guaranteed read size nor a capacity limit.
- No security breach or work loss is established.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `gap_helm_connections_sec_08:p1:F1`.

- `gap_helm_connections_sec_08:p1:F1`: confidence as filed: possible; suggested bucket as filed: other.
