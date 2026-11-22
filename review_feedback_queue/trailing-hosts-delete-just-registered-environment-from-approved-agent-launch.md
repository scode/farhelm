# Hosts can delete a just-registered environment from an approved agent launch

Reviewed commit: 40d1a549ffe9a1d72100154f59a1c1f9e74bd9ae

## TLDR

A user-approved agent launch can expose its new environment to the Hosts delete control before provisioning takes the
host lock. A deletion that wins can leave a stale launch task and a misleading cleanup failure.

## Details

**other.** Source inspection. Main reference `crates/farhelm-helm/src/provisioning/tensorlake_add.rs:459`.

SPEC.md at tip:1457–1463 sets the ordinary launcher rule and 4750–4770 specifies direct Allow for a new environment;
SPEC_impl.md:6316–6327 routes that to provisioning. GUI approvals/answers.rs:108–123 uses Work::Answer, excluded by
operations.rs:237–258 from hosts.rs:1050–1066's launch lockout. After Allow, agent_requests.rs:4519–4554 starts the same
owned launch. tensorlake_add.rs:393–412 and manager.rs:2110–2136 publish its actor before the busy/run write and
provisioning lock (448–469). GUI hosts/offers.rs:510–524,559–567 offers provider delete while the new row is listed and
progress is absent, and remove.rs:136–151,347–376 can send it without the local launcher flag. The helm's removal entry
point can take that lock first (hosts.rs:1209–1211,1314–1326,1344–1389). Pending Podman with neither ID nor create
marker admits deletion (environment/podman.rs:1414–1421). Purge can remove the busy and run entries before the launch
reacquires the host lock and unconditionally inserts its task handle (service.rs:2533–2549,2563–2568.
Tensorlake_add.rs:448–469,481–535). Provider create checks the row, and failure cleanup then reports it could not remove
an already deleted environment (environment/podman.rs:1159–1175, tensorlake/hosts.rs:364–384,
tensorlake_add.rs:1145–1186. Hosts.rs:1350). Ordinary provisioning and rerun already check the row under the guard.
Bucket: other, limited to false acceptance, stale bookkeeping and misleading failure. Confidence: shipped controls and
race established by source. No measured timing. Possible coverage: finished-task-handle-published-after-host-purge (fix
covers a child deleting its own row).

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `provision/p1`.
