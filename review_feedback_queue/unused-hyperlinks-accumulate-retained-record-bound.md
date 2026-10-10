# Unused hyperlinks accumulate without a retained-record bound

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Empty terminal hyperlinks accumulate metadata without adding visible content.

## Details

F40 — **definite** — `crates/farhelm-ui/assets/vendor/xterm.js:1; bytes [190528,190894); R:4329–4338` — Unused
hyperlinks accumulate without a retained-record bound

Opening an identifier-less hyperlink registers a record and row marker immediately, even when no cell uses it. Closing
the hyperlink clears the active identifier without unregistering that record. Repeated empty open/close pairs on a
stationary row therefore accumulate metadata outside scrollback and pending-write bounds. Row clearing or disposal can
release it; memory exhaustion was not induced. Assess an upstream correction or integration remedy that avoids unused
retention and bounds link metadata.

## Evidence and triage context

- R:4322–4337 routes a nonempty OSC 8 URI to registerLink immediately.
- R:4339–4340 only clears the active urlId.
- R:7549–7556 creates a fresh marker and map entry for every id-less opening.
- R:7584–7585 removes an entry only after its markers disappear.
- R:5074–5082 ties marker disposal to buffer-line lifecycle; repeated empty open/close pairs do not advance that
  lifecycle.

Retained confidence: **definite**. Suggested bucket: **highest**.

Possible cover:

- SPEC.md:2290–2295 requires an unresolved mitigation-cost judgment.
- TRIAGE_OUTCOMES.md:2359–2380 addresses link target presentation, not metadata retention.
- TRIAGE_OUTCOMES.md:6061–6094 addresses transport queues, not completed-write retained state.

Caveats:

- Clearing or disposing the associated row can release records.
- Memory exhaustion was not induced.
- The service-level report below describes the same causal mechanism at a different primary site.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `vendor_02_sec:p1:F9`.

- `vendor_02_sec:p1:F9`: confidence as filed: definite; suggested bucket as filed: highest.
