# The held-font regression lacks a mount-attempt witness

Reviewed commit: d70f87c633b35bdac0d7ab4bc3000b7ca4e0bc65

## TLDR

The font test can finish its no-mount observation window before a terminal first tries mounting. Ordinary page text can
request the font first, so later terminal appearance does not prove the test would catch premature mounting.

## Details

Suggested bucket: **other**. Confidence: Source-only assessment; the established mechanism, contract ambiguity and
verification limits are preserved below. No reproduction was performed.

Source location: `e2e/tests/terminal-font.spec.ts:217-252`. Locations and revisions in the assessment below identify the
reviewed source; later main is outside this review's pinned range.

Reviewed commit: d70f87c633b35bdac0d7ab4bc3000b7ca4e0bc65; relevant test, terminal script and session-view caller
unchanged through pinned tip 463cac1528b5c5e259404975380d321a3fff0412. TLDR: the held-font regression can miss a broken
mount gate when terminal startup occurs after its negative observation. At e2e/tests/terminal-font.spec.ts:217-230 the
test acknowledges ordinary app.css text may issue both weight requests before a terminal mount. Releasing regular and
confirming it loaded (232-249) does not establish a terminal mount attempt. The four-second window (251-262) has no
positive attempt witness, and readiness/real output are required only after bold release (264-273). The actual caller
independently awaits mint_lease (crates/farhelm-ui/src/session_view.rs:659-664,738-746); terminal_specs returns None
while that lease is absent (1578-1580), and the effect does no sync until specs exist (1707-1712). It then separately
polls for asynchronously injected terminal.js before sync (1720-1729; documented 385-395). sync calls mountWhenReady
only later (crates/farhelm-ui/assets/terminal.js:2904-2973). No awaited test condition couples those prerequisites to
the observation window. If the first mount attempt occurs after the negative assertions and bold release, correct and
regressed gates can yield the same passing result; startup merely occurring somewhere inside the four-second window is
not enough to establish that specific false-pass schedule. Suggested bucket: other, a maintainer test-oracle defect.
Confidence: confirmed missing fixture witness and independent asynchronous producer ordering; the false-pass schedule is
conditional and unobserved, with no measured runtime frequency. Current production correctly requires both weights
(terminal.js:438-476,3208-3243); no production failure, security impact or user-work loss is claimed. possible_cover: no
matching acceptance found. SPEC_impl.md:1085-1090 accepts the production gate, not this missing test premise;
TRIAGE_OUTCOMES.md:5965-5978 and 13085-13094 concern font retention/fallback and mutable fontFamily observation.
proposed_drop: none. This reverses the initial pass's dismissal as an arbitrary delayed-script schedule: the committed
caller has real independent lease/script waits that the test does not observe. Reconcile as the current test remainder
of g122/f1, whose historical plan attribution remains
e6da5fd8b0041b1554c0407acffa5d44074c2349:plans/queue/terminal-font-required.md:130-134; do not count this shared
mechanism twice. g315/g316's production-gate conclusions remain intact. The rejected-font test is distinct: its Farhelm
gate error supplies its own attempt witness and cannot establish this held test's premise. A gate-attempt observation
before starting the timed window would supply the missing boundary.

Provenance: `g317/f1`, `g122/f1`, `g315/f1`, `g316/f1`. These are reviewer-local identities, not additional findings.
Proposed drop: none for the retained remainder.
