## What this was about

Farhelm had no way to remove itself from a remote host. `farhelm uninstall` only removes Farhelm from the Mac it runs
on, and the docs' "Uninstall Farhelm" page told you to stop the host's sessions, then ssh in and run `systemctl` and
`rm` by hand. You asked (TODO "Uninstall Farhelm from remote hosts") for a supported way to do it from Farhelm, and for
those manual steps to go from the docs. Your answers during planning: an item in the remote host's `⋯` menu, the helm
doing the work over ssh like setup and Update, no CLI verb; refuse while sessions or terminal tabs are alive and name
them, never kill anything; keep the host's data and say where it is; on success the host leaves the list in the same
action, and a failure keeps the row and can be retried; the docs' manual steps go completely.

That is what was built. A remote host's menu now has **uninstall** (after the separator, above **remove**). Choosing it
asks the helm for a plan, which opens in a dialog naming, by their paths on the host, the supervisor's user service, its
unit file and Farhelm's program directory (the binary and any private tmux), plus the data directory that is kept and a
note that deleting it by hand removes the data. Nothing on the host changes until you confirm. The helm then disables
the service, removes the unit file, stops the supervisor, reloads the user manager, removes the program directory, and
forgets the host and its cached sessions. A line above the host list then says Farhelm was removed and where the data
remains. The helm's own machine has no uninstall item; Farhelm there is still removed with `farhelm uninstall`.

## Things you should know

- **The step order was changed by the review, and it matters.** The first version reloaded the user manager before
  stopping the supervisor. A reviewer pointed out, and I then confirmed on a real user manager with a throwaway unit,
  that systemd forgets a removed unit's `KillMode=process` at a reload: the stop then kills the whole control group,
  which includes the private tmux server holding the host's sessions. The order is now disable (without letting
  `systemctl` reload on its own), remove the unit file, stop, then reload. The real ssh-to-localhost test now keeps an
  ended session's tmux server alive and checks it survives the uninstall.
- **It refuses rather than guesses in a few situations**, each with a message naming the reason: a session that has not
  ended (an unknown status counts) or any open terminal tab; a session list too long for the supervisor to send whole; a
  host that is not connected, or whose supervisor answers but cannot be used (it runs another protocol version, or it
  does not prove it is the same machine, including one that answers without any identity at all); a host whose
  supervisor service was installed by `farhelm helm setup` there (the message points at running `farhelm uninstall` on
  that host); a connected supervisor not run by its unit (one started by hand); a unit systemd loads from somewhere
  other than the planned path; a binary outside Farhelm's program directory; and a data directory that really lies
  inside the program directory (paths are compared as the host resolves them, so a symlink cannot hide it). All of it is
  checked again when you confirm, by planning again and requiring the same plan.
- **Retrying a failed run.** The run stops at the first failing step and keeps the row, with its steps showing what is
  left; choosing **uninstall** again plans only what remains. Because the unit file goes before the supervisor stops, a
  retry still has a connected host up to that point. After it, a retry may finish without a connection, but only when
  the helm's probe positively shows no supervisor answering. A retry after the program directory is already gone also
  finishes.
- **One state it will not handle on its own:** if something reloads the host's user manager between a failed run's unit
  removal and its retry, the running supervisor has lost `KillMode=process`, and stopping it would kill the tmux server.
  Uninstall refuses there and says an Update from the hosts panel rewrites the unit, after which uninstall can run. The
  same refusal covers a hand-written unit without that setting.
- **It does not stop the private tmux server.** If the host still has ended sessions, their tmux server keeps running
  (from the removed binary, if it was Farhelm's own tmux) until it exits or the host reboots. Stopping it would be a
  kill, which you ruled out, so this is recorded in SPEC_impl.md rather than handled.
- **Progress shows in the row the way Update's does.** While the run goes, the row's status spot shows "uninstalling"
  with the step count and current step, and hovering it lists every step, using Update's own inline status. This matters
  after the stop step, when the row's connection status alone would read unreachable while the removal is still going.
  The row's details also show the full step list, since the row opens for the confirmation dialog.
- **The success notice is built in the browser** from the plan you confirmed, because the helm keeps nothing for a host
  it has forgotten. It is registered before the request is sent, so it still appears if the host leaves the list before
  the reply arrives, and it is withdrawn on a refusal, a failed run, or a manual remove of that host.
- SPEC.md gained a paragraph in Topology describing uninstall, a pointer from "Uninstall scope and interaction", and
  matching lines in "Ownership during cleanup and provisioning" and "Supported host setup". SPEC_impl.md's Provisioning
  section describes the operation, the step order and why, and the checks.

## Open questions and possible follow-ups

- **Screenshots for the new docs section.** The site's editorial rules ask for annotated screenshots wherever a page
  shows where to click. The new "Uninstall Farhelm from a remote host" section on Manage hosts is prose only. New shots
  are captured and then published only after you have approved them, and a production docs build fails on an unpublished
  shot, so an unattended run cannot add them without blocking the landing on you. The options: approve this as is and
  ask for a follow-up that adds shots of the menu item and the confirmation dialog; or ask for that follow-up before
  landing, which delays it. I recommend the first: the prose is accurate, the follow-up is small, and the shots can be
  checked against a page you have already read.
- **A program directory that is itself a symlink.** If `~/.local/lib/farhelm` on a host is a symlink, uninstall removes
  the link, reports success and forgets the host, while the directory it pointed to (with the Farhelm binary) stays on
  the host. Setup never creates that layout, and the host's data is safe either way. The options: accept it as an
  unsupported layout with a slightly misleading success; or have uninstall refuse a symlinked program directory with a
  message naming its target, a small follow-up. I recommend the follow-up refusal, since a success that leaves the
  binary behind is the kind of quiet partial result SPEC.md's "Supported host setup" says to refuse instead.

## The PRs

1. https://github.com/scode/farhelm/pull/1565/changes `feat:` the helm side: the uninstall operation and its HTTP route,
   the host commands, the checks, SPEC.md and SPEC_impl.md, and tests (including a real uninstall over ssh to
   localhost). Its last change, the browser cleanup's matching of the reworded "busy" refusal, came after its own three
   review rounds and was reviewed in PR 2's third round.
2. https://github.com/scode/farhelm/pull/1568/changes `feat:` the hosts panel: the menu item, the confirmation dialog,
   the success notice, the docs pages, a browser spec, the changelog entry, and the TODO entry's removal.

Both were rebased onto main after the add-host dialog trim landed; the only conflict was adjacent TODO.md removals.

## Checks

Run on the final code (recorder run ids in brackets); later changes to these commits touched only docs, SPEC text,
comments and test files outside each run's selection:

- `cargo nextest run -p farhelm-helm -p farhelm-ui` selecting the helm's provisioning and hosts modules and the UI's
  hosts, provisioning and api modules [8c7b202d]: 315 passed. This includes the real-transport provisioning cases; the
  ssh-to-localhost one ran its uninstall leg for real (no skip), and the local one ran too. The UI changes made after
  that run (inline progress and the retry notice fix) were covered by the UI modules again [65499f6a]: 91 passed.
- Playwright on Chromium and WebKit: the whole `provisioning.spec.ts` plus the `terminal-multihost.spec.ts` remove and
  uninstall tests (both uninstall specs included) [e88f57e2]: 108 passed.
- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm-ui --features
  desktop --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, `dprint check`, `python -B scripts/check-test-sleeps.py`,
  `python3 releasing/check-changelog.py format`, and the website build (`bun run build`, all internal links valid):
  clean.

Reused, with why it still counts:

- Playwright on both engines, the `terminal-multihost.spec.ts` update-progress, host-menu and add-host tests [f2234efe]:
  they passed there. That run predates two changes: the provisioning spec's cleanup fix (that spec ran in full
  afterwards [e88f57e2]) and spec and docs wording. Neither touches what those tests exercise. Its failures are covered
  in the next list.

Failures seen along the way, and what they were:

- Four existing host-menu tests expected **remove** right after the menu's separator [8912e897]. Updated for the new
  item; passing since [dda035d2, f2234efe].
- `provisioning.spec.ts` tests failing in their cleanup, which could not remove the hosts they had added [8912e897 once,
  f2234efe three times]. This was my regression, not a flake: the spec's cleanup retries a removal refused as busy only
  when the refusal's text matches, and PR 1 had reworded that text to mention uninstall. I first took the single early
  occurrence for a timing flake, and five repetitions [hunt batch 63d441a9] happened not to hit it. The cleanup now
  matches the new wording (in PR 1, with the message change), and the whole spec passes [e88f57e2].
- "remove-skip-confirmation: a saved answer removes directly" timed out once on Chromium waiting for the host actions
  panel to finish measuring its position [f2234efe]. It passed in every other run that included it, before [8912e897,
  123a56ed] and after [e88f57e2], on both engines. Not logged as a flake on one observation.

Skipped: the full nextest battery, doctests, the UI's JavaScript unit tests, the CentOS provisioning script, the desktop
smoke, `cargo check -p farhelm-desktop` and the desktop asset check. The change is confined to provisioning, the hosts
panel and their tests, which the selections above cover; no doc examples or JavaScript assets changed; the desktop app
embeds the UI crate, whose desktop build the Clippy run above compiled, and no assets were added; the CentOS container
would exercise the same commands against the default layout, which the ssh-to-localhost case runs with fixture paths,
and nothing touches the desktop app.

## Review gate

The gate passed for both PRs: nothing open remains from the reviews beyond the declined points below. Each PR was
reviewed by a fresh Claude Opus 5.5 agent at high effort and a gpt-6-astra agent at high effort, each asked for
correctness, design and idiomatic code against the plan.

- PR 1 went three rounds. Round 1 found the reload-before-stop kill (confirmed on a real user manager before fixing),
  data-directory checks that compared spellings rather than real paths, a retry exception that let unusable hosts
  through without proof nothing was running, a connected supervisor not run by its unit, and test waits that could race.
  Round 2 found a probe answering without the recorded identity being accepted, and a retry that could never finish once
  the program directory was gone. Round 3 found only a stale SPEC_impl.md sentence. All fixed. Declined: a rare internal
  bookkeeping race that every provisioning operation already shares (a run finishing before the helm has recorded its
  task can leave a stale entry; nothing user-visible was identified); and the symlinked program directory, which is an
  open question above.
- PR 2 went three rounds. Round 1 found that the success notice could be lost if the host left the list before the reply
  arrived, docs telling users to "stop" sessions when a stopped session's open tab still blocks, the browser test's
  cleanup disturbing a setting the suite's other tests rely on, a test that could not detect host changes before the
  confirm, and progress not shown inline like Update's. Round 2 found that, when retrying after a failed uninstall, a
  stale view of the earlier failure could cancel the notice the retry owes, and that the test did not force the row to
  leave the page before the reply. All fixed, inline progress included. Round 3 found only spec and docs wording points,
  now fixed, and reviewed PR 1's teardown wording fix too. Declined: the screenshots above.
