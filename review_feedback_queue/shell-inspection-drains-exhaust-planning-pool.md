# Shell inspection can retain every host-planning slot

Reviewed commit: 3bb35222ce3fc4bcc2a6c2aed16475351e9d208f

## TLDR

Host discovery or planning can remain stuck if a shell-inspection child exits while a descendant keeps its output pipes
open. Four such operations can block planning for other hosts. An actual helper configuration producing this was not
observed.

## Details

`crates/farhelm-helm/src/provisioning/backend.rs:1570–1628,1790–1827` bounds waiting for the child but then awaits
stdout and stderr readers without a deadline. Killing or reaping the direct child does not close pipe writers inherited
by a descendant. Helm-owned planning tasks retain a shared permit across this operation
(`provisioning/service.rs:980–1040`); the pool has four slots. This is a planning-pool consequence across hosts, not
only one execution request waiting to finish. Local SSH helpers or configuration need to establish the inherited-pipe
trigger; the inspected ordinary ControlPersist path does not establish it.

The reviewed change bounds `stop_probe` stderr cleanup. That fixes its named narrow path but leaves shell inspection
adjacent and pre-existing. Its broad changelog description should not be read as proof that every discovery drain is
bounded. Establish a supported helper trigger and either narrow that claim or address the separate lifetime boundary.

Suggested bucket: **high**, for potentially blocking all planning. Confidence: **confirmed unbounded waits and permit
lifetime; real configuration and pool exhaustion unverified**. No unsafe installation, host deletion or user-work loss
was established. Proposed drop: none.

Possible cover: `BUGS.md:80–101` and `TRIAGE_OUTCOMES.md:5057–5082` accept an execution run slot held by output
draining. They do not clearly accept exhausting the shared planning pool. The narrow probe decision at `16607–16625`
concerns the fixed call, not this drain. No exact Planned, queue or filter match was found. A host-local dependency
exception does not clearly cover blocking other hosts' planning. Keep the scope interpretation and pre-existing
attribution for triage.

Provenance: `host-discovery-whole-1/f1` in the trailing review through `fe0348ef8d4fabc3c6140a4fd89fb19dd34178b1`. A
separate independent audit refutes unsafe installation from discarded probe classification; that exclusion does not
clear this planning-lifetime concern. No runtime reproduction was performed.

Original finding IDs: `host-discovery-whole-1/f1`.
