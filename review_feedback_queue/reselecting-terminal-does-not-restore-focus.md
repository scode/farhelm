# Reselecting a revealed terminal leaves focus on its selector

Reviewed commit: 4ba9bf48d9f42c1ab4218ff56dd3806b04bc47fb

## TLDR

Keyboard activation of an already-selected session row or tab leaves typing focus on that button rather than returning
it to the revealed terminal. The exact reselection policy is unspecified; no harmful input route is alleged.

## Details

Suggested bucket: **other**. Confidence: Source-only assessment; the established mechanism, contract ambiguity and
verification limits are preserved below. No reproduction was performed.

Source location:
`crates/farhelm-ui/assets/terminal.js:1179-1183,3101-3106,4499-4500; crates/farhelm-ui/src/list/view.rs:2928-2945; crates/farhelm-ui/src/tabs.rs:283-289`.
Locations and revisions in the assessment below identify the reviewed source; later main is outside this review's pinned
range.

New independent discovery at reviewed commit 4ba9bf48d9f42c1ab4218ff56dd3806b04bc47fb. TLDR: selecting the session or
terminal tab already on screen does not return typing to its terminal once it has finished loading; focus stays on the
selector until another focus action. The click listener only writes focusOrigin, sidebar guarded_open only persists
selection and calls the root on_open, and root lib.rs:1860-1863 writes the same keyed session. session_view.rs:1197-1239
follows same-session writes without remounting; the tab selectors merely set the same selected id at 2249-2255,2266-2274
(tabs.rs:283-289). Stable specs do not change focusedEl, so sync has no focus handoff at terminal.js:3101-3106, and
reveal is one-shot at 4499-4500. Keyboard activation on a focused selector establishes the button-focus premise without
relying on a browser pointer-focus policy; no held key or subsecond race needed. Confirmed conditional source behavior
for an already-revealed stable island. Existing helper e2e/tests/helpers/terminal-suite.ts:500-508 explicitly works
around current behavior by selecting a different terminal first; that acknowledgement is not a specification acceptance.
OTHER: convenience/typing-focus failure; no highest route or loss of work alleged. No runtime reproduction or frequency
measurements. Remains at pinned tip 463cac1528b5c5e259404975380d321a3fff0412: terminal.js:976-980,2896-2901,4192-4193;
session_view.rs:2015-2020,2031-2039; tabs.rs:283-289; list/view.rs:2966-2982; lib.rs:1861-1864. Six coverage sources
searched. No exact accepted behavior, Planned item, BUGS entry, queue duplicate or ledger decision found.
SPEC_impl.md:1100-1104 addresses reveal only; ledger:18680-18684 states select-then-type motivation but does not
expressly address already-revealed reselection. SPEC.md:1504-1509 competing-input timing rule has a different trigger.
No rare-glitch or subsecond filter established for stable repeat activation. possible_cover: explicit already-selected
focus policy, if supplied; none found. proposed_drop false. This is distinct from focus being stolen during initial sync
or automatic tab fallback, because here explicit selection fails to restore terminal focus.

Provenance: `a066/discovery-reselect-focus`. These are reviewer-local identities, not additional findings. Proposed
drop: none for the retained remainder.
