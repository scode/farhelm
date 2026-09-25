# UPDATE of a hand-started supervisor reports success on the old build

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

Updating a host whose supervisor was started by hand can show "completed" while the host keeps running the old version,
leaving a crash-looping service behind.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0602-2b597e9-f96c` (audit of Area 8, provisioning) as
`F3 / COR-UPDATE-HAND-STARTED-ATTACH`, tagged **definite**. Anchors and title: `provisioning/service.rs:1383-1422`,
`provisioning/plan.rs:408-433`, `provisioning/service.rs:642-713`,
`crates/farhelm-helm/units/farhelm-supervisor.service.in:5` — UPDATE of a supervisor not run by the unit starts a
crash-looping second supervisor and can report success while the old build keeps serving

SPEC.md treats a supervisor started by hand (`farhelm supervisor run` in a terminal) as a normal way to run a host.
UPDATE planning (`plan_update_unguarded`, service.rs:642-713) accepts any supervisor that answers the probe and never
asks what is running it — a hand-started process, a differently named unit, or `farhelm-supervisor.service` itself. The
plan (plan.rs:408-433) then writes `farhelm-supervisor.service`, runs `systemctl --user enable --now`, and later
`systemctl --user restart`. If the running supervisor is not that unit, `enable --now` starts a _second_ supervisor on
the same state directory. It cannot take the state-directory lock (the supervisor refuses with "a supervisor is already
running against …"), exits with failure, and `Restart=on-failure` restarts it in a loop. Because the unit is
`Type=simple` (unit template line 5), `systemctl` returns success as soon as the process is spawned, not when it is
serving.

The final attach step (service.rs:1383-1422) asks the helm's connection manager to reconnect and waits until the host is
`Connected`, has a client, and shows a new _incarnation_. The incarnation is a helm-side counter that changes on every
new connection, not a property of the supervisor, so reconnecting to the untouched old supervisor satisfies it. The run
is marked Completed. Nothing compares the connected supervisor's `build_version` to the release just installed, or
checks the unit's MainPID. If the old build speaks a different protocol, attach instead times out with a generic error
(see F17). The failing unit stays enabled, so it tries again at every boot, and each start attempt briefly violates
SPEC.md's "at most one supervisor per user per host".

The progress record is the user's only evidence of what happened, and here it says success while the host keeps running
the old version. Suggested change: when planning, compare the unit's `MainPID`/`ActiveState` with the answering
supervisor and refuse with an explanation when the supervisor is not that unit; after attach, require the connected
`build_version` to equal the installed release.

Restater note: whether the run reports success depends on timing. systemd's default start limit (5 starts in 10 s, with
100 ms between restarts) is likely to be reached within about a second of `enable --now`, and once it is, the plan's
later `systemctl --user restart` is refused ("start request repeated too quickly") and the run fails at the restart step
instead of lying. The false "completed" outcome is therefore possible rather than certain; the crash loop, the enabled
failing unit, and the missing version/ownership checks hold either way.

User-visible consequence: updating a host whose supervisor was started by hand can show "completed" while the host keeps
running the old version, leaving a crash-looping service behind.
