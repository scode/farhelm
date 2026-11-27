# Report: CLI options stop swallowing the next flag, and the CLI parsing audit

## What this was about

Most of `farhelm`'s free-text options and arguments took the word after them as their value whatever it looked like:
they carried clap's setting that lets a value start with `-` (called the hyphen allowance below). So a forgotten value
quietly took the next flag. `farhelm agent host add dev --alias --tailscale` asked the user to add a host named
`--tailscale`, and the Tailscale option was dropped. A title forgotten before `--yolo` became a session titled `--yolo`
that lacked the YOLO assertion. #5129 had already fixed this for `--model` and `--effort`. The TODO entry asked for the
same treatment everywhere else, plus a sweep for places where Farhelm's command-line parsing departs from normal `clap`
behavior.

You decided:

- **Scope.** Take the hyphen allowance away from every identifier, name and path option (25 of them, on
  `farhelm agent`'s verbs and the launch flags), from `--wrapper`, `--command` and `--resume-command`, and from the nine
  free-text arguments (a rename's new title, template names, a host add's destination, a host edit's or remove's host,
  and `farhelm helm checkout-config set-root` and `set-post-clone`).
- **Exceptions.** Keep it on `--prompt`, so a message starting with a Markdown bullet or `---` still works. Keep it on
  two hidden internal commands that older callers rely on.
- **Audit cleanups.** Fix everything else the audit found:
  - read `--prompt-file` after parsing;
  - give template writes their own flags, with an explained `--template` refusal;
  - restore `--help` on the custom-agent reporter;
  - have clap check spawn's launch rule, the absolute paths two internal commands take (the agent-screen development
    tool and the remote-host cleanup command), and the template key and title kind;
  - align spawn's help labels;
  - drop an unused clap feature.
- **Left as they are.** The hidden flags kept only to refuse removed options with an explanation (`--profile`,
  `--confirm-yolo` and the like), `farhelm agent help`, the runtime check that a host edit changes something, and the
  installer's own `--expect` check.

Two PRs build it.

### What changed for someone using Farhelm

- **A forgotten value is now a usage error.** On `farhelm agent`, `farhelm spawn` and the two checkout-config commands,
  it fails at once (exit status 2) instead of taking the next flag.
- **New spelling for values that start with `-`.** Such a value is now joined to its option with `=` (`--title=-draft`).
  An argument that starts with `-` goes last, after `--` (`rename ... -- -draft`). A bare `--prompt-file -` still reads
  standard input.
- **Where agents learn this.** The instructions Farhelm gives agents say so. So does the help of the verbs where clap's
  own error would suggest the wrong fix (see below).
- **`--prompt` is unchanged.** It still takes a message that starts with `-` as a separate argument.
- **`--prompt-file` is read only once the whole command has parsed.** A mistyped flag next to `--prompt-file -` now
  fails at once instead of first waiting for input on stdin. A file that cannot be read, or whose contents are refused,
  now exits 1 instead of 2. The error names the file.
- **The custom-agent reporter prints its help again.** `farhelm custom-agent report-conversation --help` (or `-h`) with
  nothing else after it prints the help. Every other form, `--id --help` included, stays silent as the custom-agent
  contract promises. The contract page's changelog records the carve-out, and SPEC.md's sentence on the reporter now
  names it.
- **Template writes.** `farhelm agent template create` and `edit` now refuse `--template` as a usage error (exit 2
  instead of 1), with the same explanation as before. `--key` and `--title-kind` now list their choices when refused.
- **Spawn.** `farhelm spawn` with neither `--inherit-agent` nor a launch flag is now a usage error (exit 2 instead of
  1). Its help names the usual ways to describe the child's launch.

## Things you should know

- **Scripts or agents with old spellings.** One that passed a value starting with `-` as a separate argument (for
  example `--title -draft`) now gets a usage error and must use `=` or `--`. I found no caller in the repository that
  does this. The agent instructions already taught `=` and `--` for rename titles, so agents following them are
  unaffected. The changelog fragment says so for the release notes.
- **clap's own hint is wrong for options on some verbs.** On a verb that also takes an argument (rename, template create
  and edit, host add and edit), clap's refusal of `--alias -x` suggests "use `-- -x`", which does not work for an
  option. I could not change clap's message. Those verbs' `--help` now ends with a line giving the right spelling, and
  the agent instructions say it too.
- **The Mac forwarder.** On a Mac, `farhelm` runs through a small forwarder script that the installer writes. It now
  lets the reporter's `--help` output through. That only takes effect once a Mac is updated with this release's
  installer.
- **One check is stricter than the plan said.** The plan asked for a debug-only check that `--prompt-file` was read
  before a launch is built. I made it a check that also runs in release builds. Otherwise a future command that forgot
  to read the file would launch without the user's message instead of failing.
- **A guard against regressions.** A test now walks the whole command line definition and fails if any option other than
  the listed exceptions takes a separate value starting with `-`. So a new option added with the hyphen allowance is
  caught.
- **Not mine, but noticed.** On current `main`, `cargo clippy -- -D warnings` fails on an unused method in the helm's
  notification settings code (`save_ntfy_config`). It came from other work and this plan does not touch it. I linted the
  CLI crate without `-D warnings` and read its own warnings, of which there were none.

## Open questions and possible follow-ups

- **The `--` hint on positional verbs.** It could be removed entirely by giving those options a custom error, but that
  means post-processing clap's errors. I recommend leaving it with the help line, as now.
- **The clippy failure on `main`** (above) needs a fix from whoever owns the notification settings work; it is outside
  this plan.

## PRs

- #5163: fix: refuse cli options whose value is missing.
- #5165: fix: clean up cli parsing quirks (also removes the TODO entry).

## Checks

Run now:

- The CLI crate's tests, the agent, spawn and checkout-config CLI tests and the terminal-host stop command's tests, on
  the final stack
  (`cargo nextest run -p farhelm --bins --test agent_cli --test spawn_cli --test checkout_config_cli --test terminal_host_stop`):
  238 of 238 passed (recorded run 09fee562-2a4e-4ed8-ab62-3ef8fe856868). On #5163 alone, the same selection without the
  stop tests: 220 of 220 passed (recorded run 7d2683c8-0410-4f54-bc9d-d87113cb6ee8).
- `cargo clippy` with all targets for the CLI crate, run without `-D warnings` because that form fails on `main` for the
  unrelated reason above; the CLI crate itself produced no warnings. Also `cargo check` of the fixtures crate (the other
  clap user), `cargo fmt --check`, `dprint check` on the changed Markdown,
  `python3 releasing/check-changelog.py format`, `sh -n` and `shellcheck` on the installer scripts, and the test-sleep
  check: clean.
- The docs website build (`bun run build` in `website/`), for the changed custom-agent contract page: built, with all
  internal links valid.

Reused:

- The end-to-end custom-agent reporter tests (`cargo nextest run -p farhelm --test e2e -E 'test(custom_agent)'`): 3 of 3
  passed on #5165 as it was before its review fixes (recorded run b1c7df22-9f5e-466b-b6c2-fdfff974aa1b). The fixes since
  change only error wording, a check that now also runs in release builds, help text and comments; none touches the
  reporter's path.
- The macOS installer and forwarder script test under its Linux shims
  (`bash scripts/test-install-sh.sh --farhelm target/debug/farhelm`): 557 checks, 0 failed, including the four new
  forwarder checks, on #5165 before its review fixes (recorded run 5bbc4333-9210-4bae-8806-2155a0450708). Since then
  only a comment outside the forwarder's text changed in the installer.

Skipped:

- The rest of the workspace tests, the desktop, GUI and other batteries. The code change is confined to the `farhelm`
  CLI's argument parsing and the installer's forwarder, which the tests above cover; the other edits are SPEC.md,
  SPEC_impl.md, the custom-agent contract page and TODO.md, covered by the formatting check and the website build.

## Review gate

Each PR had a fresh-context Opus 5.5 review at high effort, with the plan's charter and the test-authoring checklist.

- **#5163:** one round. No high or medium findings. Two lows were fixed:
  - my comment and spec text gave a wrong technical detail about clap's error for `--prompt` (your reason, Markdown
    bullets, stands). The detail that matters: clap's misleading "use `--`" hint appears on verbs that take an argument,
    and now for their options too, hence the help line;
  - the `=` spelling was tested for only some options; it is now tested for every one, plus the structural test above.
    Five nits were fixed too: the instruction wording, help text that showed a code reference, the changelog wording,
    the set-root help, and a list of affected options kept by hand in a test (now backed by the structural test).
- **#5165:** one round. No high or medium findings. Three lows were fixed:
  - a `--prompt-file` error had stopped naming the file;
  - the debug-only check, now a release check;
  - a stale installer comment. Six nits were fixed too, among them a new test that runs `farhelm` with a stdin that
    stays open, proving a mistyped flag no longer waits on it.
