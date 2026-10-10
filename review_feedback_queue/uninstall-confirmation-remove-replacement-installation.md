# An uninstall confirmation can remove a replacement installation

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

An old uninstall approval could remove an adopted replacement installation.

## Details

F64 — **possible** — `crates/farhelm-helm/src/provisioning/service.rs:1821` — An uninstall confirmation can remove a
replacement installation

The uninstall plan retains no approved installation identity. Adoption can change that identity outside the provisioning
lock, after which otherwise matching plan facts can pass revalidation for the replacement. The ordinary single-window
trigger remains unverified because known provisioning disables adoption in the UI. Capture the approved identity,
compare it after execution-slot waits, and serialize adoption with uninstall execution. The established conditional
removal concerns installation files, supervisor shutdown, and registration, not demonstrated session-data or
running-work loss.

## Evidence and triage context

- crates/farhelm-helm/src/provisioning/http.rs:212-225: the uninstall endpoint creates a plan or consumes its opaque
  identifier; confirmation supplies no installation identity.
- crates/farhelm-helm/src/provisioning/service.rs:1177-1184: planning retains the plan, host ID and registration. Lines
  87-109 and 2400-2410 show that the uninstall confirmation and registration omit host_identity.
- crates/farhelm-helm/src/provisioning/service.rs:1540-1555: the accepted run holds the provisioning lock but waits for
  a global execution slot before revalidation.
- crates/farhelm-helm/src/hosts.rs:977-991 calls manager.adopt directly. crates/farhelm-helm/src/manager.rs:2528-2632
  changes the identity and reconnects without acquiring the provisioning lock or invalidating uninstall plans.
- crates/farhelm-helm/src/store.rs:5281-5312: adoption checks the old identity and coordinates, then updates
  host_identity and confirmation preferences while preserving the destination and binary/state-directory coordinates.
- crates/farhelm-helm/src/provisioning/service.rs:1821-1838: revalidation compares registration, fetches the current row
  and replans against that row. Lines 1247-1265 compare the fresh probe's identity with this current row, not the
  originally approved identity.
- crates/farhelm-helm/src/provisioning/plan.rs:288-304 and 685-717: plan equality covers operation, target, paths,
  actions, distribution and architecture, but no installation identity. A replacement with matching plan-visible facts
  produces an equal plan.
- crates/farhelm-helm/src/provisioning/service.rs:1636-1641 and 2193-2212: successful revalidation executes service
  disable/removal, supervisor stop, directory removal and registry deletion.
  crates/farhelm-helm/src/provisioning/backend.rs:2549 and 2580 perform systemctl stop and rm -rf without an
  installation-identity check.
- crates/farhelm-ui/src/hosts.rs:1648-1651 and 3121-3148: the UI includes provisioning activity in its busy state and
  refuses the Adopt click while busy. crates/farhelm-ui/src/provisioning.rs:1550-1557 maintains that busy state for a
  running run. This limits the report's claimed UI sequence.

Retained confidence: **possible**. Suggested bucket: **highest**.

Possible cover:

- {"basis": "TRIAGE_OUTCOMES.md:6991-7006, uninstall-missing-id.md", "comparison": "Covers a fresh probe disagreeing
  with the current registry, including missing identities. Here adoption makes the current registry agree with the
  replacement; the missing comparison is with the earlier uninstall approval. Trigger and scope differ."}
- {"basis": "SPEC_impl.md:3361-3367", "comparison": "Requires replanning and fresh-probe equality with the recorded
  identity. It does not explicitly accept carrying an earlier uninstall approval across adoption."}
- {"basis": "TRIAGE_OUTCOMES.md:7008-7035 and review_feedback_queue/uninstall-reload.md:14-24", "comparison": "Concern
  stopping the private tmux server during removal of the approved installation, including sessions started during that
  removal. They do not authorize removing a replacement installation adopted after approval."}
- {"basis": "SPEC.md:2233-2246, One GUI at a time", "comparison": "Concurrent GUIs have best-effort consistency, but the
  finding establishes a backend approval-binding omission rather than an exclusively multi-GUI trigger. Ordinary
  single-GUI reachability remains unproven; this is not an exact coverage basis."}
- {"basis": "review_feedback_queue/provisioning-update-replaces-binary-before-setup-guard.md:7-11", "comparison":
  "Covers UPDATE overwriting a binary after setup takes ownership. It does not cover UNINSTALL crossing an adopted
  installation identity."}
- {"basis": "review_feedback_queue/uninstall-forgotten-success.md:14-23", "comparison": "Covers a false success notice
  after another client forgets a failed uninstall's host. It causes no additional removal and does not cover
  replacement-installation deletion."}

Caveats:

- Requires an identity replacement and successful adoption before uninstall revalidation, unchanged plan-visible facts,
  reconnection, and a replacement passing all ordinary uninstall checks.
- The backend interleaving is source-grounded; a complete ordinary single-GUI sequence was not established because
  adoption is disabled during known provisioning activity.
- No runtime reproduction was performed.
- The demonstrated consequence is replacement supervisor shutdown, installation-file removal and registry deletion.
  Session-data deletion or loss of running session processes is not established.
- TODO.md:33-43 contains only the unrelated planned connection-reader work. BUGS.md's accepted failures do not match
  this trigger or consequence.
- FILTER.md:36-42 and 130-135 exclude wrong-target destructive actions and trust-boundary consequences from the relevant
  rarity filters.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `helm_state_provisioning_08_cor:p2:F1`.

- `helm_state_provisioning_08_cor:p2:F1`: confidence as filed: Definite; confirmed by inspection; suggested bucket as
  filed: highest.
