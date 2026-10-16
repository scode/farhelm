### What this was about

The docs promised more than Farhelm does in two places: the paste guide said hidden control characters were removed, and
the restart guides said clearing a conversation prevented returning to it. Separately, the docs preview could show old
release notes after their source files were saved.

The agreed small fixes correct those claims and refresh the preview's release notes after changelog or screenshot-list
changes.

### Things you should know

“Work in a session,” under “Copy, paste, and links,” now says escape characters are stripped but other control
characters reach the program. The Codex guide and the opening of “Agent hook injection” explain that Restart uses the
conversation Farhelm last learned of. Restarting soon after a conversation switch can still bring back the earlier one;
Codex reports the new conversation after its next reply. Product paste and restart behavior did not change.

The preview now watches the changelog and release screenshot lists for edits, additions and deletions. Its existing
automatic in-process refresh remains, so a request during that refresh may briefly fail before a reload succeeds.
Invalid screenshot JSON reports an error without killing the preview; the next valid save can recover. Page generation,
the build and the preview command are unchanged.

The Overview page, README, published release notes and screenshots were not changed. The website has not been deployed.

### Open questions and possible follow-ups

None required for these three outcomes. All stayed within their complexity gates.

### The PRs

[PR #2104](https://github.com/scode/farhelm/pull/2104/changes): correct the paste safety claim.

[PR #2106](https://github.com/scode/farhelm/pull/2106/changes): qualify which conversation Restart resumes.

[PR #2109](https://github.com/scode/farhelm/pull/2109/changes): refresh release notes during docs preview.

All three are drafts in that linear order.

### Checks run, reused and skipped

- Changed-file dprint checks passed for each PR and after the final rebase and ledger amendments.
- Frozen dependency installation passed. The website build passed at each review unit: 32 pages and all internal links
  valid. The final successful build took about five seconds.
- The real preview command ran in an owned container against an isolated copy of the site. The old configuration
  demonstrably left both inputs stale. With the fix, subsequent changelog saves and screenshot-list edits, deletions and
  additions updated the rendered release-notes page. The same preview process survived every save, and the test stopped
  it normally. No maintainer preview port or published input was changed.
- The first copied-site attempt failed before its initial-page premise because linked dependencies confused Astro's
  compiled paths. Sharing that dependency directory also caused a cache-ownership refusal in one build. The fixture was
  repaired to copy its dependencies, the affected cache ownership was restored, and both the live proof and build then
  passed. Those failed attempts are retained and are not counted as passes.
- Reused each build, live result and source review after reading the entire incoming main diff and rebasing all three
  changes. Incoming changes were plan tracking and reports only; each PR's website files stayed identical to its
  reviewed and validated version.
- On resumption, inspected the retained findings and successful build/live output, then read the incoming diff from
  `717f3702` to `51e34da5` and rebased all three changes. That delta also contains only plan tracking and a report. Each
  stage's website files remain identical to the earlier pushed and tested revisions (`807aa8c5`, `f617b790`, and
  `ce711b6a`), so the same evidence still covers the rebased stack.
- Skipped Rust, product browser, desktop, installer and release tests: the first two PRs change ordinary prose, and the
  third changes only the docs dev integration, directly covered by the live preview proof and website build. No product
  runtime behavior changed.

### The review gate's outcome

All three required fresh-context gpt-6.1-sol high source reviews found no actionable issues. Implementation and
execution evidence were checked locally; the source reviewers ran no tests. Separate sol medium wording cold reads
passed after clarifying one description. The preview title retains chore because the plan explicitly assigns that type
to maintainer tooling. A documentation pass covered every touched file. No scope departure or unresolved source finding
remains.

### Landing

Landed as #2104, #2106 and #2109, bottom-up, fifth of the round's nine plans, tested stacked with the others on main.

#### Review

An independent fresh-context review found nothing blocking. The preview's release-notes watcher runs only under the
development server, never in a production build, watches only the changelog and the release-notes data, and cannot
trigger itself.

#### Fixes made while landing

- The Codex page said Farhelm learns the new conversation after `/clear` "once Codex has finished its next reply". Codex
  reports it when you send your next prompt, and it becomes resumable once that reply ends; the window in which restart
  can bring back the earlier conversation runs until that prompt, which can be indefinitely long. The page now says
  that, and the hook-injection page no longer says "soon after"; it says restart can bring back the earlier conversation
  until Farhelm has learned of the new one, which for some agents happens only at the next prompt or reply, and
  reintroduces the term "conversation reporters" that its next sentence uses.
- The paste paragraph now says escape characters are removed so a paste cannot end early and type the rest as
  keystrokes, while other control characters such as Ctrl-C still reach the program.

#### Left as is

Follow-up: the Pi page makes the same overstated promise ("After `/new`, restart no longer resumes the old
conversation"). The preview also restarts the development server when the changelog is saved, which is harmless.

#### Checks

Run on all nine of this round's plans stacked on main (terminal-link-safety, ntfy-token-server-change,
remote-uninstall-fixes, update-failure-reporting, docs-accuracy-fixes, host-picker-dotted-i,
setup-advice-state-directory, copy-guidance-resize, brick-test-feedback-evidence), with every landing fix in:

- `cargo fmt --all -- --check`, `dprint check`, the changelog lint and the sleep checker (301 delays, none missing a
  reason); `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`, the UI crate's
  clippy with the desktop feature and `cargo check -p farhelm-desktop`; the brick-test tooling's own test.
- Through the recorder: the protocol and helm packages in full with pinned tmux, run
  `6352fad3-a3b1-48dc-9884-3efc17ee03c6`: 1258 passed; the CLI's own unit tests, run
  `0e1855d7-6f43-4da7-8a9a-495936ae565e`: 171 passed; the UI crate with the desktop feature, run
  `513addbb-2505-4c54-bc07-2ac2ec06713b`: 655 passed.
- The JS harness (257 passed) and the website build.
- The terminal-link-clicks, terminal-links, terminal-files, ntfy-settings, provisioning, settings and tooltip-coverage
  specs on Chromium and WebKit, run `97a26936-2b7b-42a5-af59-8f0fdb0b9d78`: 208 passed, 2 failed. Both failures were the
  clipped-disclosure test pinning the refusal's old layering after the quick switcher closed (see terminal-link-safety).
  After that expectation was fixed, the terminal-link-clicks spec on both engines, run
  `a698ea79-451d-4dd4-87f5-24ef1d4b1cad`: 16 passed. An intermediate run, `6113fd52-60bf-4730-ae85-5e9dc2e90d46`, failed
  on WebKit only because the first corrected expectation assumed Chromium's behavior.
- Not run: the supervisor package and the CLI's end-to-end targets, which no PR in this round changes; other browser
  specs.
