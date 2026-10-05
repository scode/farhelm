# A failed uninstall can be reported as successful after another client forgets the host

Reviewed commit: 6943f94c24a193bad9433e718bbe4e1fb6b237dc

## TLDR

A failed uninstall can be reported as successful after another client forgets the host.

## Details

`F55 / COR-UNINSTALL-FORGOTTEN-SUCCESS` — **definite** — `crates/farhelm-ui/src/hosts.rs:1074` — A failed uninstall can
be reported as successful after another client forgets the host

A window can announce that Farhelm was uninstalled even though the uninstall failed and another window merely removed
the host from the helm's list. The confirming window prepares its success notice before submitting uninstall, then shows
that notice whenever the host disappears. It normally withdraws the notice after reading a failed run, but that failure
is read by a component owned by the host row.

If uninstall fails, its host lock is released. A second client can then use Remove to forget the host before the first
client receives the failed progress response. Remove deletes the helm's retained run, and the disappearing row unmounts
the first client's progress reader. The first client never learns the failure and interprets the absent host as
successful software removal. Farhelm files or services can remain despite the success announcement. This sequence need
not happen within a sub-second click window; a delayed progress read is enough.

Show success only from completion evidence correlated with that uninstall, retained outside the deleted host row. If the
host disappears without that proof, report an unknown outcome. A focused two-client scenario should hold the confirming
client's progress response, fail uninstall, and then Remove the host from the other client. The specification promises
that a failed uninstall keeps its progress available for continuation; this false success report is the review filter's
stated exception for misleading success.

Suggested bucket: high

Possible cover: none. Remove in the same client clears that client's pending notice, but Remove in another client cannot
clear it. The TODO entry about symlinked program directories during remote uninstall is not a Planned disposition for
this finding and concerns what uninstall removes, rather than proof that it completed.

Caveats: No runtime reproduction was performed. Remove happens after the failed run releases its lock; Remove while
uninstall is running is correctly refused. No additional deletion or process loss is claimed.

## Review context

Review: seven-day main diff `7cc0681495159e4a6abe543dffdff1acc0720675..6943f94c24a193bad9433e718bbe4e1fb6b237dc`,
correctness and security only, nofix.

Confidence as retained: **definite**. Suggested queue bucket: **high**.

Originating reviewers and passes: `ui_systems p3`.

Possible cover recorded during collection: none; same-client Remove clears own notice but another client does not.
TODO67–70 is not Planned and concerns host commands rather than proof of uninstall completion..

Collection caveats: No runtime reproduction. Remove occurs after failed run releases lock; Remove while running is
correctly refused. No extra deletion or process loss claimed.

## Filed reviewer metadata

- `ui_systems p3`: confidence as filed: definite; confirmed by code and the allowed cross-client sequence. No runtime
  reproduction was run. Suggested bucket as filed: high (material false success for uninstall).
