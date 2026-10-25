# Browse results can survive an automatic host fallback

Reviewed commit: 4ae55cc50dafcae6836abe835939ca79034cbdf7

## TLDR

On desktop, an old host's directory result can become an explicit folder after New falls back to the local host.
Frequency and the web route are unverified; final Launch still displays and checks its target.

## Details

Suggested bucket: **other**. Confidence: Source-only assessment; the established mechanism, contract ambiguity and
verification limits are preserved below. No reproduction was performed.

Source location: `crates/farhelm-ui/src/list/create_form/draft.rs:1811-1816`. Locations and revisions in the assessment
below identify the reviewed source; later main is outside this review's pinned range.

Reviewed commit 4ae55cc50dafcae6836abe835939ca79034cbdf7. TLDR: an old host's directory result can become an explicit
folder choice after the launcher has automatically switched to another host. Source-supported single-GUI trigger: New
follows selected remote installation A (no explicit host pin) and holds a completed A browse result. A hosts refresh
reports A in IdentityMismatch. matching_host_option rejects that state and effective_create_host falls back to local L
(shared.rs:506-560); create_form synchronously sets live_destination=L (2658-2665), but the parent create_target remains
A until its effect (view.rs:1298-1324), and live_browse_connection remains A's connection until a child effect
(2709-2714). Browse rendering and acceptance use those lagged signals instead of live_destination
(create_form.rs:1372-1402,3817,6797-6811; draft.rs:1811-1816). The first fallback render can therefore still show A's
result beneath the L destination. A desktop event in the render-to-effect interval passes browse admission, makes its
captured path explicit and clears remembered_destination (draft.rs:1826-1835), so subsequent L default seeding cannot
replace it (1461-1477). The event interval is source-supported by locked Dioxus desktop: webview.rs:562-588 returns
while sent edits await flush, and event handling at 78-99,101-191 dispatches directly through runtime.handle_event
without draining queued effects. Cached app.rs and webview.rs individually match archive members; desktop archive
checksum was verified against the reviewed lockfile. No frequency measurement or reproduction. An independent supported
input is selected-session removal via CLI, which clears current/open_destination (view.rs:1635-1640;
lib.rs:1867-1870,1897-1901) and can produce the same fallback; the automatic IdentityMismatch route needs neither a
second GUI nor CLI timing. The mechanism persists at pinned tip 463cac1528b5c5e259404975380d321a3fff0412:
create_form.rs:1439-1469,2946-2952 and draft.rs:1895-1933. Suggested bucket other. This establishes incorrect browse
target admission, not wrong-machine dispatch, security impact or user-work loss; the later visible Launch still shows
its submitted path and checks the final connection. Confidence confirmed static mechanism conditional on an event
arriving in the desktop edit-flush interval; web interval unverified. possible_cover none exact after six-source
searches. SPEC.md:838-854 and SPEC_impl.md:2077-2081 establish target-side bounded directory browsing; the code
explicitly promises current installation/connection at render and activation (create_form.rs:1366-1402). The One GUI
rule (SPEC.md:3403-3424) does not cover the automatic fallback route. Planned is empty; no matching BUGS/INDEX item;
ledger folder-picker-skips-symlinks and directory-browse-test portability entries concern different mechanisms. No
FILTER match is established: an accepted selection changes draft state and can survive subsequent effects rather than
only showing a transient wrong label; no action on user-owned work is claimed. proposed_drop none. This is distinct from
untagged history response provenance, canonical history and visible batch partial replay.

Provenance: `a030/d2`, `g242/f4`. These are reviewer-local identities, not additional findings. Proposed drop: none for
the retained remainder.
