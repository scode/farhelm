# Newer hosts get downgraded by Update / "update all", and the supervisor can stop running

Reviewed commit: de774a1ee8815ce833da77deac593a55d82f7be3

## TLDR

The hosts panel offers Update on every remote host, and "update all" sends it to every host at once. The helm never
checks whether the host already runs a newer Farhelm than the helm itself. On such a host, Update replaces the newer
program with the helm's older one and restarts the supervisor. When the newer release changed the supervisor's database
format, the older program refuses to open that database. The supervisor then keeps failing to start, the host goes
unreachable, and the newer program is gone with no copy kept. The run does report failure (the attach step times out).
Recovery means reinstalling the newer version on the host by hand.

## Details

Source: gap-filling review pass, 2026-09-30, slice helm-actors.

Reviewer's confidence: confirmed (traced end to end in code; not reproduced).

Reviewer's bucket suggestion: high.

Related earlier decision for triage to weigh: during the first review pass the same session's fresh-context drop checker
agreed to drop a narrower candidate ("UPDATE from an older helm downgrades a newer supervisor") on the basis of
SPEC.md:1718 and SPEC_impl.md's "exactly what the provisioning helm expects", judging it a deliberate user action with a
recoverable, unavailable host. This finding adds that "update all" sweeps such hosts in without the user choosing them,
that the newer binary is not kept, and that recovery needs a manual reinstall, so under the queue's rule that an unclear
cover is not a cover it is recorded.

Possible cover for triage to check: SPEC.md:1718 ("Supporting a range of historical data schemas and hardening every
upgrade/downgrade path are not current design goals"). That sentence is about schema compatibility, not about an Update
action that downgrades on its own. TODO.md's "Take a pre-upgrade backup of the on-disk state so a release can be rolled
back" would make recovery easier but does not cover the trigger. No queue or ledger entry found.

- **Nothing on the helm side checks direction.** `plan_update_unguarded`
  (`crates/farhelm-helm/src/provisioning/service.rs:723-839`) ignores the probed `build_version` (the `..` patterns at
  741-746 and 779-783). `revalidate` (1085-1150) ignores it too. The payload is always this helm's own release version
  (SPEC_impl.md, ReleasePayloadSource).
- **The UI offers Update on any remote row.** `update: !run_active && update_allowed && !local_setup && !plan_in_flight`
  (`crates/farhelm-ui/src/provisioning.rs:2155-2167`), where `update_allowed` is only the host kind. "Update all"
  (`available_remote_updates`, `crates/farhelm-ui/src/hosts.rs:226-247`) takes every row that shows that offer. The helm
  already works out build age for the `old version` advisory (SPEC.md:746), so a "host is newer" signal is cheap to add.
- **Execution replaces the binary with no backup.** It does `mv -f` over the host's binary (`backend.rs:993-1009`),
  rewrites the unit, then runs `systemctl --user restart` (`plan.rs:469-473`, `backend.rs:2208-2221`).
- **The older binary refuses the newer database.** The supervisor store refuses a `user_version` above its own
  `SCHEMA_VERSION` (`crates/farhelm-supervisor/src/store.rs:2307-2312`; the schema is at 23 and is bumped often, see
  TODO.md's 0.3.0 note). The restarted unit (`Type=simple`, `Restart=on-failure`) crash-loops. `restart` still exits 0,
  so only attach fails, after 30 s (`service.rs:1505-1520`).
- **What survives.** `KillMode=process` keeps the tmux server, so agent processes keep running and the state directory
  is not modified. They are just unmanaged and unreachable until the newer binary is put back.
- **Realistic triggers.**
  - A host updated by a newer helm, such as an RC helm or a second helm.
  - A host updated by that host's own installer.
  - A helm rolled back to an older release, followed by "update all".
- **How to verify.** Fake-backend test: a probe reports a build newer than the helm, then assert that `plan_update`
  still returns a plan and execution runs `InstallPayload`/`RestartSupervisor`.
- **Fix shape.**
  - Refuse (409 with a clear message) or hide Update when the probed build is newer than the helm's, and skip such hosts
    in "update all".
  - Or require an explicit "downgrade" confirmation.
