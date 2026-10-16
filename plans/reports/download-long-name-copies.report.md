### What this was about

The desktop app could save a terminal-linked file once, then fail every attempt to save another copy if its name was
already near the filename limit. The copy number made the proposed name too long. The maintainer delegated a small
change to shorten only numbered copies' stems, preserving their final extension and leaving the first save unchanged.

Numbered names now reserve room for the copy number and extension within a fixed 255-byte limit, shortening the stem
only at a UTF-8 character boundary. Ordinary names retain the existing naming convention.

### Things you should know

The fix stays within the agreed name-allocation function and its unit tests. Atomic publication still chooses a free
name without overwriting existing contents. This adds no per-filesystem limit query or new publication error handling.
An extension that leaves no room for the numbered stem retains the existing failure rather than changing the file type.

The regression uses actual filesystem publication in an owned temporary directory, with ASCII and multibyte names at the
component limit. It confirms the second copy's bytes and the original's preservation. No live desktop save was run.

### Open questions and possible follow-ups

None. The fix met the recorded complexity gate without a scope departure.

### The PRs

- [#2114: save numbered downloads with long names](https://github.com/scode/farhelm/pull/2114/changes) — draft, based on
  main.

### Checks run, reused and skipped

- Recorded `cargo nextest run -p farhelm-helm --lib -E 'test(downloads::)'`: all 14 selected tests passed; 1,096
  unrelated tests selected out. Run `0e20498b-0496-46b3-a0ba-8924f0fcd336`, with pinned nextest and tmux, four slots and
  no retries. The complete JUnit and output were inspected; no runtime substrate skip was found. This includes the three
  new naming tests and existing transfer, native save/open, no-clobber, failure and cancellation coverage.
- Cargo formatting, changed-Markdown dprint, changelog format lint, the isolated sleep checker and explicit-commit Git
  whitespace check passed. The sleep checker inspected 301 delays with no missing reasons.
- The first recorder invocation refused before executing tests because its default evidence directory was inaccessible
  in the build container. The corrected run used an owned evidence directory and passed. This was a setup refusal.
- Ran `cargo clippy -p farhelm-helm --all-targets -- -D warnings`: passed.
- Later changes only add the PR link; executable source and tests are unchanged. Incoming template-editor changes, Codex
  guide text and plan tracking do not affect download naming or its tests, so no rebase or runtime repetition was needed
  for those changes.
- Skipped full workspace tests, browser, desktop and installer runs because this change is confined to the helm's naming
  iterator and the real publication regression directly covers the affected operation. The affected crate's lint covers
  all changed Rust targets; no wider lint risk was identified.

### The review gate's outcome

The required fresh-context gpt-6.1-sol high source reviewer reported no findings after receiving the full outcome,
complexity gate and test-authoring contract. It performed static review and did not independently run tests. A separate
gpt-6.1-sol medium wording cold read passed. Implementation and investigation stayed with the executor; no workhorse was
used. The report receives its separate native cold read before delivery.

### Landing

Landed as #2114, the second of the round's seven plans, tested stacked with the others on main.

#### Review

An independent fresh-context review found nothing blocking: the stem is shortened against the byte limit without
splitting a character, the final extension is kept (which opening in the desktop app depends on), every candidate is
distinct, and a name with no room for a stem fails once instead of looping. The bug shipped, so a "fixed" entry is
right.

#### Fix made while landing

The changelog fragment now starts from what the user does: clicking a file path in a terminal to download it in the
desktop app no longer fails when Downloads already has a file with that very long name.

#### Left as is

With an extension that leaves only a few bytes, the stem can shrink to nothing, giving a name that starts with a space;
legal and very unlikely.

#### Checks

Run on all seven of this round's plans stacked on main (managed-checkout-row-accessibility, download-long-name-copies,
save-template-ime-escape, helm-start-without-ca-bundle, stopped-session-launch-error, trash-totals-unchecked,
checkout-override-error-wording), with every landing fix in:

- `cargo fmt --all -- --check`, `dprint check`, the changelog lint and the sleep checker (301 delays, none missing a
  reason); `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`, the UI crate's
  clippy with the desktop feature and `cargo check -p farhelm-desktop`.
- Through the recorder, after rebuilding the CLI: the protocol, helm and supervisor packages in full with pinned tmux,
  run `4256456b-4340-4e64-86a5-85aa71206a7a`: 2395 passed; the UI crate with the desktop feature, run
  `5d0c7c3a-4d29-4a92-a8e2-a1eba6893786`: 656 passed.
- The JS harness (257 passed) and the website build.
- The github-checkouts, templates, trash-dialog, sidebar, provisioning and tooltip-coverage specs on Chromium and
  WebKit, run `7f4cef76-eea8-4b05-9a31-1260bb14498a`: 456 passed.
- Not run: the CLI's end-to-end targets and other browser specs; no PR in this round changes them.
