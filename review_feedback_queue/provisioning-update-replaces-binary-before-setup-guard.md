# A remote update can replace a host's farhelm binary after `farhelm helm setup` has taken the host over

Reviewed commit: de774a1ee8815ce833da77deac593a55d82f7be3

## TLDR

When you update a remote host from the hosts panel, Farhelm replaces the host's farhelm program first. Only afterwards
does it check whether `farhelm helm setup` has since taken over that host's supervisor service. Suppose you switch the
host to setup while the update is queued or downloading. The update then overwrites the program your setup services run,
and only after that stops with "refusing to replace a unit managed by farhelm helm setup". Your setup-managed services
switch to the helm's version the next time they restart. The replaced program is not kept.

## Details

Source: whole-codebase review, 2026-09-30. A slice reviewer dropped this as a seconds-long window without a cited basis;
an independent checker found that the spec designs against this interleaving, and it was traced and written up
separately. Traced by reading code and specs; not reproduced. A rare race that the operator has to cause, and the run
fails visibly.

- **Step order.** The plan is frozen as EnsureDirectories, then Upload, then `InstallPayload` for farhelm
  (`crates/farhelm-helm/src/provisioning/plan.rs:404`) and for tmux (`plan.rs:435`), then `WriteUnit` (`plan.rs:450`),
  then daemon-reload, enable, linger, and restart. The executor runs them in order
  (`crates/farhelm-helm/src/provisioning/service.rs:1265-1287`).
- **The marker check covers only the unit write.** The `refuse_setup_managed` guard
  (`crates/farhelm-helm/src/provisioning/backend.rs:830-844`) is enabled only by `install_bytes` for unit content
  (`backend.rs:2130`). A local payload install passes `false` (`backend.rs:2094`). The remote payload install,
  `install_uploaded_source` (`backend.rs:946-1000`), has no guard: it checks the digest and does `mv -f`.
- **Nothing between planning and execution re-checks the marker.** The marker test runs only in `inspect`
  (`backend.rs:1925-1990`), which is called at planning (`service.rs:797-809`). Revalidation for UPDATE
  (`service.rs:1085-1150`) re-checks the registration, update trust, and supervisor identity through `probe`, but never
  runs `inspect`. SPEC_impl.md:2643-2648 acknowledges the race ("a retained plan is confirmed later and setup may have
  run on the host in between") but puts the fix only on the unit write.
- **What gets replaced.** For UPDATE, `farhelm_path` is the binary the helm actually connects to. Planning records the
  path the probe resolved (`service.rs:784-786`), and `plan_for_row` uses it (`plan.rs:493-527`). Otherwise it is
  `~/.local/lib/farhelm/farhelm`. Setup owns only units (`crates/farhelm/src/setup.rs:1-16`): its units point at
  whichever binary the operator ran (`SetupContext::exe`, `setup.rs:47-51`). So the replaced binary is the one setup's
  units run only if the operator ran setup with that same binary. That is likely in practice (`farhelm` on PATH is
  usually what the helm connects to), but it is not guaranteed.
- **Prerequisites.**
  - Setup won't overwrite an unmarked unit (`existing_managed_text`, `setup.rs:1060-1085`). On a host provisioned from
    the panel, the operator must first move the provisioning unit aside and then run setup. The only other case is a
    host that had no unit under that name.
  - ADD has the same order. But ADD's revalidation switches to using a supervisor as-is when one answers
    (`service.rs:1004-1080`), and its binary goes to the lib directory, so the overlap there is smaller.
- **How long the window is.**
  - Plans are kept in memory with no time limit, evicted only after 64 newer plans (`MAX_PENDING_PLANS`,
    `service.rs:37`; `retain_plan`, `service.rs:893-903`) or when the helm restarts.
  - Per SPEC_impl.md:2643-2644, the panel consumes an UPDATE plan right after the Update click.
  - After that, the run still waits for the host's provisioning lock (`service.rs:926`) and one of 4 run slots
    fleet-wide (`service.rs:38`, `935-940`). "Update all" can queue many hosts behind those slots. Then it downloads the
    payloads (`prepare_payloads`) and uploads them. In practice that is seconds to minutes. A client that holds a plan
    and consumes it much later is possible over the wire; not checked whether any client does.
- **Consequence.**
  - The binary is swapped atomically, so running processes keep the old one. After the swap, `WriteUnit` fails with exit
    77 and the run is reported as failed. The earlier steps show as completed. Nothing is rolled back and there is no
    backup.
  - When setup's helm and supervisor units next restart, they run the helm's release version. If that is older than what
    the operator installed, this is a downgrade; not checked whether a downgraded binary would refuse newer state.
  - If the path is the installer's `~/.local/bin/farhelm`, the installer's ownership record
    (`scripts/install.sh:255-275`, `417-430`) no longer matches the binary. That also happens on any UPDATE to an
    installer-placed registered path, with or without setup, so it isn't specific to this race.
  - The tmux payload goes to the lib directory, which setup normally doesn't use.
- **What the spec promises.**
  - SPEC.md:1655-1660 promises that provisioning won't touch the supervisor unit "both when planning and at the moment
    of writing". That promise is kept.
  - It says nothing explicit about the binary. On a setup-managed host, the planning refusal means provisioning normally
    touches nothing, and this race breaks that.
  - SPEC.md:319-331 ("Concurrent and interrupted runs") forbids "replacing a file the command does not own". But it
    lists the installer, `farhelm helm setup` and `farhelm uninstall`, not panel provisioning. Whether it applies here
    is a judgment call for triage.
- **How to verify.** Write a backend test with a remote or e2e fixture: plan an UPDATE, then write a unit whose first
  line is setup's marker, then consume the plan. Assert that `install-farhelm` completed and replaced the binary's
  bytes, and that `write-unit` failed with the setup refusal.
- **Fix shape.**
  - Re-run the unit-marker test at the start of execution: in revalidation, under the host provisioning lock, before the
    first change to the host.
  - Also put the same `head -n 1` unit-marker test into the payload install's rename command, as the unit write already
    does. That shrinks the window to a single remote command.
  - Fully closing the race needs host-side coordination with setup. When this was rebased onto main at 1ec60cc, the spec
    sentence that said such coordination was not built yet had been replaced by c4974fc: uninstall now takes setup's
    unit-directory lock (SPEC.md:319-324 at 1ec60cc). Remote provisioning still takes no such lock, so the race stands;
    having provisioning take that same lock is a possible fix, not something the spec currently requires.
