## What this was about

A tunnel-error notice in the desktop app could cover a terminal file or web link's destination while leaving the link
clickable beside it. The maintainer asked for those clicks to refuse, and for the refusal to stay readable if that
remained a small fix. A separate decision accepts a link-refusal message covering the top of an approval card while the
pointer stays on the link. Finally, a desktop Cmd-click could open a file after another window enabled its type even
though its hover had offered only a download; the maintainer chose a small fix for that timing gap.

## Things you should know

File and disclosed web links now check the live tunnel-error notice as well as approval cards before acting. A covered
destination refuses transfer or opening, with wording that names a covering Farhelm surface. The tunnel notice sits
below that refusal, so the explanation remains readable even when they overlap. Both changes reuse the existing
rectangle check and refusal layer; the agreed complexity gate was met.

The specification now explicitly accepts refusal messages covering an approval card's heading, requesting session and
host while the pointer stays on a refused link. The card remains answerable from the keyboard; leaving the link
withdraws that raised cover. This PR changes specification text only.

A desktop modified click now requests opening only when that hover offered it. Otherwise the file is still saved, and
its result says that opening was not offered. A missing opening hint also stays save-only. The helm still checks the
current allowed file types when opening, so an earlier offer cannot override a later refusal. This fix also met its
small, localized complexity gate.

The browser regressions inject the desktop notice's actual markup and production CSS over real terminal hovers. They
prove clickable-pointer geometry, no transfer or opening on refusal, readable refusal layering and recovery after
removal. They do not exercise the native desktop shell or physical Mac input. All three original feedback items were
removed and their execution records now point to the corresponding PRs.

## Open questions and possible follow-ups

None requires a maintainer decision. Native desktop notice rendering was not separately exercised; Chromium and WebKit
cover the shared rendering and event path with the notice injected.

## The PRs

- [#2348 — refuse links hidden by tunnel notices](https://github.com/scode/farhelm/pull/2348/changes), head
  `4aebbc47d340866819a8a4cca05120a21974dacb`.
- [#2350 — allow link error banners to cover approval cards](https://github.com/scode/farhelm/pull/2350/changes), head
  `2b8514a70459b6bb66eee8af2ed06a9e1219ed64`.
- [#2351 — honor file hover opening hints](https://github.com/scode/farhelm/pull/2351/changes), head
  `bd76a5b9c7de5f166a446eb3b8ae5e96660fffb8`.

All are open drafts in one linear stack. A final fetch found main unchanged from the stack base,
`a5193424ba610f9adebdeec0e59258eb6f5716f6`, so no rebase was needed.

## Checks run, reused and skipped

- PR1: `node --test` in the UI asset test directory passed 310 tests, run `137c7b2f-b6a5-4042-9f2d-d6f6461bc605`, at
  tested working revision `78892422bd38ff5bbb09e163a7e69c1f6bf10696`, before the browser-fixture geometry correction and
  execution URL. Later runtime code changes are covered by the addon command below.
- PR1 browser gate: `npx playwright test terminal-link-clicks.spec.ts -g 'a tunnel notice covering'` passed four cases,
  two each on Chromium and WebKit, run `7ff7de9e-3bda-410e-b1df-803415c6925e`, at tested working revision
  `36fea52484a98244f16133fea2644ce327be6262`. This successful command is reused for the final stack: PR2 is
  specification-only; PR3 changes only the native opening-request choice, leaving these browser-mode admission, geometry
  and refusal paths unchanged. Later PR1 changes only added its execution URL.
- PR3: `node --test crates/farhelm-ui/js-tests/terminal-files.test.js` passed all 28 addon tests, run
  `8c93c0bc-27dd-4182-a52d-b6036d178d31`, at tested working revision `6d1eb1beb7a4f1d9c46383cd34dcd5529c9e6e68`, before
  the execution-URL amendment. Coverage includes both platform modifiers, false or absent opening offers, allowed
  opening, later opener refusal, save-only results and unchanged-cell checks.
- Targeted `dprint check`, `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py`, and the
  isolated-interpreter test-sleep checker passed. The sleep checker inspected 304 delays with no missing rationale; its
  coverage remains applicable because later changes touched JavaScript unit tests and prose, with no Rust or
  browser-test changes. CLI, fixture and web builds passed as browser prerequisites.
- Earlier attempts remain retained: `136c483e-7a11-4148-9e1c-29f78107f808` selected a renamed test group and ran no
  cases; `903c0079-818d-4aa3-9786-6df123903bbe` had two fixture failures because the injected notice covered the
  hover-reset corner; `7260628f-fd22-44fe-9594-25dc82173fbd` had two positive-control fixtures missing an opening offer.
  Those fixtures were corrected and the affected tests passed in the narrow commands above. Failed commands are not
  claimed as successful coverage.
- Rust runtime tests, Clippy and broader browser suites were skipped: no Rust code changed, and the focused
  browser/JavaScript cases cover the changed behavior. The specification-only PR ran no runtime tests. Native
  desktop-shell and physical Mac input checks were not run.

## Review gate outcome

Each PR passed a separate fresh-context gpt-6.1-sol review at high effort for correctness, design and idiomatic code.
The first review also rechecked the browser fixture correction; the third covered the opening-offer fixture and
documentation updates. No findings remain. Commit and PR wording passed fresh-context cold reads. The public report
received its own required cold read before delivery.

### Landing

Landed as #2348, #2350 and #2351, merged bottom-up, unchanged from what the report describes. It landed in the same
round as agent-spawned-session-rules, which merged just before it. The machine running the landing rebooted between the
two plans' merges; nothing had merged for this plan at that point, and the landing carried on afterwards.

#### What landed since the stack was based, and whether it interacted

Since the stack's base, main gained agent-spawned-session-rules (blank messages to running agents are refused, plus two
specification clarifications about agent-composed launches and prompt cards) and plan bookkeeping. A fresh-context
reviewer and I read the two plans against each other: they edit different sections of SPEC.md, keep separate
TRIAGE_OUTCOMES.md entries, and only this plan touches the UI. The only conflict was in the review feedback queue's
index, where each plan removes its own entries; both removals are kept. The landed code is identical to what was tested.

#### Fixes made during the landing

None.

#### Things the landing found

- The tunnel-error notice used to sit above everything else in the Mac app. To keep the refusal readable, this plan
  moves it to the same layer as approval cards. It is now drawn below hover help, the brief launch-sequence message near
  the bottom of the terminal, and the tunnel permission dialog's dimmed background. It still sits above approval cards
  where the two overlap, as before. Where these overlap, in a narrow window say, the notice can now be partly hidden;
  nothing about what a click does changes. Found by reading the stylesheet, not seen in the app. I think this is
  acceptable: the overlaps are brief or transient, and the alternative is a notice that can hide why a click was
  refused. It is listed under follow-ups in case it looks wrong in the app.
- The executor's browser run covered only the new tunnel-notice cases, which left one gap the report did not mention:
  the plan also rewords the existing refusal for a link hidden by an approval card ("covered by another Farhelm
  surface"). The landing's run of the whole link-click file closes that gap; it passed.

#### Possible follow-ups

- The two changelog drafts are worded for curation: the tunnel-notice one does not say the notice exists only in the Mac
  app, and the file-opening one says "a modified click" where the 0.30.0 notes say "Cmd-click (Ctrl-click on Linux)".
- If the tunnel-error notice looks wrong partly hidden behind hover help or the launch-sequence message in the Mac app,
  give it its own layer between approval cards and the refusal message.

#### Checks

Run now, on this stack rebased onto main with agent-spawned-session-rules beneath it, which is the tree that landed
apart from plan bookkeeping:

- `node --test` in the UI's JavaScript test directory through the recorder: 311 passed, run
  `2eb59d77-4530-4f5b-99c8-055a9a133d52`. The report's 310 was from before the opening-hint fix added its test.
- After `cargo build` and the release web build,
  `npx playwright test terminal-link-clicks.spec.ts approval-layout.spec.ts` through the recorder on Chromium and
  WebKit, one worker, no retries: 50 passed, run `ece7b2c1-da21-4d50-893b-5480e5676e38`. These are the whole files that
  exercise link refusals and the approval region, not only the new cases.
- `dprint check` on every changed documentation, changelog, stylesheet, script and test file, and
  `python3 releasing/check-changelog.py format`: passed. The Rust checks from the same run are in
  agent-spawned-session-rules' landing notes; this plan changes no Rust code.
- The machine rebooted after both runs had finished and been read; their records were lost then, so the results above
  are from that reading.
- Skipped: the native Mac app and physical Mac input, as in the report, and the rest of the browser suite, since the
  plan changes only link refusals, their stacking and the desktop opening decision.

Nothing in the executor's report became untrue at landing.
