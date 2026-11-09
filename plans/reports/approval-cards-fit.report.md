## What this was about

Tall agent request cards could push Allow and Deny below the visible area. Long requester IDs, narrow fact columns and
permission explanations made session-start requests especially awkward. Every request kind now shares an aligned fact
list, a compact From line, bounded text boxes and an answer bar that stays visible. The region can use three quarters of
the main area; text boxes shrink toward two lines before the details between the header and answers scroll.

## Things you should know

The requester ID is shortened only on screen. Its full escaped spelling remains in hover and accessible text, and Copy
ID copies the full raw reference. Target and copied-session IDs remain complete. Technical destinations scroll sideways
within their own values; URLs and permission flags wrap in full. Other waiting requests have their own scrolling list.
When even the viewport cannot fit the fixed controls, waiting list and late-answer notice, the whole region scrolls so
every answer remains reachable.

Permission chips reuse the session list's classification. Own setting still explains that Farhelm treats it as YOLO;
command-launch assertions remain unchecked claims. YOLO and Custom share the warning YOLO chip; Guarded names the vendor
mode; Approve, Smart approve and Chat retain their mode names; Unknown identifies a launch predating classification.
Stored workspace trust stays separate. Templates keep their existing permission meanings. Answer order, permanent
grants, raw targets and the 700ms arming pause are preserved.

The completed TODO entry is removed and a fixed changelog fragment is included. The docs-shot assertions were updated,
but no docs image was recaptured or published: the published docs shots still show the old card.

## Open questions and possible follow-ups

None required for this plan. Native link opening was not exercised on a Mac; the desktop-only button path was preserved,
the desktop-feature Rust tests passed, and WebKit supplies shared-renderer engine coverage. No live installation
changed.

## The PRs

- [#3273 — keep agent approval buttons visible](https://github.com/scode/farhelm/pull/3273/changes), draft targeting
  main, head `00a8058545c98b49e4358b0c42eaab11681036bf`.

## Checks run, reused and skipped

Run during execution through the recorder. Commands below are the child commands, with private evidence locations and
tool paths omitted. Browser commands ran from `e2e/` on Chromium and WebKit:

- UI approval Rust tests, `cargo nextest run -p farhelm-ui --features desktop --lib -E 'test(/^approvals::/)'`:
  `8370c0d4-97c3-4829-9457-4271b2d74057`, 26 passed, 1,094 selected out. Pinned nextest 0.9.148, four slots, zero
  retries; no tmux needed.
- `npx playwright test 'approval-layout.spec.ts' -g 'every approval kind'`: `90b10670-5644-474c-9b6c-37af642d82d4`, six
  passed, three per engine. Each width steps through 29 fixtures covering all 15 wire kinds, launch verbs and link
  variants.
- `npx playwright test 'approval-layout.spec.ts' --grep-invert 'every approval kind'`:
  `ad9cc83c-2204-4998-a549-d3025b6c68ef`, 56 passed, 28 per engine. Together the two complementary selections cover all
  62 cases, including computed text shrink limits, notice/queue budgeting, the tiny-viewport exception, identity
  copying, host cards and arming behavior.
- Selected spawn, managed-checkout, keyboard, modal, Rename and host-setup regressions:
  `f55743e1-e62b-460a-b73b-61e4d9271823`, 22 passed, 11 per engine. These include real fake-agent approval answers,
  permanent grants, denial and keyed recovery, plus independent focus ownership and keyboard scrolling. The exact child
  selection was:

  ```sh
  npx playwright test '(^|/)(spawn|github-checkouts|keyboard-scroll|modal-trap|terminal-replay-rename|provisioning)\.spec\.ts$' \
    -g 'fake agent spawns|host-only grant|denying a spawn|agent managed checkout approval|approval.s complete initial prompt|SSH command arrival preserves an independent approval owner|Rename wraps Close|held rename success preserves an unrelated focus destination|host retirement releases before removal and preserves approval focus|agent host add asks'
  ```

- Approval-covered terminal links and refusal layers: `346dfc1e-d02c-4ed6-aeaf-f6a51e2f19e1`, 10 passed, five per
  engine. This file belongs to the older-terminal suite, so its recorded run explicitly retained tmux mode. The other
  browser runs used terminal hosts. All browser runs used three worker slots, owned stacks, pinned tmux and zero
  retries. The recorder retained `FARHELM_E2E_SUITE_TERMINALS=tmux`; global setup confirmed that mode. The child
  selection was:

  ```sh
  npx playwright test 'terminal-link-clicks.spec.ts' \
    -g 'a covered file hover refuses plain and modifier clicks before transfer|covered OSC web targets refuse while plain URLs and uncovered OSC targets open|header confirmation stays above a link refusal|the raised refusal explanation draws above every marked link cover'
  ```

The final four browser commands passed 94 cases with no skipped, unstarted, failed or flaky cases. Their source captures
and reports are complete, output reached EOF without truncation, no forced cleanup was needed, and all three stacks
finished cleanup after each command. The completed selected output has no runtime `SKIPPED` markers. Final-bundle
screenshots were inspected at wide and narrow sizes, including long host labels and prompt data.

Also passed:

- `cargo clippy -p farhelm-ui --features desktop --all-targets -- -D warnings`
- `cargo fmt --all -- --check`
- `node --check crates/farhelm-ui/assets/approval-layout.js`
- `dprint check SPEC.md SPEC_impl.md TODO.md releasing/changelog.d/approval-cards-fit.md`
- `git diff --check`
- `python3 releasing/check-changelog.py format`
- `python -B scripts/check-test-sleeps.py`, with the isolated interpreter: 429 calls, zero missing reasons.

`cargo build -p farhelm` and the pinned `dx build --package farhelm-ui --platform web --release` from the UI crate
completed before their applicable browser runs.

Reused after rebase: the Rust result and Clippy cover unchanged Rust; the final four browser runs cover the final
renderer, CSS, layout script and tests on the working tree based at `7501c63b91feadb0e89553bad036329e10525d73`. Incoming
changes through `a6b6deccd56f3b67910cf24d29984dd1b2540cfe` concerned Mac uninstall and bookkeeping, with no approval
interaction. The feature interdiff was empty, source hashes stayed identical, and the final commit's tree matches the
rebased candidate. Builds and static checks retain their coverage; changed-Markdown and whitespace checks passed again
after rebase. These runs make no claim about the incoming Mac uninstall change.

Earlier failed development attempts remain retained, rather than replaced by these passes:

- `c2f5e7b4-1842-47c3-be32-ff33dae91a14`: the initial Rust approval selection passed 24 and failed two new tests. One
  fixture exceeded the real ID byte limit; another tried to establish a static Dioxus class through dynamic mutations.
  Corrected tests passed narrowly, then in the final 26-test selection.
- `8f4cbd63-c758-45da-908f-4540cc3e2981`: the exact WebKit short queue/notice case reproduced the reviewer's clipped
  answers before the fixed-content budget correction.
- `6bcfae00-9ada-4d1b-b378-e4c978097b68`: after that fix, the same case exposed fractional test-scroll positioning of an
  answer border. The test now scrolls just inside the boundary while retaining full-visibility assertions.
- `4b34402b-7c3a-455c-ba6e-7f6126961796`: the same case then reached the final queue header, whose border was less than
  one pixel beyond the rounded scroll limit. Its oracle now checks every clipping boundary within the existing one-pixel
  tolerance and actually selects the request.
- `9c7ef3de-f0d8-4a2b-bd96-b5c9c73e53f2`: the first full layout command passed 57 and failed five. Three WebKit width
  cases measured before the approval observer settled; two shrink cases exposed the well's content-box/frame mismatch.
  Separate observer readiness and explicit border-box sizing corrected them.

One full layout attempt was terminated before publishing its report (`c150399c-9453-4dae-9642-7ef92cda1ea4`); its counts
and cleanup are unproven and supply no pass evidence. The recorder summary retains that gap and the five failed commands
separately. No same-session failure was added to the latent-flake ledger.

Skipped: the full workspace/browser batteries, native desktop/Mac execution, real vendor-agent turns, unrelated
installer and provisioning suites, docs-shot capture and CI dispatch. The selected checks cover the shared renderer,
geometry, answer and focus risks; those broader runs add no identified evidence for this change. Docs recapture was
outside scope.

## Review gate's outcome

The required single fresh-context gpt-6.1-sol/high reviewer found one P1: fixed waiting headers and a late-answer notice
could consume space the fallback calculation had reserved for answers. The measured fixed-content minimum and a boundary
regression resolved it. The same reviewer cleared the final CSS and test deltas with no outstanding findings. A separate
own-model scope check confirmed the small long-host button-wrapping correction needed no new mechanism. Commit and PR
wording passed its cold read, and every touched file received a separate documentation pass.

### Landing

Landed alone, as #3273. After the executor's last rebase, main gained the **Always update** button in the sidebar's host
update prompt and a TODO.md entry. The only conflict was textual, in TODO.md, where this plan's entry and the update
prompt's entry were both removed. A fresh-context review read the change against main. The two plans share no styles,
and the request region is sized from the main pane, so the update prompt's new button and error line cannot change its
space. The permission labels the cards reuse from the session list were not touched by the other change. The plan also
relies on the order of answers, the 700ms pause before answers count, and the ID shortening; the review found all of
these still hold.

#### Fixes made at landing

- The new From line showed the full host, session title and requester ID on hover through the browser's own tooltip.
  Farhelm's rule (SPEC_impl.md, hover help) is that no element uses the browser tooltip, because its delay of a second
  or more cannot be shortened, especially in the Mac app's WebKit. The page-wide check for that rule never opens a
  request card, so it did not catch it. All three now use Farhelm's own tooltip, with the same text. The plan's tests
  that checked the hover text, and the docs-screenshot review capture that checks the requester ID, now look for
  Farhelm's tooltip. This is the change the reviewer proposed; the fix itself was not reviewed again, and the checks
  below ran on it.

#### Things you should know

From the review, left as they are:

- When **Always allow** cannot turn the host's setting on, the reason appears at the bottom of the card's scrolling
  details. On a card whose details overflow, the user sees the answer buttons do nothing and has to scroll the details
  to see why. Moving the reason up next to the answer bar would fix it; that changes the card's measured layout, so it
  was not done at landing. This one is worth a follow-up.
- For screen readers, the From line's ID carries the full ID as its label, and the host and title are labelled "From
  host" and "From session". Those labels sit on plain text, where screen readers treat them unevenly: some ignore them
  and read only the shortened ID, others read "From host" or "From session" in place of the host name or title.
- On template cards the "asserted YOLO" chip stretches across the whole value column and comes out in italics.
- A few of the new layout tests are weaker than they read: one card-height check always passes, the narrow-pane test no
  longer checks that labels stack above their values, and the narrow short-queue test accepts either layout.

#### Checks

Run on the plan rebased onto main with the landing fix, the tree that merged, on this development machine and, as the
maintainer asked for the landings of 2026-11-09, on Tensorlake sandboxes:

- Here: `cargo fmt --all -- --check`, `dprint check`, `python3 releasing/check-changelog.py format`, the test-sleep
  checker, `node --check` on the layout script, `scripts/check-desktop-assets.sh`,
  `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`,
  `cargo clippy -p farhelm-ui --features desktop --all-targets -- -D warnings` and the UI's JavaScript unit tests
  (`node --test`) passed.
- Here, recorded `cargo nextest run -p farhelm-ui --features desktop --lib`: 1,120 passed, none skipped (run
  `1f963d06`).
- Tensorlake, browser, using the release web build, on Chromium and WebKit: the `approval-layout` spec (all of the
  plan's layout tests), the specs the executor chose for real approval answers, focus and keyboard scrolling (`spawn`,
  `github-checkouts`, `keyboard-scroll`, `modal-trap`, `terminal-replay-rename`, `provisioning`), and `host-updates`,
  whose sidebar change landed just before. 411 passed, one failed, and two skipped. The skips are the real-agent spawn
  test, skipped once per engine because it runs only with vendor credentials. Runs `9c795d47`, `b0705d5e`, `fe0d996d`,
  `ed71cc13`, `5b2521ed`, `52560fd6`, `ee09cda8`, `5ee19fec`, `0a574057` and `206decae`.
- The failure, in run `5b2521ed` (Chromium), was `expanded setup failure closes the menu below it` in the provisioning
  spec: a host's menu in Hosts never opened. That test is an open flake already recorded in FLAKES.md (2026-11-05) with
  this symptom. The plan does not touch Hosts or its menus. Run alone five times on Chromium on a sandbox, on this
  build, it passed every time (run `30999f77`), and it passed on WebKit in the main run.
- Tensorlake, the separate set of tests for old tmux terminals: `terminal-link-clicks`, which covers links under a
  request card, on both engines. 36 passed (runs `1f90e3e8`, `c277e3bd`).
- The browser runs went through the recorder's general command mode, because its Playwright mode always runs both
  engines and the runs here split the engines across sandboxes; the recorder still keeps each run's output and result,
  and under the sandbox image's Node 24, a slightly older release than this machine's. Every sandbox reported that the
  services the tests rely on (a per-user systemd and SSH to itself) worked.

Reused: nothing from the executor's runs.

Skipped: the docs-screenshot review capture whose assertion the landing fix changed (it needs the staged docs fleet and
only photographs for development review; the same attribute is checked by `approval-layout`), the rest of the browser
suite (the change is confined to the request cards, which the specs above cover), the workspace's other Rust tests (no
helm, supervisor or protocol code changed), the desktop smoke (no desktop-only code changed) and hosted CI (manual-only,
not dispatched).
