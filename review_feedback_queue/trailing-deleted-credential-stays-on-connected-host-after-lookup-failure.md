# A credential the user deleted stays usable on a connected host until it reconnects

Reviewed commit: 107915bb765efe671d5012f461ed3c340cca43af

## TLDR

Suppose a host is connected, the user removes a Claude or Codex token from the recipe and deletes it from Settings →
Credentials, and then saves some other credential that host uses. Before this change, that delivery happened to remove
the deleted token from the host. Now the host keeps the deleted token in its launch credentials until it reconnects, so
new agent sessions and tabs on that host can still use it. The host shows an amber warning in the meantime.

## Details

**highest.** Recorded by the trailing review of `1b3fe49a..e159ffc7`; the tip `e159ffc79ab147be44e92a92b59571650a4f322d`
still has it. Bucket uncertain between `other`/`high` and `highest`; recorded `highest` under the trailing review's
err-higher rule. What would move it down: a decision that deleting a catalog credential does not promise prompt removal
from hosts already using it. The reviewer's proposed spec-accepted drop (SPEC.md 571-573, SPEC_impl.md 6201) went to the
independent drop check, which DISAGREED: SPEC.md's "cannot read" wording describes a read failure and does not clearly
accept keeping a credential the user deleted.

### Reviewer record (`g11-credentials/f1`)

Location: `crates/farhelm-helm/src/provisioning/credential_delivery.rs:985`. Summary: A deleted credential's value stays
on a connected host after a failed launch lookup instead of being removed

Revision of candidate f1, re-recorded as a finding because the consequence could be read as a security one (a credential
the user deleted stays usable on a host) and the brief records a spec-accepted candidate that could be highest as a
finding with proposed_drop.

Reviewed commit: 107915bb765efe671d5012f461ed3c340cca43af. The tip e159ffc79ab147be44e92a92b59571650a4f322d still has
it.

TLDR: Suppose a host is connected, the user removes a Claude or Codex token from the recipe and deletes it from Settings
→ Credentials, and then saves some other credential that host uses. Before this change, that delivery happened to remove
the deleted token from the host. Now the host keeps the deleted token in its launch credentials until it reconnects, so
new agent sessions and tabs on that host can still use it. The host shows an amber warning in the meantime.

Details: apply_agent_environment (credential_delivery.rs:985-997 at tip) returns lookup_aborted when either agent's
lookup fails, leaving the agent-credentials file and stamps untouched. A connection keeps the selections it read at
connect time (read_bindings, line 1228; SPEC.md 529-530 "Pending recipe edits do not change that connection's
selections"), so a push for another name used by that connection (push_one, line 714) looks up the deleted name, fails,
and keeps the old file. Before the change, the failed agent's entry was omitted and the file rewritten without it.
Without any intervening push, the deleted value stayed until reconnect both before and after the change, so the change
only removes this incidental cleanup. Reconnect reads the edited recipe and removes the entry (SPEC.md 569 "None removes
that agent's managed entries from the private state file"). Running processes keep their environment either way (SPEC.md
569). Fix, if wanted: when a launch lookup fails because the credential no longer exists (as opposed to a storage
error), omit only that agent's entry rather than freezing the whole file, and say which in the spec.

Suggested bucket: other. The established consequence is that a credential the user deliberately deleted stays usable on
their own host until the next reconnect, which the spec states. It would move to highest if deleting a catalog
credential were shown, by SPEC or product intent, to promise prompt removal from hosts already using it (e.g. as the
response to a leaked token). I found no such promise: SPEC.md 'Settings → Credentials' text only refuses deleting a
credential a recipe uses.

Confidence: confirmed (source).

proposed_drop: spec-accepted. SPEC.md lines 571-573: "If Farhelm cannot read either Claude's or Codex's recipe
credential, the host keeps both agents' last delivered launch credentials. The host warning names each affected agent,
and a replacement for those launch credentials reports that delivery failed." SPEC_impl.md "Named credential catalog"
line 6201: "When either agent's selected launch credential lookup fails, delivery leaves the combined file, both launch
stamps and both legacy settings files alone." Because it could be highest, this drop needs the independent drop check.

possible_cover: none known.

Review evidence (findings logs, delivery audits, dispositions and drop-check verdicts) is retained under run
`71907637-41f3-4337-888c-8515d58c5dbb` as `g11-credentials/f1`.
