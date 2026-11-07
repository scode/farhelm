# Failed startup can leave the capture process running

Reviewed commit: 407a49435278844248d573083fdea6e23fd96c02

## TLDR

A screen-capture run can fail after launching its agent but before returning the run handle, leaving the detached
capture process and its terminal host running without the caller cleaning them up.

## Details

Source coordinates from the original records: `scripts/agent_screen_host.py:119-134`;
`scripts/capture-agent-screens.py:176-183,252-254,722-725`. References below use these paths' basenames unless another
path or revision is stated.

Recorder disposition: **verified**. Triage priority: **other**. Source-confirmed conditional behavior does not imply an
observed incident. Reviewer suggestions, narrower refutations and unresolved premises below remain part of the finding.

New separate-discovery finding, introduced by 2d5859c5f144e0c286a0576976aae2d1c66ddc7c and unchanged through tip.
HostRun.start creates a detached controller with start_new_session=True (agent_screen_host.py:114-117), consumes its
ready line and closes stdout (119-123), then writes directory/pid (133) before returning the handle (134). That metadata
write can raise OSError, e.g. if /tmp runs out of space or quota after the preceding files were created. There is no
exception cleanup guard around this post-spawn ownership interval; the only run.stop call is the explicit no-ready
branch at 123-132, which this successful-ready path bypasses. A KeyboardInterrupt in the same post-ready interval
likewise bypasses it; do not generalize to every earlier interruption, because a broken ready pipe can instead trigger
Rust shutdown.

This is not rescued by either supported caller. capture-agent-screens.py:176-183 leaves Terminal.run as None until
HostRun.start returns. Its finally at 722-725 calls close, whose 252-254 body does nothing when run is None.
agent-tui.py:62-63 likewise only receives and prints the handle after the call returns; its main only catches HostError
(312-318), not this OSError/KeyboardInterrupt. The backend has already adopted the host before printing ready
(agent_screen.rs:100-127). After ready it waits for control requests/signals (128-147); it does not monitor
parent/ready-pipe lifetime. Rust shutdown at 104-109 occurs only after that serving loop returns. Python Popen
destruction does not stop its process: local /usr/lib/python3.12/subprocess.py:1120-1134 polls and retains a still-live
child, and _internal_poll:1973-2008 only waitpid-polls it. Thus the source establishes the ownership leak conditional on
the metadata failure after ready, without requiring a failed stop. No actual process lifetime, vendor spend or data loss
was measured. The run directory generally remains available for manual recovery; do not claim it was deleted or that no
recovery is possible.

Fix should retain exception-safe ownership immediately after Popen and stop the owned run on any failure before
successful handle transfer, preserving/rethrowing the original error and keeping failure evidence as appropriate. A
focused injected pid-write failure after a successful ready handshake can verify cleanup without real vendor turns.
Suggested bucket other: maintainer tooling stranded resources, no catastrophic or highest consequence established.
Confidence confirmed source-level exception/ownership route; no runtime experiment performed. proposed_drop false.
possible_cover none: g012/l3 and g047/f2 concern ignoring a stop attempt's failure and deleting recovery state, whereas
here no cleanup attempt is made and the caller never receives ownership. SPEC_impl.md Testing:7543-7557 accepts
single-user/PID/concurrency assumptions, not bypassing cleanup after a startup metadata failure; its capture contract at
4307-4313 does not accept this either.

Review provenance: trailing-review run `7c7c7b55-12fc-4573-b10e-01d690187f50`; original records `g047/d1`, `g048/f12`.
The retained append-only histories preserve all earlier claims and corrections. Reviewed source references name their
own revisions; the inclusive reviewed tip is the hash above. This review used source inspection only, with no product
runtime experiments or live-install changes.
