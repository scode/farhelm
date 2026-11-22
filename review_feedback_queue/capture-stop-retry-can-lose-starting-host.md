# The interrupted-start retry can declare success before host creation settles

Reviewed commit: 4c18498dd4eab58a550294bf5b3e73852419f9e0

## TLDR

Retrying cleanup after an interrupted capture start can remove the recovery directory before the host finishes starting.

## Details

**other.** Source inspection.

4c18498dd4eab58a550294bf5b3e73852419f9e0 scripts/agent_screen_host.py:107–150,214–256;
crates/farhelm-supervisor/src/agent_screen.rs:98–127,154–200;
crates/farhelm-supervisor/src/hosts/stop.rs:208–217,282–312,355–372,398–402;
crates/terminal-host/SPEC.md:86–115,144–163. Compared with parent bfb0d34c216384bf70f09c2872de1132ab072cdf
scripts/agent_screen_host.py:104–134,198–231. Present at pinned 1a53d9d35889d7dde96e31320cc1681dd013737d.

Stopping a retained interrupted start can erase recovery files while that run's own host may still start. The parent
only cleaned its newly started controller on a non-ready result after constructing the child handle, and discarded the
directory regardless of the fallback verdict. The reviewed patch improves the owned-handle path and explicitly
advertises retry when interruption occurs before Popen returns a handle, but its marker proves only directory
provenance. With neither a handle nor a saved process number, and before the controller binds its control socket, retry
skips controller settlement. An empty one-time host scan can return success, after which the directory is removed. If
the host binds after discovery and the socket is removed before adoption, adoption fails ahead of normal controller
shutdown; the detached host has no idle expiry. The CPython 3.12 subprocess constructor path cited in the initial
finding permits a child to survive interruption before handle assignment. A retry must establish that the controller and
any in-flight host starter cannot create or publish another host before treating discovery as final. A stable identifier
that exists only after readiness is too late for this ownership handoff. This differs from the older
capture-stop-controller-identity case, which has a saved but reusable identity and can hit unrelated work, and from the
capture-stop-directory-marker candidate, since this directory is genuinely created by the tool. The reviewer suggested
other for loss of recovery for the run's own detached process. Confirmed conditional source ordering, with no observed
incident. ; older ownership case is distinct.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as
`capture/capture-start-retry-without-pid`.
