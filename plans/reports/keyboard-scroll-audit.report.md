### What this was about

Keyboard users could not reach and scroll some text-only areas in SSH connection help, host removal, the feedback sent
message, host settings while a save disabled its controls, and an agent's initial prompt in an approval request. WebKit
does not automatically make these areas Tab stops.

Those areas now have named keyboard entry points when they need one. Editable dialogs keep their existing Tab order.
Long SSH commands wrap without changing the copied bytes. A browser guard checks overflowing dialog regions, and SPEC.md
records how menus, suggestion lists, terminal scrolling and macOS button navigation fit the rule.

### Things you should know

The browser checks exercise Chromium and WebKit with the real renderer, including Tab or Shift+Tab into each fixed area
and PageDown scrolling. They do not exercise the native desktop bridge or macOS's system keyboard-navigation setting;
that native verification remains deferred.

Two initial browser commands failed because the feedback test's height cap still fit its short message. The corrected
fixture caps the content box below one line and separately proves actual overflow before keyboard input. Those failures
remain recorded and are not counted as passing evidence.

### Open questions and possible follow-ups

No unresolved product decisions or additional failing regions were found in this audit.

### The PRs

- [#2222](https://github.com/scode/farhelm/pull/2222/changes): keyboard reach for the five areas and wrapping SSH
  commands.
- [#2226](https://github.com/scode/farhelm/pull/2226/changes): overflow guard, keyboard-rule clarification and removal
  of the covered TODO entry.

### Checks run, reused and skipped

- `npx playwright test 'modal-layout\.spec\.ts' 'ssh-help\.spec\.ts' 'feedback\.spec\.ts' 'approval-layout\.spec\.ts' 'provisioning\.spec\.ts' 'trash-dialog\.spec\.ts'`:
  224 passed across Chromium and WebKit, recorder `72298bfc-2134-4c28-8c2a-17bb26a6bd89`. One worker, no retries, pinned
  tmux 3.7c. The owned helm and supervisors ran; provisioning uses the suite's injected backend. No runtime skip marker
  appeared.
- `npx playwright test 'keyboard-scroll\.spec\.ts'`: all 18 cases passed across Chromium and WebKit on the final tree,
  recorder `b0929c0b-4363-464c-988c-719603e7c81a`, with the same one-worker, zero-retry, pinned-tmux policy.
- Earlier focused checks passed: 20 guard cases (`dc0460f4-c59c-4206-ac85-ee8ada02de30`), two corrected feedback cases
  (`d628c207-3a73-4bbe-8832-b4ac50b2749f`), and one Chromium feedback reproduction
  (`276113da-aecf-4a49-b1ed-1e6ed92093b9`). The two failed fixture commands are `5618fc51-26a1-4f08-b4a1-797f71012a61`
  and `3ef85b80-747f-4516-b699-b160b8f48c08`.
- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, and
  `cargo check -p farhelm-ui --features desktop` passed for the production changes in `922086f6`. Their coverage is
  reused: the second PR changes only the browser guard, specification and TODO entry. The CLI and fresh web bundle built
  successfully.
- `python -B scripts/check-test-sleeps.py` passed: 302 delays inspected, none without a rationale.
  `python3 releasing/check-changelog.py format`, `dprint check SPEC.md TODO.md`, and
  `python3 scripts/check-agents-md.py` passed. dprint has no configured CSS or TypeScript formatter and excludes
  fragments; those unsupported selections were not treated as passes.
- The full application test suite and unrelated backend tests were skipped: the selected browser scenarios cover the
  changed rendering, keyboard input, focus and scrolling. No additional runtime run was needed after committing
  unchanged trees. Main did not move, so no rebase invalidated the evidence.

### Review gate outcome

Each PR passed a single fresh-context gpt-6.1-sol review at high effort, covering correctness, design, idioms and the
repository's full test-authoring checklist. The first reviewer also checked the feedback-fixture correction. Neither
review reported an actionable finding. Commit and PR wording passed separate cold reads.
