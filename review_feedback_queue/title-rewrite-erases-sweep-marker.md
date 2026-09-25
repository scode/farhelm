# Process-title rewriting erases the sweep's session marker

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

On a Mac or a Linux host without systemd, stopping or deleting a session can leave a Postgres/Redis/nginx dev server the
agent started running while Farhelm reports the operation complete.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0247-2b597e9-be5c` (audit of Area 1, process-tree kill sweep and teardown) as
`F15 / COR-TITLE`, tagged **possible**. Anchors and title: `procs.rs:1421-1423`, `procs.rs:39-45`, `procs.rs:157`,
`procs.rs:1966`, `service/sweep.rs:306-323` — The marker scan reads live process memory, so programs that rewrite their
process title erase their own marker

Once a process has left the pane's process tree (daemonized and reparented to init), the kill sweep can find it only by
reading its environment for `FARHELM_SESSION_ID`. The code and SPEC_impl describe that read as "the EXEC-TIME
environment and only that", identically on both platforms (procs.rs:39-45, sweep.rs:306-323).

That description is not accurate. On Linux, `/proc/<pid>/environ` (procs.rs:1421-1423) is a live view of the memory
region where the kernel placed the environment strings at exec, and the macOS environment region of `KERN_PROCARGS2`
(procs.rs:1966) is the same kind of view. Programs that set their process title commonly overwrite that region, because
the title space is extended into it. Examples:

- Perl and Ruby `$0=`
- Python `setproctitle` (gunicorn, celery, uwsgi)
- the nginx master
- postgres (`PS_USE_CLOBBER_ARGV`)
- redis

Two reviewers verified the Perl case: `env FARHELM_SESSION_ID=<uuid> perl -e '$0="x"; sleep 20'` shows its environ as
spaces with no marker, while the process's real environment and its children still carry the marker. If such a program
also daemonizes (`pg_ctl start`, `redis-server --daemonize yes`, nginx), neither the parent-pid walk nor the marker scan
reaches it, and children forked after the rewrite inherit the wiped region too. This is not the accepted residual of a
process that "deliberately scrubs its environment": these are ordinary dev servers.

SPEC says Stop terminates the agent's process tree including dev servers and "accidentally daemonized processes".
SPEC_impl says the sweep is the whole mechanism wherever there is no systemd user manager (containers, broken managers,
all of macOS). Stop and Delete then report success while the server keeps running.

The open premise is whether the maintainer counts this inside the accepted residual. The nginx and postgres behavior is
from source knowledge; the Perl case was verified. At minimum, correct the "exec-time environment" wording in procs.rs,
sweep.rs and SPEC_impl, and name title-rewriting daemons as a residual, especially for macOS. A real fix needs a
membership signal that title rewriting cannot erase: the cgroup where available, or capturing markers when a process is
first reached through the parent-pid walk. Add a test with a title-clobbering child.

For the user, on a Mac or a Linux host without systemd, stopping or deleting a session can leave a Postgres, Redis or
nginx dev server the agent started still running while Farhelm reports the operation complete.
