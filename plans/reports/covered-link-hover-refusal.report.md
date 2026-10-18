## What this was about

An approval request could cover the destination shown when hovering a terminal link while leaving the link itself
clickable beside the card. A click could therefore download a file or open a web page without the user being able to
read its destination. During triage, you chose to refuse covered disclosures and state that rule in the spec, then
extended the decision to web links.

File links and web links that show a destination hover now refuse clicks while an approval card covers any part of that
hover. Web links with a hover also refuse when it is cut off by the window. The existing terminal status line explains
the refusal above the cards. Moving to a fully visible hover restores the usual action.

## Things you should know

The rule includes the desktop app's modifier-click file opening and Cmd-click tunnel opening paths. Plain URLs printed
as text still need no hover unless the desktop's remote-localhost tunnel hint applies at click time. Their ordinary
opening behavior is preserved. No hover or approval card was moved to another layer; only the refusal explanation was
raised above the cards.

Chromium and WebKit exercise the shared browser behavior. The desktop app's own webview, native system opening and
native tunnel IPC were not exercised. WebKit stands in for the desktop engine family, rather than proving those native
operations.

## Open questions and possible follow-ups

None required for this plan. The check is deliberately limited to the visible approval-card region, as agreed during
triage; it does not add a general detector for arbitrary covering elements.

## The PRs

- [#2254 — refuse terminal links with hidden destination details](https://github.com/scode/farhelm/pull/2254/changes),
  draft, head `e6bfc9190d6d1dc3c672c6f74391dfadab6ddd6d`.

## Checks run, reused and skipped

- Run: `node --test` in the UI asset-JavaScript harness, 294 passed with no skips; recorder
  `6c1c8428-cc21-48fe-8610-886935487a5e`. This includes the shared geometry, deferred refusal lifetime and existing
  tunnel dispatch tests.
- Run: `npx playwright test terminal-link-clicks.spec.ts terminal-links.spec.ts approval-layout.spec.ts`, 86 passed (43
  Chromium, 43 WebKit), no skips or retries, one browser worker, pinned tmux 3.7c; recorder
  `27ea5264-1181-435f-a8e7-6f6260990df4`. The strict recorder retained complete reports for both engines. This covers
  covered file plain/modifier refusals with no transfer, a real uncovered download, covered and clipped web refusals,
  uncovered web opening, the no-hover plain-URL exception, and existing link/approval regressions.
- Run: the first six new browser cases produced five passes and one WebKit fixture failure, recorder
  `618cd6e1-8f3b-4ef0-8d3e-1f62ffb431a1`. The fixture's plain URL was too long for the space left of the card; its
  geometry assertion stopped before that click. Shortening that literal preserved the intended coverage premise. The
  exact affected case then passed on both engines, recorder `f5df33c0-5e2a-4e61-a7bc-254358d25cf4`. The original failure
  evidence is retained privately.
- Run: `cargo build` and `dx build --package farhelm-ui --platform web --release` prepared the real test application.
  Changed-file `dprint check`, `python3 releasing/check-changelog.py format` and `python3 scripts/check-agents-md.py`
  passed. The pinned-parser `python -B scripts/check-test-sleeps.py` inspected 303 delays with none lacking a rationale.
- Reused: the successful builds and JavaScript results cover the final product code; the later amendment only added the
  PR link to the triage ledger. Main remained at the plan's base `3df5399b`, so no rebase was needed. No previous plan's
  runtime result was reused.
- Skipped: Rust tests, Rust lints and desktop compilation because no Rust changed; desktop asset parity because no asset
  was added; the full browser suite because the three selected files cover link activation and approval layout.
  `npx tsc --noEmit` could not type-check because this test package has no TypeScript compiler dependency. Browser
  execution provides the test code's integration evidence.

## Review gate's outcome

The required fresh-context gpt-6.1-sol reviewer at high effort found no correctness, design, idiomatic-code or
test-contract findings. Its follow-up accepted the shorter fixture URL. The source review included the full test
authoring checklist. The commit message and PR wording passed the separate cold read. The feedback file and index entry
are removed, and the retained triage outcome names the change, bookmark and PR.
