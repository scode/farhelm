# Live pane’s process-read error degrades to PID-only identity

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A live-pane identity read failure could admit an old report by process number alone.

## Details

F330 — **possible** — `crates/farhelm-supervisor/src/service/core.rs:14501` — Live pane’s process-read error degrades to
PID-only identity

When tmux still reports a live pane, any non-running process-read result, including an error, removes the start-token
requirement. Matching then falls back to the numeric process identifier. A wrong conversation binding additionally
requires stale ancestry with a colliding number, which remains unverified. Refuse or retry inconclusive live identity
reads rather than applying the dead-pane fallback at this separate weakening site.

## Evidence and triage context

- service/core.rs:14498–14504 maps every non-running result, including Err, to None; procs.rs:483–486 then matches PID
  alone. SPEC_impl.md:1842–1843 allows PID-only anchoring once the pane has exited, not explicitly after a failed live
  identity read. A wrong binding still requires stale recorded ancestry with a colliding PID; that premise remains
  possible.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- No additional caveat recorded.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `sr_data:p1:C12`.

- `sr_data:p1:C12`: confidence as filed: not separately tagged in candidate list; suggested bucket as filed: not
  separately tagged in candidate list.
