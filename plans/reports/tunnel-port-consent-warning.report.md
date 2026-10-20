# Tunnel consent explains local port use

## What this was about

A localhost tunnel prefers the remote service's port number on the Mac. A Mac program expecting that port can then reach
the remote host, but the one-time consent warning omitted this consequence. The maintainer chose a general sentence in
the existing warning, with the same explanation in the specification and docs.

## Things you should know

The warning now says: “A tunnel uses the remote service's port number on this Mac when it is free, so a Mac program
expecting that port may reach the remote host instead.” Consent applies to every tunnel, so the wording names no
particular port. This changes the explanation; port selection, consent storage and tunnel behavior are unchanged.
Already accepted consent is not reset.

The specification's Security warning list and the Work in a session guide now say the same. The plan's old
security-model path has moved to the docs website; its existing stub also lists the warning's topics, so it received the
same narrow addition. The changed pages are `/docs/using/work-in-a-session/#open-a-remote-localhost-link` and
`/docs/how-it-works/security-model/`. No preview server was started during this unattended run.

The change stayed within the requested cheap wording fix. Tunnels shipped in v0.30.0, so this adds a new fixed changelog
fragment rather than editing a published entry. The feedback file and index entry are removed, and the outcome ledger
names this change, bookmark and draft PR.

## Open questions and possible follow-ups

None required for this plan. This warning does not establish a demonstrated credential leak or change the existing
same-port preference. A macOS address-reuse experiment was outside the agreed wording change.

## The PRs

- [#2364: explain port reuse before enabling tunnels](https://github.com/scode/farhelm/pull/2364/changes) — open draft
  based on main, head `515ff7bd4aa007f59816ab598a8b92c06d6832d5`.

## Checks run, reused and skipped

Initial checks cover working change `xqqwknvzpwknwmsppovkkmzrnqsyqvkw`, snapshot
`5670fdf65a7c50b9482df890016d70ec47546683`, on main `0deb42e62244d38ea7ac710b71f2ec9fd616d022`. Final head
`515ff7bd4aa007f59816ab598a8b92c06d6832d5` adds the commit description and ledger PR link; compiled source, site
content, fragment, file paths and headings are unchanged.

- `cargo fmt --all -- --check` and `cargo clippy --all-targets -- -D warnings`: passed; the warning's Rust renderer
  compiles and lints. No test or state-handling code changed.
- From `website`, `bun install --frozen-lockfile && bun run build`: passed, 32 pages and all internal links valid. The
  builder printed missing optional i18n/404-content warnings; no broken link or build failure occurred.
- `dprint check SPEC.md TRIAGE_OUTCOMES.md review_feedback_queue/INDEX.md website/src/content/docs/docs/using/work-in-a-session.mdx website/src/content/docs/docs/how-it-works/security-model.md`,
  passed again on final head `515ff7bd`, including the amended ledger. The initial dprint attempt requested wrapping the
  inserted SPEC paragraph; after formatting, the complete check sequence passed.
- `python3 releasing/check-changelog.py format` and `python3 scripts/check-agents-md.py`: passed at snapshot `5670fdf6`.
  Reused on final head because the fragment, file paths and headings did not change. The Rust formatter, Clippy and
  website build above are also reused from that snapshot because compiled source and site content did not change. No
  earlier plan's runtime evidence was reused.
- Rust runtime, browser, JavaScript, native desktop, installer and provisioning suites were skipped: only static warning
  text, explanatory comments and prose changed, with no new interaction or execution path. No existing test or
  screenshot pins the warning wording, and a new test repeating that string would not add meaningful evidence. No test
  source changed, so the test-sleep checker was unnecessary. No live installation was touched.

## Review gate outcome

The required fresh-context gpt-6.1-sol reviewer at high effort reviewed correctness, design and idiomatic code against
the complete outcome and agreed scope. No findings. Commit and PR wording passed a separate fresh cold read. A
documentation pass covered every touched file. The final amendment adds only the PR link to the outcome record. The
report passed its independent cold read.

### Landing

Landed as #2364, unchanged from what the report describes. It was the last of eight plans in this landing round.

#### What landed since the stack was based, and whether it interacted

Since the PR's base, main gained link-hover-covers, workspace-trust-destination, session-header-and-row-fixes,
queued-launch-snapshot, payload-lock-reproduction, triage-discards-oct18, supervisor-colon-path-refusal and plan
bookkeeping. The only one near tunnels is link-hover-covers, which changed where the Mac app's tunnel-error notice is
drawn and made links refuse clicks while that notice covers them. This plan changes only the wording of the separate
one-time tunnel consent warning, so the two do not interact; a fresh-context reviewer and I read them together.

The only conflicts were in the review feedback queue's index, where every plan in the round removes its own entries; all
removals are kept. The landed code is identical to what the landing's checks below tested.

#### Fixes made during the landing

None.

#### Checks

Run now, on one tree holding this plan together with the round's other landing plans (launch-enters-working-dir removed
after it failed its checks):

- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo check -p farhelm-ui --features desktop`, `dprint check` on every changed file including the two docs pages,
  `python3 releasing/check-changelog.py format`, and the website build: all passed.
- The UI's desktop-only Rust tests, where the consent warning's code lives: 706 passed, run
  `4926be34-2adf-40e5-a588-663695e00161`. The whole UI library, among the other libraries: 3489 passed, run
  `cdabc40d-ecaa-418e-938f-01f2cbe4da64`.
- Skipped: browser tests (the consent warning exists only in the Mac app) and the native Mac app itself, as in the
  report.

The report's statements that the PR was an open draft and that its checks ran on the main it was based on were true when
it was delivered; main then moved as described above, and the PR has now merged. Nothing else in the report became
untrue at landing.

Changed docs pages, for review: Using Farhelm, "Work in a session", section "Open a remote localhost link", and How it
works, "Security model".
