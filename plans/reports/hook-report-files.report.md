### What this was about

When an agent starts a conversation, or switches to a new one with `/clear` or `/new`, it tells Farhelm which
conversation it is in, so that Restart later resumes that conversation and not one the agent already left. Until now the
agent's hook did that by calling the host's supervisor and waiting for an answer. When no supervisor was running — on
the Mac, whenever the Farhelm app is closed, because the app runs the Mac's supervisor — the hook retried for about four
seconds, holding up the agent at startup (and on every turn for Grok and Pi, which report each turn), and then gave up.
The report was lost, and the next Restart could resume the discarded conversation.

You decided in planning that hooks should instead leave their report as a file that the supervisor picks up, so that
tracking keeps working while the supervisor is down and all the retry and timeout machinery goes away, and that an
agent's own `farhelm spawn` and `farhelm agent` commands failing while the supervisor is down is accepted for now and
written into the spec. That is what landed:

- The hook now saves the report as a small file in the supervisor's state directory and exits at once, whether or not a
  supervisor is running. Each session keeps only its newest report. Grok is the one exception: its `SessionStart` hook
  names the conversation, and its hooks on every later prompt only add details (the path of its saved record), which
  Farhelm accepts only for a conversation already named. So Grok keeps the naming report and the newest later one in
  separate files, and a run of later reports during an outage cannot push the naming one out.
- The supervisor applies waiting reports on its existing pass every two seconds, when it starts, and before a Restart
  chooses the conversation to resume. A report made while it was down is applied when it comes back.
- Because a file cannot say which process wrote it, the hook records the chain of processes it was started by, and the
  supervisor checks that chain the way it used to check the live caller: the report must come from the session's current
  terminal (which also discards a report left over from before a restart), and for Claude, Codex, Grok, and OMP from the
  agent itself rather than a sub-agent or a child it started.
- SPEC.md now says that conversation tracking keeps working while a host's supervisor is down, and that the agents'
  `farhelm` commands fail until it is back. The website's agent pages and "Your first session" say the same, and the
  TODO entry "Verify that agents keep running while Farhelm is closed on the Mac" is removed, as the plan said to. Its
  remaining part, checking the behavior on a real Mac, is the manual check under open questions below; nothing else
  tracks it, so if you want it tracked beyond this report, ask for a follow-up or a new TODO entry.

### Things you should know

- **The hook log gained a second writer.** A person debugging a session used to see `acked` or `refused` in the
  per-session hook log, written by the hook from the supervisor's answer. The hook now writes `written` when it saved a
  report, and the supervisor appends its own `acked` or `refused <kind> <reason>` line when it judges the report. A
  report that a newer one replaced before the supervisor looked gets no verdict line, which is normal after an outage.
  This was not in the plan; without it, the reason a report was refused would only be in the supervisor's log.
- **Reports now arrive up to two seconds late** while the supervisor is running (the next pass), instead of at once. The
  resume offer for a new Claude session appears within a few seconds of launch rather than immediately. Faster pickup is
  your existing Near term TODO entry "Pick up hook report files immediately".
- **A report made before Farhelm has finished starting a session waits** until the session shows up in the session list,
  then is applied on the next pass. Before, such a report was recorded straight away. Nothing visible changes except
  that the resume offer can take a pass longer to appear.
- **Every agent's report must now come from the session's current terminal**, Goose and Pi included (they had no process
  check before). Their reporters run under the agent, so this only stops a leftover report from an earlier launch, which
  the plan asked for.
- **A report that names a sub-agent is now dropped by the hook itself** (it logs `bad-payload subagent-report`), because
  writing it would replace the session's own pending report before the supervisor refused it.
- **Accepted gap:** a second agent of the same kind started inside a session, without a sub-agent marker, can still
  replace the session's pending report before the supervisor refuses it, if it reports within the same two-second pass
  or while the supervisor is down. The session's real switch is then lost, and Restart resumes the conversation it had
  before the switch, until the agent's next report corrects it. In practice this is Grok, whose hooks are configured for
  every Grok you run: a separately started `grok` inside a Grok session. Claude, Codex, Pi, and OMP children do not get
  Farhelm's hook. The old way of reporting (calling the supervisor directly) had no such window while the supervisor
  ran; during an outage the report was lost anyway. This gap was not discussed in planning: I accepted it as a narrow
  case and documented it in SPEC_impl.md's "Report files". It is yours to confirm, or to reject with a follow-up (an
  option is under open questions).
- **No protocol version bump.** The two socket messages for reporting were removed without one: only the hook ever sent
  them, and a helm and a supervisor never exchange them. During an update, a new hook writing files for a supervisor
  that is still the old version (the Mac between installing and restarting the app) loses nothing: the files wait and
  the new supervisor applies them when it starts. The other way round, a hook still running the old binary against a new
  supervisor, loses that one report; that window lasts only while a hook process started before the new binary was
  installed is still running. You accepted this gap in planning.
- **Restart now waits** for a report pass that is already running before it chooses the conversation, so a report on
  disk cannot be skipped and then discarded as belonging to the old launch. A pass takes milliseconds normally, and a
  report that cannot be applied yet is put back rather than retried within the pass, so this wait is bounded by one
  pass. Ordinary session listings do not wait.
- **Reports are applied inside the supervisor's two-second pass that also refreshes each session's status.** A report
  that is slow to check (Codex reads the conversation's record file) delays that pass, and so the status shown for the
  host's sessions, by as long as the check takes, normally milliseconds. A reviewer pointed this out; it was left as is.
- **A report the supervisor cannot apply because it cannot read its own records or tmux is kept and tried again every
  pass until that clears**, with no limit, since those are faults in the host, not in the report. It warns once per
  session, then logs quietly.

### Open questions and possible follow-ups

- **Manual Mac check (you asked for this; it needs a Mac).** It shows that a session keeps running with the app closed,
  and that a conversation switch made while the app is closed is what Restart resumes. With the app closed there is no
  window to type into, so the steps attach to the session's terminal directly through Farhelm's private tmux server: The
  commands use Farhelm's state directory, by default `.local/state/farhelm` under your home directory, and the same tmux
  the app runs sessions with (Homebrew's, for most Macs).

  1. With no other sessions running, start one Claude session in the app and send it a prompt containing a marker word,
     say `ALPHA`. Wait for the reply. The session's id is the name of the newest file in the state directory's
     `hook-log` directory (`ls -t` lists it first), `<session id>.log`.
  2. Quit the Farhelm app. In a terminal, run `tmux -S <state directory>/tmux.sock ls` and confirm the session
     `fh-<session id>` is listed, then attach with `tmux -S <state directory>/tmux.sock attach -t fh-<session id>`. The
     agent is still running.
  3. In the agent, run `/clear`, then send a prompt with a different marker, `BRAVO`, and wait for the reply. In another
     terminal, check `hook-log/<session id>.log`: the newest line is `written` with a new conversation id, and there is
     no `acked` after it yet; `hook-reports/<session id>/latest.json` exists.
  4. Detach without exiting the agent, from another terminal:
     `tmux -S <state directory>/tmux.sock detach-client -s fh-<session id>`. Then reopen the app. The same session is
     there and still running. Within a few seconds the hook log gains an `acked` line for the new conversation id and
     the `hook-reports/<session id>/` directory is empty.
  5. Stop the session and restart it from the app. The resumed conversation knows `BRAVO`, not `ALPHA`.

  If step 4 shows a `refused` line, its detail says why the report was turned down. If step 2 lists no session, the
  agent did not survive the app closing, which is a separate bug. Your release checklist `docs/manual-mac-checklist.md`
  could carry these steps if you want them run at every release candidate.
- **Follow-up option for the accepted gap:** if nested Grok runtimes reporting into a session turn out to matter, the
  hook could keep one slot per reporting process, or check its own chain against the pane before writing.

### The PRs

- #1585 refactor: split hook attribution into collecting and anchoring a chain — the process check in two halves, so the
  hook can record the chain and the supervisor can check it later.
- #1594 fix: keep conversation reports made while the supervisor is down — hook writes files, supervisor applies them,
  socket reporting removed, specs, changelog fragment, tests.
- #1595 docs: update the website for conversation reports saved as files — agent pages, "Your first session", TODO entry
  removed.

### Checks run, reused and skipped

- Run in this session, on the final code (before rebasing onto the latest main): `cargo nextest run` over the supervisor
  library, the `farhelm` binary's unit tests, and the end-to-end files for hook identity, Codex identity, wrapper
  launches, restart with resume, and session rename (run `dc6c8dfc`, 1243 passed); earlier, a wider selection that also
  covered the protocol crate, the fixtures, session lifecycle, agent relay, create idempotency, boot-id outcome, and
  attachment uploads (run `05e859b3`, 1555 passed). `cargo fmt --all -- --check`,
  `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`, `dprint check`, the
  test-sleep checker (0 unannotated delays), `python3 releasing/check-changelog.py format`, and the website build (all
  links valid).
- Reused after rebasing onto the latest main: the test runs above. What landed in between (hover tooltips, a Codex
  launch spec clarification, docs and planning changes) touches no hook, report, process-check, or supervisor
  reconciliation code, and no Rust crate other than the UI changed; clippy, dprint, and the website build were re-run on
  the rebased stack.
- New tests: a report written while no supervisor runs is applied when one starts; Grok's selection survives two later
  reports made while the supervisor is down; a report from an earlier launch is discarded after a restart; plus unit
  tests for the report file's format and size limit, the separate Grok files and the order they are applied in, files
  that cannot be read, Restart waiting for a running pass, and the check that a report is only recorded into the launch
  it was checked against.
- Skipped: the browser end-to-end suite and desktop checks, which exercise the UI and the app's window, not reporting;
  the supervisor inside the Mac app is the same code as the one tested here, but nothing in this run executed on macOS,
  which is what the manual Mac check covers; the Pi/OMP asset scenarios (the assets did not change; they still spawn the
  same hook command), the installer and CentOS provisioning checks (unaffected), and the real-agent capture tests, which
  need real vendor agents and are run on request.

### Review gate

Each PR was reviewed by Claude Opus 5.5 and gpt-6-astra, both at high effort, as you required. PR1: astra found nothing;
Opus raised nine points and then five more on the revision, all fixed (chiefly keeping the reason a process check
failed, and validating chains that come from files). PR2: astra raised five and then two more, and Opus eleven and then
seven, all addressed except two deliberate exceptions, both described under "Things you should know": applying reports
inside the two-second status pass can delay it (left as is), and reports kept back because the supervisor cannot read
its own records or tmux are retried without a limit (warnings limited instead). The fixes included Restart waiting for a
running pass, a race where a restart between checking a report's terminal and recording it could record the old launch's
report into the new one, sub-agent reports overwriting the session's own, and the order of Grok's two kinds of report
during a pass. PR3: astra three and Opus eleven, then four more, all applied.
