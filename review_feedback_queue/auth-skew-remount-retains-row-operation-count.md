# An auth/skew remount retains abandoned row operations

Reviewed commit: 7453218d831563b22b2b18a4a528b1a23f185bd7

## TLDR

After another tab restores shared credentials, a build-skew remount can leave the old GUI usable but keep an abandoned
mutation count that blocks Restart. The conditional route needs an outstanding mutation, authentication loss, build
change and another GUI.

## Details

Suggested bucket: **other**. Confidence: Source-only assessment; the established mechanism, contract ambiguity and
verification limits are preserved below. No reproduction was performed.

Source location: `crates/farhelm-ui/src/list/view.rs:786`. Locations and revisions in the assessment below identify the
reviewed source; later main is outside this review's pinned range.

Reviewed commit: 7453218d831563b22b2b18a4a528b1a23f185bd7; preserved at pinned tip
463cac1528b5c5e259404975380d321a3fff0412. TLDR: A stale browser page can keep Restart and related controls disabled
after another tab has restored usable authentication. Static conditional sequence: (1) Tab A has a row mutation awaiting
completion, with row_ops positive. A separate same-build recognized 401 opens TokenPrompt and unmounts the list,
abandoning its cleanup task while AppBody-owned row_ops/deleting survive. (2) The helm changes build. (3) A freshly
loaded same-origin Tab B uses the matching bundle to sign in. auth.rs:364-371,408-416,461-476 writes the new secret to
the shared origin's localStorage and reloads only B's own window. (4) A's already-open TokenPrompt submits the current
token. api.rs:2097-2099 latches the new build and returns before reading/storing its exchange secret. lib.rs:1774-1783
now replaces the prompt with PreferencesGate even though TOKEN_REQUIRED remains true. A has not successfully
exchanged/reloaded, but its ordinary requests read B's stored secret at send time (api.rs:2021-2024), so its remounted
list/detail can load. (5) A's phantom count still makes PaneGate refuse every claim (ops.rs:177-181,201-208),
independent of now-valid authentication. A reload clears it; ordinary new row operations cannot, because their new
row_pending set balances only their own increments. The assigned deletion is list/view.rs:786, removing mount reset
hooks; old count updates remain at 856-870 and delete awaits/cleanup at 2165-2197. Source at tip has the same lifetime
mismatch at lib.rs:1702-1711,1775-1784; list/view.rs:818-819,863,888-902,2209-2241. Explicit reads are allowed under
skew (reader.rs:232-235,475-483,639-643; list/view.rs:1923-1933), the REST funnel does not block successful skewed
requests (api.rs:1849-1909), and Restart is specifically disabled/refused by lifecycle.busy/claim_guard
(session_view.rs:2103-2117). Suggested bucket: OTHER. Confidence: confirmed conditional source mechanism with a complete
shipped-control route; no runtime scheduling/frequency claim. Caveats: this requires overlapping outstanding mutation,
sign-in expiry and build change, plus a second same-origin tab. If credentials never recover, the page is already unable
to authenticate; that simpler sequence is not independent evidence of added operation refusal. Ordinary successful
same-build sign-in still reloads and is unaffected. No user-work loss, security consequence or wrong-target operation
established. possible_cover: pinned SPEC.md:3383-3401 accepts discarded browser state, pending reports and token
friction; 3405-3416 makes concurrent GUIs best effort. Whether this also accepts stale admission state after
authentication recovery is ambiguous against SPEC_impl.md:5073-5077's explicit-operation promise. proposed_drop: none;
retain conditional OTHER for parent assessment.

Provenance: `g271/f1`. These are reviewer-local identities, not additional findings. Proposed drop: none for the
retained remainder.
