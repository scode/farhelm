### What this was about

Five findings from the 2026-10-21 triage of the highest-priority review feedback, all settled without code changes.

Two of them were about prepared checkouts (the "keep a checkout ready for faster launches" setting). Farhelm keeps those
in a reserved hidden folder, `.farhelm-spares`, inside the checkout root. A code review found two ways Farhelm can
delete a folder there that someone else replaced, losing whatever was in it. First, a launch that takes a prepared
checkout renames whatever sits in that checkout's place to a name Farhelm treats as its own launch leftovers, without
checking that it is the folder Farhelm built, and a later supervisor startup deletes those leftovers. Second, the
cleanup after building a prepared checkout deletes its build folder even after noticing that the folder was replaced.
The specification promised that Farhelm never deletes a folder replaced behind its back, including inside this hidden
folder. You decided this was "just fix spec, accept it", with one principle covering both.

The third was about the error a session shows when its agent never starts, which can quote the last few lines of the
terminal. The specification promised to leave that quote out whenever Farhelm could not be sure the text came from the
failed launch. A review found, from tmux's source, one case where it can still quote a few lines of the agent's previous
run: the session's agent is restarted in the same terminal, and that previous run still had a full-screen program
showing when the terminal was narrowed. You chose to accept it in the spec. Triage had judged it a misleading diagnostic
rather than a privacy problem: the quote stays in that session's own error, and agents reading the session's status from
other hosts see only the word "error", so the old text reaches no one who could not already read that terminal.

The last two were discards. You chose to fix only what is super cheap with no real downside, and neither was:

- Repository labels Farhelm shows for sessions, including on the summary card, can look identical when a repository's
  metadata is malformed. Launching rejects such names, and every session action is tied to the session rather than to
  its label, so no harmful choice was shown; changing how every repository label is drawn would have a visible cost.
- When two attempts to finish an interrupted move of a checkout into Farhelm's trash overlap and pick the same place for
  it, the checkout could stay on disk but drop out of the trash Farhelm shows; no file contents are lost. That needs a
  filesystem that reports "destination exists" before "source missing". Linux checks the other way round, macOS's source
  did not settle it either way, and no supported filesystem with that order was found.

### Things you should know

- SPEC.md now says that inside `.farhelm-spares`, anything Farhelm did not put there, including a folder removed and
  recreated, is out of band, and Farhelm may treat it as its own and delete it along with its contents. The protection
  against deleting a replaced folder still applies everywhere else (managed checkouts, checkout roots, the trash), and
  the removal still never follows symlinks or crosses mounts. The new wording sits in the dedicated-ownership paragraph
  under Supported user environments, with a pointer from the prepared-checkout paragraph under Managed checkouts. The
  implementation notes now say which steps rely on it.
- SPEC.md's failed-launch excerpt rule now says the quote can, in rare cases, include text from the previous run in the
  same terminal, with the narrowed-while-full-screen restart as the example, accepted 2026-10-21. The implementation
  notes explain the tmux behavior and say it was read from tmux 3.7c's source and not reproduced. The reviewer of #2565
  re-read that source and agreed with the mechanism; nobody ran it.
- Nothing changed in code, so nothing Farhelm does changed; these PRs make the specification describe what it already
  does.

### Open questions and possible follow-ups

- A few code comments still describe the old promises and now disagree with the specification. Two comments in the
  prepared-checkout cleanup code say a prepared checkout is "never adopted", and two comments in the terminal code
  describe the failed-launch quote as containing only the failed launch's own output. I left them alone because this
  plan ruled out any code change. I recommend correcting them in a small comment-only follow-up, as both reviewers
  suggested, so a future reader of that code is not misled into treating the old promises as still required.

### The PRs

- [#2563](https://github.com/scode/farhelm/pull/2563/changes): specification accepts that Farhelm may delete replaced
  folders inside `.farhelm-spares`; closes both prepared-checkout findings.
- [#2565](https://github.com/scode/farhelm/pull/2565/changes): specification accepts that a failed launch's error
  excerpt can quote the previous run; closes that finding.
- [#2566](https://github.com/scode/farhelm/pull/2566/changes): closes the two discarded findings, with no spec or code
  change.

### Checks

- Run: `dprint check` on every changed file, and `python3 scripts/check-agents-md.py` after each PR's queue removals;
  both clean.
- Reused: none.
- Skipped: every runtime test (Rust, browser, JavaScript, installer). All three PRs change only specification text and
  review-queue bookkeeping, which the repository's validation rules exclude from runtime tests.

### Review gate

Each PR had one fresh Opus 5.5 review at high effort. None found a correctness problem; each checked the new
specification text against the code, and the reviewer of #2565 also checked it against tmux's source. The review of
#2563 caught a wording mismatch with SPEC.md's own terms and a sentence that credited the deletion to the launch rather
than the later startup cleanup; both were fixed. The review of #2565 made the tmux mechanism and the SPEC.md example
more precise (it takes a narrowing, not any resize). Both suggested correcting the stale code comments above; I did not,
for the reason given there. The review of #2566 found nothing to change.
