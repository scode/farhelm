# Unbounded hyperlink records can exhaust the viewer’s memory

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

Empty hyperlinks retain records beyond the terminal's visible-content limits.

## Details

F42 — **definite** — `crates/farhelm-ui/assets/vendor/xterm.js:1; module 8811; bytes [280987,282063); R:7547–7586`;
`crates/farhelm-ui/assets/vendor/xterm.js:1, module 8811, bytes [280987,281221) and [281871,282062), readable lines 7547–7556 and 7584–7585`
— Unbounded hyperlink records can exhaust the viewer’s memory

The hyperlink service stores a fresh record and marker for every identifier-less opening and releases it only when
associated markers disappear. Empty hyperlink pairs can keep registering on a stationary row while writes complete and
scrollback stays fixed. The retention defect is established, but browser exhaustion and honest tmux's treatment of empty
spans were not tested. Surface it for proportionate remedy assessment, including unused-link reclamation or a
retained-metadata bound, without locally patching the vendor bundle.

## Evidence and triage context

- R:7545 owns the retained maps; R:7549–7556 adds a new record and marker on each id-less registration.
- R:7584–7585 deletes records only when all associated markers are removed.
- R:4334–4340 registers on opening and merely clears the current attribute on closing.
- R:5074–5082 associates marker cleanup with row trimming, insertion and deletion.
- crates/farhelm-ui/assets/terminal.js:4337 and :4343 allow completed writes to leave this state retained.
- crates/farhelm-ui/assets/vendor/xterm.js:1; representation [private review artifact] creates link metadata on nonempty
  OSC 8 URIs before any linked text is required; readable line 4339 closes only the current attributes.
- Readable line 7547 allocates an entry and marker for each id-less registration; readable line 7584 removes an entry
  only after its final marker is removed.
- Readable line 5074 ties markers to line trim, insertion and deletion; repeated opening and closing on a stationary
  line causes none of the relevant disposal events.
- Actual registerLink tokens retained 1,000 distinct records for the same URI on the same line.
- crates/farhelm-ui/assets/terminal.js:4337 writes received terminal bytes directly into this parser.

Retained confidence: **definite**. Suggested bucket: **highest**.

Possible cover:

- SPEC.md:2290–2295 conditionally accepts difficult-to-avoid availability degradation; mitigation difficulty has not
  been established here.
- TRIAGE_OUTCOMES.md:2359–2380 covers hover disclosure, not retention.
- TRIAGE_OUTCOMES.md:6061–6094 covers bounded frame queues, not link-service maps.
- SPEC.md:2290 accepts availability degradation only when it cannot easily be avoided; that remedy assessment remains
  open. Frame-budget triage and the accepted tmux-notification backlog cover different storage and lifetimes.

Caveats:

- The reported bounded reproduction was not independently rerun.
- OOM was not induced.
- Overlaps vendor_02_sec:p1:F9, whose primary anchor is the caller rather than this service.
- Definite confidence concerns unbounded retention, not an independently demonstrated crash.
- Browser OOM thresholds and throughput were not measured.
- An honest tmux's treatment of empty OSC 8 spans was not tested; a hostile supervisor can provide terminal data
  directly.
- The proof exercises actual registration tokens with a stationary-line marker fixture; no browser exhaustion was
  attempted.
- Full OSC parser dispatch and an honest tmux's empty-span output were not executed. The direct untrusted
  supervisor-to-viewer byte path is present.
- No OOM, shared-control outage, session death or work loss was demonstrated.
- A bound or reclamation fix may require upstream work; do not patch the vendor bundle or assume the specification
  requires elaborate DoS defenses.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `vendor_03_sec:p1:F1`, `vendor_03_cor:p1:F2`.

- `vendor_03_sec:p1:F1`: confidence as filed: definite; suggested bucket as filed: highest.
- `vendor_03_cor:p1:F2`: confidence as filed: definite for unbounded retention; suggested bucket as filed: highest.
