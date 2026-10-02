# Plans index

One line per plan, in the order the plans are executed. This file must always match the plan files in this directory.

- [executed] `triage-signin-status-paths.md` — execute the 17 triage outcomes decided on 2026-10-01 (sign-in recovery,
  Codex and Claude status reading, the Pi plugin rename, slow hosts, non-UTF-8 paths, transfer timeouts).
- [executed] `triage-feed-sink-identity.md` — execute the second batch of 2026-10-01 triage outcomes (event-feed
  keepalive, Delete waiting for the terminal reader, simpler duplicate-host handling, plus a `BUGS.md` entry and a
  `FILTER.md` filter).
- [pending] `triage-yolo-sighup-replace.md` — execute the third batch of 2026-10-01 triage outcomes (YOLO detection for
  custom launches and Cursor's switch to `cursor-agent`, the missing-host and adopt YOLO gaps, SIGHUP shutdown, and
  Replace keeping the confirmed "nothing alive" answer).
