# Parity: Launcher

Old Playwright specs (reference commit `24bc0580`; read one with `git show 24bc0580:e2e/<path>`) and what replaces them
in the GPUI app. Port behavior from SPEC.md and these specs' acceptance cases, not one spec file at a time. SPEC.md
sections: Creation, Sandbox sessions, Managed checkouts, Container sessions, Lifecycle operations (Restart with and
Replace with).

The Replace with cases of `tests/replace.spec.ts` and `tests/terminal-restart.spec.ts` are this area's too, although
those specs' rows sit in `sidebar-and-session-view.md` with the rest of their tests.

| Old spec                                    | Tests | Covers                                                                                                                                          | Status         |
| ------------------------------------------- | ----- | ----------------------------------------------------------------------------------------------------------------------------------------------- | -------------- |
| `tests/launcher-enter.spec.ts`              | 25    | Clone search Enter picks an unmatched name before a second Enter launches                                                                       | not yet ported |
| `tests/launcher-tabs.spec.ts`               | 6     | New opens on the agent tab, and each tab keeps its own fields and draft                                                                         | not yet ported |
| `tests/launch-button-alignment.spec.ts`     | 1     | launcher summary and actions stay pinned at ${viewport.width}x${viewport.height}                                                                | not yet ported |
| `tests/launch-progress.spec.ts`             | 5     | launcher status waits two seconds, counts up and ends on a real refusal                                                                         | not yet ported |
| `tests/launch-refusal.spec.ts`              | 7     | a quoted command placeholder is refused by native admission                                                                                     | not yet ported |
| `tests/cursor-composer.spec.ts`             | 1     | Cursor launches without claiming session tracking                                                                                               | not yet ported |
| `tests/goose-pi-composer.spec.ts`           | 2     | Goose and Pi permission transitions update the rendered draft                                                                                   | not yet ported |
| `tests/grok-composer.spec.ts`               | 1     | the Grok composer clears and hides unsupported model and effort choices                                                                         | not yet ported |
| `tests/omp-composer.spec.ts`                | 2     | the OMP composer offers OMP's own vocabulary and the row shows the effective permission                                                         | not yet ported |
| `tests/opencode.spec.ts`                    | 2     | OpenCode keeps composer controls while offering default and explicit Zen models                                                                 | not yet ported |
| `tests/github-checkout-composer.spec.ts`    | 18    | managed checkout offers no retired container controls                                                                                           | not yet ported |
| `tests/composer-word-search.spec.ts`        | 6     | launcher refusals use warning colors in ${appearance} appearance                                                                                | not yet ported |
| `tests/custom-permissions.spec.ts`          | 9     | custom permission search focuses literal words and switching agents clears them                                                                 | not yet ported |
| `tests/guarded-permissions.spec.ts`         | 2     | guarded choices explain each agent and Cursor keeps its own setting                                                                             | not yet ported |
| `tests/permission-switch.spec.ts`           | 7     | ${query} selects YOLO before choosing a harness                                                                                                 | not yet ported |
| `tests/remembered-permissions.spec.ts`      | 3     | the remembered structured-launch permissions mode survives an open, a reset, and a later default launch                                         | not yet ported |
| `tests/model-defaults.spec.ts`              | 1     | ${label} can launch with its configured model                                                                                                   | not yet ported |
| `tests/named-conversations.spec.ts`         | 3     | initial prompt launcher follows capabilities and binds literal request retries                                                                  | not yet ported |
| `tests/nonblocking-launch.spec.ts`          | 15    | a pending New keeps one immutable draft while sidebar, switcher and terminal work                                                               | not yet ported |
| `tests/yolo-guard.spec.ts`                  | 13    | a YOLO launch asks first and starts only when confirmed${height === 844 ? "" : ` at 390x${height}`}                                             | not yet ported |
| `tests/clone.spec.ts`                       | 13    | a structured GUI clone pre-fills without launching, then starts its ready successor                                                             | not yet ported |
| `tests/destination-authority.spec.ts`       | 8     | New inherits a filtered selected destination with replacement=${replace}                                                                        | not yet ported |
| `tests/podman-environments.spec.ts`         | 8     | active placement segments preserve explicit host recipe and place                                                                               | not yet ported |
| `tests/tensorlake-sandbox-sessions.spec.ts` | 16    | recipe placement explains prerequisites and requires billing consent                                                                            | not yet ported |
| `tests/github-checkouts.spec.ts`            | 18    | quick launch waits for a managed preview and ${collide ? "reveals its path race" : "starts directly"}${base ? " for a template base name" : ""} | not yet ported |
| `tests/claude-trust.spec.ts`                | 4     | Claude trust hint preserves the terminal and opens the opt-in caveats                                                                           | not yet ported |
| `tests/terminal-create-idempotency.spec.ts` | 9     | a null-permission ${harness} template retains consent and one session after a lost reply                                                        | not yet ported |
| `tests/restart-with.spec.ts`                | 42    | named restart with requires a target when no conversation was captured                                                                          | not yet ported |
