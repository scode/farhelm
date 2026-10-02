# Plans index

One line per plan, in the order the plans are executed. This file must always match the plan files in this directory.

- [executed] `triage-signin-status-paths.md` — execute the 17 triage outcomes decided on 2026-10-01 (sign-in recovery,
  Codex and Claude status reading, the Pi plugin rename, slow hosts, non-UTF-8 paths, transfer timeouts).
- [executed] `triage-feed-sink-identity.md` — execute the second batch of 2026-10-01 triage outcomes (event-feed
  keepalive, Delete waiting for the terminal reader, simpler duplicate-host handling, plus a `BUGS.md` entry and a
  `FILTER.md` filter).
- [executed] `triage-yolo-sighup-replace.md` — execute the third batch of 2026-10-01 triage outcomes (YOLO detection for
  custom launches and Cursor's switch to `cursor-agent`, the missing-host and adopt YOLO gaps, SIGHUP shutdown, and
  Replace keeping the confirmed "nothing alive" answer).
- [executed] `triage-restart-takeover-update.md` — execute the fourth batch of 2026-10-01 triage outcomes, the
  high-priority review queue (Restart keeping the session selected, the stuck page lock, takeover-safe tab attaches,
  Update refusing to downgrade "too new" hosts, Replace and probe finishing after a dropped request, and smaller fixes).
- [pending] `triage-confirm-ssh-identity.md` — execute the fifth batch of 2026-10-01 triage outcomes (confirmations bind
  to what they showed, Farhelm's ssh connections never forward or run a `RemoteCommand`, report-only conversation
  identity in the spec, helm-side pacing of host hints, Claude's resume selector refusal, and small gated fixes and
  discards) (after `triage-restart-takeover-update.md`).
- [pending] `triage-clipboard-terminal-limit.md` — execute the 2026-10-02 highest-priority triage outcomes (a bound on
  the desktop window's pending clipboard writes, a hard size limit on terminal data from a supervisor, and removing the
  deferred filesystem-fallback queue item).
