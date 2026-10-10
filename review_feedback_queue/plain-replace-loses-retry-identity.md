# Plain Replace loses its retry identity

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Retrying plain Replace can launch another replacement session.

## Details

F101 — **definite** — `crates/farhelm-ui/src/api.rs:2559` — Plain Replace loses its retry identity

Plain Replace drops its request identity when the response outcome is unknown. If the source session remains, retrying
uses a new identity and creates another replacement rather than recovering the one already launched. Duplicate agents in
the same folder are established; conflicting writes or lost work are not. Retain the key and replacement intent across
unresolved attempts, reconcile with that key, and distinguish retry from an explicitly new replacement, including
acknowledged partial failures.

## Evidence and triage context

- crates/farhelm-ui/src/api.rs:2559 mints a new key per replace_session call and does not return or retain it.
- crates/farhelm-ui/src/list/view.rs:2203 and session_view.rs:1481 call that API; both clear the operation state after
  failure.
- crates/farhelm-helm/src/sessions.rs:2959 requires a surviving source, line 3018 passes the supplied creation key, and
  line 3144 can fail source deletion after creation.
- crates/farhelm-supervisor/src/service/core.rs:7010 deduplicates by key and allocates a new identity for an unseen key.

Retained confidence: **definite**. Suggested bucket: **high**.

Possible cover:

- SPEC.md:798 accepts a reported partial create/delete result. It does not clearly accept silent duplication when that
  result was lost. SPEC.md:601 requires creation retry safety. The completed request-cancellation fix in
  TRIAGE_OUTCOMES.md:5193 addresses server task ownership, not client intent retention.

Caveats:

- Inspection only; no runtime reproduction.
- The source must remain present when the retry reaches the helm. After successful source deletion, the live-source
  lookup normally refuses the retry.
- A received partial-failure response already names the replacement. The strongest uncovered case is an unknown outcome,
  not a deliberate additional Replace after inspecting that response.
- Multiple replacements are established; loss of work depends on what the duplicated stored command does. Highest is a
  conservative provisional bucket.
- TODO.md:33–43 plans moving creation off the supervisor connection reader, a different mechanism. BUGS.md and the
  inspected queue/ledger material provide no matching acceptance.
- The source must still exist when the retry resolves it.
- A deliberate new Replace after reading a partial-failure response is not the strongest defect case.
- Do not claim data loss merely because duplicate commands could conceivably interfere.
- No runtime reproduction.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `ui_desktop_04_cor:p1:F1`.

- `ui_desktop_04_cor:p1:F1`: confidence as filed: definite; suggested bucket as filed: highest.
