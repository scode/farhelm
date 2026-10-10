# The busy-pane assertion depends on another pane’s readiness

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A delayed producer could make the busy-pane test reject correct status.

## Details

F165 — **possible** — `crates/farhelm-supervisor/src/service/ticker.rs:5869` — The busy-pane assertion depends on
another pane’s readiness

Readiness is established from the quiet pane, but the busy producer can be descheduled while tmux and sampling continue.
Both panes can then accumulate unchanged samples; the busy pane correctly becomes Idle while the test unconditionally
expects Running. No failing execution was observed. Establish a changed busy-pane sample after the quiet pane qualifies,
then settle sampling before final assertions.

## Evidence and triage context

- crates/farhelm-supervisor/src/service/ticker.rs:5848–5854: the busy shell produces output independently of the
  sampler.
- crates/farhelm-supervisor/src/service/ticker.rs:5862–5873: readiness checks only the quiet pane's streak.
- crates/farhelm-supervisor/src/service/ticker.rs:545–563: each pane's identical captures increment its own streak.
- crates/farhelm-supervisor/src/agent_kind/screen_reader.rs:204–213 and
  crates/farhelm-supervisor/src/service/status.rs:348–354: a qualifying unchanged streak correctly reads Idle.
- crates/farhelm-supervisor/src/service/ticker.rs:3758–3769: the neighboring test waits for a busy-pane change after
  quiet readiness and settles the ticker.

Retained confidence: **possible**. Suggested bucket: **other**.

Possible cover:

- SPEC_impl.md:1643–1655 specifies per-pane sample-based classification and supports Idle for this input; it does not
  establish the fixture's producer-readiness assumption.

Caveats:

- No failing run was observed.
- The open premise is producer descheduling long enough for qualifying unchanged captures while tmux and the sampler
  continue.
- SPEC_impl.md:1643–1655 supports the classifier's behavior under that schedule; it does not validate the fixture's
  assumption.
- The open premise is sufficiently long producer descheduling while the sampler continues.
- No product classification defect is alleged.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `gap_supervisor_state_cor_07:p1:F3`.

- `gap_supervisor_state_cor_07:p1:F3`: confidence as filed: possible; suggested bucket as filed: other.
