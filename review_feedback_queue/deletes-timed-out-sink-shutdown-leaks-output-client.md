# Delete’s timed-out sink shutdown leaks an output client

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Delete could leave a still-tracked output client retrying after timeout.

## Details

F333 — **possible** — `crates/farhelm-supervisor/src/service/teardown.rs:607` — Delete’s timed-out sink shutdown leaks
an output client

The output-client wait can time out while its registered cleanup task remains pending, yet teardown destroys the
session. Safe shutdown retains that client and retries, so the supported consequence is a leaked or retrying local
client, not host-wide work loss or a tmux-server crash. Non-hung delay beyond the limit was not reproduced. Preserve
cleanup responsibility until completion and expose pending retry state after session teardown.

## Evidence and triage context

- terminals.rs:1943-1955 can return TimedOut with a registered sink reaper still pending. teardown.rs:607-618 warns and
  kills its tmux session anyway. The separate output_reaps guard at :515-550 does not enumerate these sink waits.
  terminals.rs:736-758 intentionally retains the client and retries safe shutdown.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- TRIAGE_OUTCOMES.md:4677-4679 accepts the different overwritten-record tracking gap, not expiry of a tracked wait.
  shutdown-expiry.md concerns supervisor exit, also a different consequence. FILTER.md:44-64 would cover a strictly
  hung-tmux trigger with only host-local leakage, but the evidence does not establish that this trigger is necessary.

Caveats:

- No ordinary reproduction. A non-hung delayed shutdown exceeding the five-second limit remains an open premise. Do not
  describe this as demonstrated security, work loss or a tmux-server crash.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `ss_general:p1:C5`.

- `ss_general:p1:C5`: confidence as filed: not separately tagged in candidate list; suggested bucket as filed: not
  separately tagged in candidate list.
