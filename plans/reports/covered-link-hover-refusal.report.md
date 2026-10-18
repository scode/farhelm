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

### Landing

Landed on its own in round 88, as #2254, with one fix made during the landing (below).

#### What landed since the PR was based, and whether it interacted

Only plan-queue bookkeeping reached main between this PR's base and its landing, and no other plan landed in this round.
The landing rebase was clean. The landing monitor read the change against what main already holds, and a fresh-context
reviewer did the same independently. Neither found an interaction with the localhost tunnels, the earlier terminal link
safety rules, the session action shortcuts, the menu focus fix, naming after quick launch or the one-way YOLO shield. In
particular, both the plain open and the Cmd-click tunnel open are checked after the existing link-ignoring rules, the
tunnel hint is rechecked at click time, and a refused tunnel never falls back to opening the raw link.

The clipped-file refusal this change extends shipped in v0.29.0, so the changelog fragment is a new "fixed" entry rather
than an edit of an earlier one.

#### Fixes made during the landing

The reviewer found a regression. When a program prints a file link as a hyperlink (rather than as plain text), the
terminal treats a redraw of that row as the pointer leaving the link and coming straight back. Before this PR, leaving
such a link went through a short delay, and the immediate return cancelled it, so a refusal explanation stayed raised
through a redraw; v0.29.0's clipped-file refusal depends on that. This PR also lowered the explanation immediately on
leave, so any redraw of the row would have dropped it beneath the hover and the approval cards. The landing restored the
delayed path for file links, with a code comment saying why. No test redraws a hyperlinked file's row after a refusal;
the fix rests on reading the code, and the checks below show it broke nothing else.

#### Possible follow-ups

- Web links have no such delay yet: if a refused web link's row is redrawn while the pointer stays on it, the
  explanation drops to its ordinary layer, where an approval card can cover it. The click is still refused and nothing
  opens. Fixing it needs the web-link hover to recognize its own return after a redraw, as file links do, plus a browser
  test that redraws the row; the landing left it alone because that is new behavior to design, not a regression.
- While the pointer stays on a refused link, the raised explanation spans the top of the terminal and covers the top of
  an approval card, where the card names the requesting session and host. It goes away as soon as the pointer leaves the
  link. Triage asked for the explanation to sit above the cards, so this is the expected trade-off; the case that
  remains is answering the card from the keyboard with the pointer still on the link.
- SPEC.md says a hover is not visible when "another Farhelm surface such as an approval card" covers it, but the code
  checks only the approval-card area. That covers the realistic case today. The other surfaces that can sit above a
  hover are full-window dialogs, plus a few small, short-lived ones (hover help, the quick-launch echo, a tunnel error
  notice) that the check ignores. The report and triage record the narrower check, but SPEC_impl.md does not, so a
  future surface that floats over the terminal would not be checked unless someone adds it.
- Another plan in flight, session-details-bottom (#2253), moves the approval cards to start just below the top of the
  terminal. It merges cleanly with this change, but it means the cards will cover link hovers near the top more often
  and the raised explanation will always overlap the card's top. The browser tests added here measure the card's live
  position, so they should adapt; that plan's landing should re-run them.

#### Checks

- Run now, on the rebased PR with the landing fix: the UI JavaScript harness through the test recorder (`node --test`,
  294 passed, run `001bee8a-6a7c-476b-8208-a31d8fa6e42d`), then fresh `cargo build` and
  `dx build --package farhelm-ui --platform web --release`, then
  `npx playwright test terminal-link-clicks.spec.ts terminal-links.spec.ts approval-layout.spec.ts` through the recorder
  with pinned tmux, one browser worker and no retries: 86 passed (43 in Chromium, 43 in WebKit), run
  `9f6dd468-f8b9-44f9-bd9c-5015f1a5989d`. These are the same three browser files the executor ran, and they include
  every new case.
- Reused: the executor's formatting, changelog, instruction and test-sleep checks, which the landing fix does not
  affect. Its browser and JavaScript runs are superseded by the ones above.
- Skipped: Rust tests, lints and desktop compilation, because no Rust changed; the desktop app's own webview, native
  file opening and tunnel paths, as in the report.

Apart from the landing fix, nothing in the executor's report became untrue at landing.
