# A failed sink record hides an older reaper

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

An untracked older reaper could make planned shutdown finish cleanup too early.

## Details

F332 — **possible** — `crates/farhelm-supervisor/src/service/terminals.rs:1911` — A failed sink record hides an older
reaper

A failed output-client record can leave an older client's cleanup task without a registered completion receiver. The
global wait counts tracked workers and candidates, allowing planned teardown to return before that task finishes.
Ordinary overwritten-record Delete behavior is already accepted; this remainder concerns unsafe planned-stop completion.
Reachability and a tmux-server crash remain unverified. Preserve tracking of outstanding cleanup tasks or include their
ownership in the shutdown wait.

## Evidence and triage context

- TRIAGE_OUTCOMES.md:4677–4679 and terminals.rs:1911–1918 accept Delete being unable to wait for an overwritten client's
  reaper. terminals.rs:1124–1144 can create no tracked receiver for an unregistered final handle; lines 1154–1157 still
  launch its shutdown. wait_for_all_output_cleanup at 1759–1811 counts registered Reaping/Live entries and candidates,
  not that detached receiverless reaper. teardown.rs:1152–1155 treats return from this wait as completion.
  shutdown-expiry.md:19–24 covers timeout expiry, not an omitted reaper causing early completion; BUGS.md:34–44 excludes
  planned stops from its abrupt-death acceptance.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- No additional caveat recorded.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `sr_lifecycle:p1:C4`.

- `sr_lifecycle:p1:C4`: confidence as filed: not separately tagged in candidate list; suggested bucket as filed: not
  separately tagged in candidate list.
