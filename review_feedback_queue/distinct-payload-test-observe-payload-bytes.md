# Distinct-payload test does not observe payload bytes

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

The distinct-payload test could accept installing the wrong executable bytes.

## Details

F282 — **possible** — `crates/farhelm-helm/src/provisioning.rs:6253` — Distinct-payload test does not observe payload
bytes

The fake backend ignores payload contents in this fixture and records only operation labels. Substituting Farhelm's
bytes for the tmux source while keeping the tmux label leaves its completion and label assertions unchanged. Current
production selects correctly and no mutation run occurred. Observe actual uploaded and installed contents, using the
backend's stateful behavior or equivalent recording, and reject deliberate payload substitution.

## Evidence and triage context

- provisioning.rs:6250–6251 states that the test protects the tmux branch from receiving the Farhelm executable. Lines
  6265–6269 create distinct payloads, but FakeBackend::absent initializes stateful=false at 446. upload_path ignores the
  payload at 589–600, and install_path reads/copies it only when stateful is enabled at 612–620; otherwise it records
  the supplied kind at 622–625. The test asserts only completion and operation labels at 6296–6310. Consequently,
  substituting prepared.get(Farhelm) for the install source at provisioning/service.rs:2153 while retaining the action's
  tmux kind at 2159 leaves all these observations unchanged. Current production code selects the correct source; this is
  a definite oracle defect, with no runtime mutation experiment performed. No matching cover was found in Planned, BUGS,
  queue entries, or targeted ledger searches. Suggested disposition: other / would_fix; observe the actual payload
  contents and reject a deliberate substitution.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- None identified in completed collection evidence.

Caveats:

- No additional caveat recorded.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `helm_state_provisioning_05_cor:p1:C2`.

- `helm_state_provisioning_05_cor:p1:C2`: confidence as filed: not separately tagged in candidate list; suggested bucket
  as filed: not separately tagged in candidate list.
