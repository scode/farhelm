# Download tests acquire sessions before cleanup ownership

Reviewed commit: a5f5f8213eeb0a1a5161c22cf6260501aca81be9

## TLDR

A failed test setup can leave its agent session running and listed for subsequent browser tests.

## Details

Suggested bucket: `other`. Confidence: confirmed failure path by code inspection; setup rejection was not injected.
Source: dedicated `gpt-6.1-sol` high adversarial review. Proposed drop: none.

In e2e/tests/terminal-files.spec.ts:73–85 and :211–225, createSession succeeds before an awaited page.route registration
outside try/finally. The refusal test likewise creates its session at :154–155, then awaits addInitScript outside the
cleanup guard at :156–157. If those setup awaits reject or time out, cleanupSession is never called. The evidence
fixture owns browser observation, not API-created sessions (helpers/evidence.ts:31–65); createSession also installs no
cleanup owner (helpers/fleet.ts:371–398). Move subsequent setup inside the cleanup guard immediately after acquiring the
session. Scope is test-owned state during the shared run: start-stack.sh:242–265 eventually kills its tmux servers and
removes scratch, so this is not an indefinite machine-wide leak.

Possible cover: Existing header-regression-fixtures-escape-cleanup.md, tab-retarget-fixture-escapes-cleanup.md and
template-regression-fixtures-arm-cleanup-too-late.md concern different tests and fixtures.

Current-main status: Persists at pinned main: corresponding create/setup/try boundaries remain at :249–261, :330–333 and
:387–401.

The full commit diff and relevant paths were read. No runtime reproduction was run.

## Additional affected scope

Reviewed commit: 2e6fe8d09b747faec24382d95852bfb5d5688c90

## TLDR

A browser setup failure can leave test sessions running and affect later tests sharing the stack.

## Details

Suggested bucket: `other`. Confidence: confirmed by control flow; failure not injected. Source: dedicated `gpt-6-astra`
high adversarial review. Proposed drop: none.

e2e/tests/terminal-files.spec.ts:109–123 and 187–203 create real sessions, then await page.route before entering the
cleanup-owning try/finally. If route registration rejects, including when the browser closes or the test times out,
cleanupSession is never called. Both shells can remain blocked indefinitely in their initial read.
helpers/fleet.ts:371–399 creates the session without registering independent cleanup. Move route setup inside a cleanup
scope armed immediately after successful creation. This affects the four redraw cases and the OSC-label replacement
case; it is a test-isolation defect, not a shipped hover regression.

Possible cover: TRIAGE_OUTCOMES.md headings history-fixture-enables-yolo-installing-restoration-guard.md and
keyboard-launch-fixture-has-same-separate-setup-ownership.md address analogous failures in sidebar fixtures, not these
sessions. Stack-wide cleanup eventually bounds persistence but does not prevent contamination during the run.

Status at frozen endpoint: Persists at 4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703: terminal-files.spec.ts is unchanged
from the reviewed commit. No fixing commit in the frozen range.

Review coverage: full commit diff read: true. No commit-diff content remains unread. An oversized combined tool response
was truncated; required product code and policy content was subsequently reopened in bounded calls. The entire
13,089-line triage ledger was not read linearly; coverage was targeted searches and the cited entries. Unchanged
portions of large callers, tests and the minified vendor bundle outside the cited excerpts were not exhaustively read.
No Sol reports or new working-copy queue finding contents were read.

Runtime evidence: None, as requested by default. Findings are static review conclusions. No checkout files were created,
modified or deleted; no fetch, rebase or helper agents.
