# Combined file admission can publish an obsolete joined path

Reviewed commit: acaaa03556ea27013f3c643266c6e60a82be8691

## TLDR

Unverified after investigation: a container terminal can offer an obsolete joined filename if neighboring output changes
while a separate hyperlink lookup is pending. The download path independently checks recognition and refuses the changed
target.

## Details

Suggested bucket: **other**. Recorder confidence: **unverified after investigation**. The original reviewer's
confidence, qualifications and severity history are retained below; source-confirmed conditional behavior is not a
runtime observation.

The existing container-redraw-discard item concerns losing a valid hover during replacement admission, not publishing an
already-invalid retained result. Independent consultation agrees OTHER for the established stale-hover/refusal
consequence; a proven unauthorized transfer would require HIGHEST. The duration and recovery of the stale display have
not been measured, so a complete rare-glitch filter match is not established.

## Recorded source assessment

Reviewed acaaa03556ea27013f3c643266c6e60a82be8691; recording-only clarification, no new source investigation or runtime
action. TLDR: the combined container link provider has a source-established final-publication freshness gap; a resulting
obsolete joined-filename hover and click refusal remain unverified after investigation. Keep those two claims distinct.
Exact source basis: terminal.js:1372-1387 retains admittedPaths and admittedOsc independently and publishes both from
finish without re-recognizing retained paths. terminal-files.js:395-398 validates prose signatures only when that batch
completes. grid-terminal.js:1364-1374 final callback checks generic answerLines identities; grid-buffer.js:477-495,
:534-562 captures soft-wrap/URL/OSC dependencies, which for an unwrapped row and separate one-row OSC tag can be only
abs-1 through abs+1. terminal-files.js:218-228 reads cuts beginning abs-2 through abs+1, and a cut beginning abs+1
depends on abs+2. Thus a prose result can pass its own validation, wait for OSC, and become obsolete when abs+2 creates
a consecutive cut, without changing the generic final callback dependencies. grid-terminal.js:1403-1412 captures
per-link evidence only at setHover, after this change; terminal-files.js:482-520 hover requests/displays the retained
path without pathCurrent. receive at grid-terminal.js:1966-1992 drops rowCache and schedules paint without incrementing
the pending lookup token; noteModelChange:1318-1320 only clears rowCache. Source checks actually performed at the
reviewed commit: terminal.js 1355-1395; grid-buffer.js 100-205,355-415,425-454,465-500,525-572; grid-terminal.js
1335-1385,1885-1910,1950-1995; terminal-files.js 150-230,350-410,470-545,565-700; SPEC_impl.md 3780-3838. Actual source
results: no exact final-publication full hard-cut freshness check found; one-row taggedLines boundaries and nonwrapped
urlWindow do not include abs+2; no pending-lookup invalidation on an unrelated row receive; download does independently
compare current display and path/range recognition. Contract premise: SPEC_impl.md Session-host file reads at reviewed
:3785-3793 excludes consecutive possible hard cuts, :3795-3799 requires changed neighbor recognition to leave/requery,
and container-provider paragraph :3826-3834 requires rejecting changed buffer evidence before publishing admitted links
(tip corresponding paragraph :3832-3840). Likely source route: request a nonwrapped head row carrying a printed
head/tail cut and a separate one-row OSC file link; complete prose admission while holding OSC stat; change abs+2 so the
tail starts a second possible cut; release OSC. Source admits final publication of the old prose result. Exact missing
observation: an owned static-grid/browser fixture must demonstrate that distinct OSC/prose stat replies can be ordered
this way, that the abs+2 row update is applied between prose completion and combined publication, and that the resulting
offered hover actually displays the old joined target before an attempted click is refused. No such native/static-grid
observation, browser schedule, frequency or runtime pass was executed. The brief explicitly prohibits runtime, process,
network and exploit actions and limits this review to source, so that observation cannot be performed within this
assignment. Settling evidence: a controlled static-grid test with separate delayed stat responses, an asserted applied
abs+2 update, tooltip/path inspection after OSC release, and a click asserting no download; run under the owned browser
harness when authorized outside this source-only review. Suggested bucket OTHER for the retained stale-hover/refusal
scope: independent check12 clarification agrees this narrow consequence because terminal-files.js:667-676 independently
refuses changed recognition before download/open. This is not a blanket severity reduction: unauthorized transfer
remains hypothetical and would require HIGHEST if established; no such route is claimed or dropped here. Confidence
unverified after investigation for observed stale hover, likely source route; confirmed static absence of the final
guard. Proposed repair: revalidate all retained path signatures at combined finish or capture the complete hard-cut
evidence before initial lookup and check it at final publication. proposed_drop none. possible_cover none beyond the
stated spec contract; parent handles queue coverage. Original l3 lead, withdrawals, proposed drops and reopening remain
preserved append-only; this event supersedes the overly broad earlier refutation without erasing it.

Provenance: trailing-review run `f57167f6-e8b1-4ac6-ac31-413e21137114`, original record `g013/l3`, exclusive base
`49ce8cb76e9f4c841e15122a89264c7c908171dc`, inclusive tip `9e3ab60b86025ca74948ededd09b99cd81a2477c`. The private
findings history, dispositions, delivery audits and independent verdicts preserve revisions and narrow exclusions. No
fix or triage decision was executed.
