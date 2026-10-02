# Clipboard writes have no admission bound before the blocking pool

Reviewed commit: 1ec60cc06baff3442bcdd73760a9ebdc8b08c49c

## TLDR

A remote terminal that sends many clipboard updates can queue unbounded blocking work and delay unrelated Farhelm
operations such as session and authentication requests.

## Details

Found by pre-pr-review-swarm run `20261001-1343-1ec60cc-5337` as `F17 / COR-CLIPBOARD-QUEUE`, tagged **definite**.

At `crates/farhelm-helm/src/clipboard.rs:100`, every authenticated OSC 52 clipboard request calls
`tokio::task::spawn_blocking`. The native clipboard sink serializes writes behind one mutex, but no admission limit is
taken before tasks enter Tokio's shared blocking pool. A slow sink therefore leaves one task holding the mutex and an
unbounded queue of additional clipboard tasks waiting behind it. The per-request text limit bounds individual
allocations, not request rate.

Bound admission before spawning. One active write with a bounded latest-value replacement, or a small explicit limit
with refusal, fits the best-effort clipboard contract. Hold admission until the blocking closure actually finishes, and
test a burst against a deliberately blocked sink.
