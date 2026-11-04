# Repeated grid typing test accepts an earlier invocation's echo

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

Repeating the dedicated host-terminal typing test can report success from an earlier test's screen without proving that
this invocation delivered its typing.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed static oracle
weakness with source-established repeat/shared-stack/replay path; no runtime experiment.

Source locations: 5e467f4d64f8c815173ce95cb6d180e83aa67704:e2e/host-terminals/grid-terminal.spec.ts:72-77; unchanged at
ca9b0a19856dc136f63ad2d14f1076027edce1eb.

Repeating the dedicated host-terminal typing test can report success from an earlier test's screen without proving that
this invocation delivered its typing. Exact source basis at tip: grid-terminal.spec.ts:74 names its input only by
testInfo.project.name, then :77 accepts containment of its echo. Its opener (:31-55) reattaches the shared e2e-session
and checks ready/grid/focus, without resetting its screen or establishing absence of the expected echo.
host-terminals/playwright.config.ts:16-18,37-80 boots one shared stack with one worker and no per-test reset hook.
Official Playwright1.64.0, immutable upstream commit606ecd8b34bac100db32561b6278ffd76e5eb3cd,
runner/loadUtils.ts:262-278 builds repeat-each suites by cloning the file suite under the same project;
runner/testRunner.ts:422-451 performs one global/plugin setup before loading/running those suites. Thus invoking the
same file with repeat-each permits a correct first iteration to leave this project's exact echo on the same live
terminal for its next iteration; no worker restart creates a new stack. fake_agent.rs:1675-1684 starts a small primary
screen and :1762-1764 appends its echo/prompt without clearing it. GridStream::catch_up at
service/grid_stream.rs:420-460 sends the current emulator screen on the next attachment.
grid-terminal.js:609-615,684-744 paints those existing row runs into DOM text; its input path:1408-1413 scrolls and
clears selection, not old screen output. With the early sparse screen, an earlier matching echo remains visible. A later
invocation with working output/replay but lost outbound input can satisfy :77 from that already-drawn echo. This
establishes the test oracle's inability to distinguish delivery now from delivery earlier; no current product input-loss
defect or measured false-pass frequency is claimed.

Possible coverage and caveats: none exact found. Candidate coverage searched SPEC.md, SPEC_impl.md, TODO Planned,
unlanded plans/queue, BUGS.md, queue INDEX/files and TRIAGE_OUTCOMES using exact file/marker plus old-output, echo and
repeat terms. TODO Planned is empty (tip23-25); SPEC_impl.md Grid client1706-1735 and Testing6742-6808 specify
replay/render/testing behavior, not accepting stale evidence. .agents/validation.md:632-645 explicitly describes this
test as checking the agent's echo of the typed line. FILTER.md Rare self-correcting glitches excludes success reported
for a failed operation and in any event rarity is not established. Searches of current sibling finding logs found no
same file/marker/echo-repetition finding; g107's earlier refused-input bytes, g153/g190 prompt readiness, and g193's
negative old-history-delivery assertion are different mechanisms.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g118/f2`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is 5e467f4d64f8c815173ce95cb6d180e83aa67704.
Persistence verified at ca9b0a19856dc136f63ad2d14f1076027edce1eb, grid-terminal.spec.ts:72-77.. This was a source-only
review; runtime occurrence and frequency were not measured.
