### What this was about

When Replace with started keeping the source session's own folder, and the helm later began refusing any Replace that
would start a fresh managed checkout, the desktop launcher lost every way to start a managed checkout inside a sandbox
or container session that already exists. Some launcher code written for that case stayed behind. The main piece was the
note in the Destination card, below the checkout's folder, warning that such a folder is removed permanently, not moved
to the trash, when its last session is deleted. The others were three spots in Replace with's handling that looked for a
checkout of its own. None of it could run any more, and no test exercised it, so a later change bringing the path back
could have relied on wording and code nobody had checked. A trailing code review had recorded the leftover note as a
review-feedback item. You chose a full cleanup that also closes that item, with no release note.

What was built, in one PR:

- The launcher no longer has the existing-environment checkout note or the code that picked it. The note in the
  Destination card now says one of two things, as it already did in practice: permanent removal for a checkout inside a
  new sandbox or container the launch creates, and the trash for a checkout on an ordinary host. The code's comments say
  why there is no third case, and that a future path starting a checkout inside an existing environment would need its
  own note.
- The three Replace with spots that looked for a checkout of its own are gone. Replace with behaves exactly as before,
  including the line that says the replacement keeps working in the source's managed checkout.
- The review-feedback item is closed, and the TODO.md entry for this cleanup is removed.

Nothing changes for anyone running Farhelm.

### Things you should know

- **SPEC.md needed no change.** The review-feedback item also asked to remove a SPEC.md sentence promising the note for
  a checkout inside an existing environment. The helm Replace change had already removed it, so "Managed checkouts"
  matched the code before this PR.
- **No changelog fragment.** The PR is a `refactor:` with no effect anyone can see, and the unreleased changelog draft
  the TODO entry mentioned had already gone out in v0.46.0's release notes.

### Open questions and possible follow-ups

None.

### The PRs

- #5160 `refactor: remove dead Replace checkout code from the launcher`

### Checks

- The launcher's and launch templates' unit tests in the desktop app, through the test recorder
  (`cargo nextest run -p farhelm-gui --lib -E 'test(/launcher::/) | test(/templates::/)'`): 963 passed, none failed (run
  0caca2f6).
- `cargo fmt --all -- --check`, `cargo clippy -p farhelm-gui --all-targets -- -D warnings` (no unused code or dead-code
  warnings left), and `dprint check` on TODO.md and the review-feedback index: clean.

The tests, clippy and dprint ran on the change before review. The review's fixes changed only comments in the code
(rewording, and one comment rewrapped) and left TODO.md and the index alone, so those results still cover the change
that landed. `cargo fmt --all -- --check` ran again after the fixes, clean.

Skipped: screenshot comparisons, since none of the remaining drawing changed (the removed note had no screenshot left);
every test outside the launcher and templates, since only unreachable launcher code was removed.

### Review gate

A fresh-context Opus 5.5 agent at high effort reviewed the PR, then verified the fixes; the gate passed. It found no
bugs. It traced every way a Replace with draft or request could have carried a checkout and confirmed that none does,
that the helm itself refuses one, and that the live Replace with behavior is untouched. Its three findings were wording:
a comment that implied the wrong condition, a comment that relied on an unstated rule about its callers, and the commit
message's accuracy. All three were fixed, and the verification found nothing else.
