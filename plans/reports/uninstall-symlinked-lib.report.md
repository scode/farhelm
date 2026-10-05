### What this was about

The hosts panel's uninstall removes Farhelm from a remote host: it stops and removes the supervisor's service, deletes
Farhelm's program directory on the host (`.local/lib/farhelm` under the user's home directory, which holds the Farhelm
binary and any private tmux), and drops the host from the list. Before deleting anything, uninstall checks that the
supervisor it is about to stop runs from inside that program directory, comparing paths after following any symbolic
links. If the program directory was itself a symbolic link to a directory elsewhere, the supervisor's binary, followed
through the link, was inside the link's target, so the check passed. Deleting the directory then deleted only the link:
uninstall reported success and dropped the host, while the Farhelm program stayed on the host.

You decided in planning that uninstall should refuse this layout, both when it works out the list of steps it shows you
and again in the command that deletes the directory; that removing a host from the list without uninstalling must keep
working for any layout; and that the refusal should point to that. That is what landed:

- Uninstall now refuses a host whose program directory is a symbolic link, a dangling one included, before changing
  anything. The message names the directory and where the link points (or says the host cannot resolve the target), says
  uninstall does not run on this layout, and suggests removing the host from the list instead. The host stays listed and
  nothing on it changes. Uninstall works out its steps again when you confirm, so a link made between showing them and
  confirming is refused at that point, with no step run.
- The command that deletes the directory checks again right before deleting. A link made after you confirmed, while the
  earlier steps ran, makes that step fail with a message saying so; the host stays listed, and choosing uninstall again
  is refused up front.
- A link higher up the path, such as a home directory that is itself a link, is not refused: deleting the directory
  through it deletes the real directory, as intended.
- Removing a host from the list is unchanged: it runs none of uninstall's checks or host commands.
- SPEC.md's list of what uninstall refuses now includes this case and the pointer to removing the host; SPEC_impl.md
  says where the check happens and that the delete command repeats it. A changelog fragment is included, and the TODO
  entry "Refuse symlinked program directories during remote uninstall" is removed.

### Things you should know

- **Hosts running older Farhelm versions get the fix too.** The checks are shell commands the helm sends over ssh each
  time; they do not depend on the Farhelm version installed on the host.
- **Hosts uninstalled before this fix with this layout still have the Farhelm program on them**, in the directory the
  link pointed to. Nothing in this change finds or cleans them up; such a host is no longer in the list, so Farhelm
  cannot reach it. The layout is unusual, so I did not plan anything for it.
- **The link test runs before uninstall's other checks of the host's files**, including the one that refuses a path the
  host cannot resolve. Otherwise a dangling link would get the older, generic "uninstall cannot tell where … leads"
  message, which does not mention removing the host from the list. To do this, uninstall now asks, for every path it
  looks at on the host, whether that path is itself a link (the plan suggested asking only about the program directory);
  only the program directory's answer is acted on.
- **Some refusals still come first.** Uninstall checks for running sessions and open terminal tabs, and that the host is
  the same machine it has on record, before it looks at the host's files. A user with this layout and a running session
  is first asked to stop the session, and only on the next try learns that uninstall cannot run on this layout at all. A
  reviewer pointed this out; it was left as is, because changing it would reorder uninstall's checks beyond what the
  plan asked for.
- **The website's host management page was not changed.** It lists none of uninstall's per-layout refusals, so this one
  is not added there either; the refusal message itself explains the situation.

### Open questions and possible follow-ups

- The sessions-first order above could be changed so the layout is checked first. My recommendation is to leave it
  unless someone actually runs into it: the layout is rare, and the cost is one extra round of stopping sessions.

### The PRs

- #1614 fix: refuse remote uninstall when the program directory is a symlink.

### Checks run, reused and skipped

- Run: `cargo nextest run -p farhelm-helm --lib -E 'test(/^provisioning::/)'`, the helm's whole provisioning module,
  including uninstall's planning tests and a test that runs the real host commands against directories on the test
  machine (run `c90c58fa`, 191 passed). A first run of the same selection (`9548f1f5`) failed two tests on a mistake in
  this change's host command (for a service with no unit file it printed one value too few), fixed before `c90c58fa`.
  After the review fixes, the uninstall tests alone again (`530e449f`, 12 passed). `cargo fmt --all -- --check`,
  `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`, the test-sleep checker
  (0 unannotated delays), `python3 releasing/check-changelog.py format`, and `dprint check` on the changed Markdown. New
  tests cover the refusal (linked, dangling, and made between showing the steps and confirming) and the real host
  commands' answers and refusals for a linked, a dangling and a linked-parent program directory.
- Reused after rebasing onto the latest main: those runs. What landed in between (asking the user before an agent's
  farhelm command acts) adds a host setting and the records behind agent requests; it touches none of uninstall's
  checks, its host commands, or removing a host. The rebased change was compiled and its formatting and Markdown checked
  again.
- Skipped: the browser end-to-end suite (the panel's flow is unchanged, and browser tests do not run real host
  commands), desktop checks, and the installer and CentOS provisioning checks (they do not run remote uninstall).

### Review gate

The PR was reviewed by Claude Opus 5.5 and gpt-6-astra, both at high effort, as you required. gpt-6-astra found nothing.
Opus found no correctness problems and raised seven minor points, all applied; the one worth knowing is the
sessions-first order described above, which now has a comment in the code.

### Landing

Landed on 2026-10-05 (UTC) as #1614 (the uninstall symlink fix), one squash commit on main.

#### What else was on main

Nothing that could interact. The change was rebased onto main right after the feature that asks the user before an
agent's `farhelm` command acts landed, and between then and the landing main gained only the planning queue's own
bookkeeping, which touches no code, spec or test. No other plan landed alongside it. A separate reviewer that had not
seen the work checked this independently before the merge and reached the same conclusion. It also found nothing outside
the PR that the change breaks. The browser tests and docs screenshots that walk through uninstall run against a stand-in
for real hosts, which this change updated to report that the program directory is not a link, so the steps and messages
they check are unchanged. The installer, local uninstall and CentOS provisioning tests never run remote uninstall, and
the website quotes none of uninstall's refusals.

#### Checks

- Reused: the report's checks, on the reasoning the report gives for reusing its test runs across its rebase. The code
  that landed is the rebased change the report describes: nothing but the queue's own files reached main after it.
- Skipped: running anything again during the landing, since nothing that could interact landed in between.

Nothing in the report above was made untrue by the landing.
