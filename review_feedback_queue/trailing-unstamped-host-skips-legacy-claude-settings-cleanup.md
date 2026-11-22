# Upgrading an unstamped host skips legacy Claude settings cleanup after an ambiguous prior write

Reviewed commit: 67d8541dce597e3a70dc37857167ed39b6db2d68

## TLDR

If an earlier Farhelm version wrote Claude endpoint settings onto a host but lost the connection or crashed before
recording that delivery locally, upgrading skips cleaning up that settings file. Because Claude can prefer a base URL in
its settings file over the new launch environment, a newly delivered Claude token could be sent to the old endpoint URL.

## Details

**highest.** Source inspection. Main reference `crates/farhelm-helm/src/provisioning/credential_delivery.rs:798`.

In crates/farhelm-helm/src/provisioning/credential_delivery.rs:795-822, `apply_claude` only invokes `clear_claude` on
`~/.claude/settings.json` when `record.claude.is_some()` (line 798). When `record.claude` is `None`, it records
`record.claude = Some(EndpointStamp::Cleared)` directly without reading or cleaning `~/.claude/settings.json`, then
proceeds to write `agent-credentials` (lines 824-876). If a pre-#4593 Farhelm wrote `ANTHROPIC_BASE_URL` into
`~/.claude/settings.json` on the host and suffered a lost SSH reply or helm crash before `write_record` committed
`record.claude`, `record.claude` remains `None` and the stale URL in `~/.claude/settings.json` is never removed before
new credentials are published in `agent-credentials`. Reported as a finding with `Proposed drop` per the independent
drop check requirement for any spec-accepted candidate that could be in the `highest` bucket. Verified by source
analysis only.

Reviewer metadata (g21-recipe-creds-delivery-core/f1): suggested bucket: highest (potential credential
exfiltration/misrouting to a stale endpoint URL; proposed drop below for independent drop check); confidence: confirmed;
possible cover: SPEC_impl.md ('Named credential catalog', lines 6007-6008 and 6018-6020).

Additional source record at `67d8541dce597e3a70dc37857167ed39b6db2d68`, `SPEC_impl.md:6018`
(g22-recipe-creds-specs-tests/f3):

In `crates/farhelm-helm/src/provisioning/credential_delivery.rs:795-822` (`apply_claude`), legacy cleanup of
`~/.claude/settings.json` runs only when `record.claude != Some(EndpointStamp::Cleared)` AND `record.claude.is_some()`.
When `record.claude` is `None`, the helm immediately records `record.claude = Some(EndpointStamp::Cleared)` and proceeds
to upload `agent-credentials`. Both `SPEC.md:530-531` and `SPEC_impl.md:6007-6008,6018-6020` explicitly accept this
tradeoff to preserve image-authored Claude settings on fresh hosts and note that remote success before a local stamp is
ambiguous. Recorded as a finding with `Proposed drop` per the rule that spec-accepted candidates that could be in the
`highest` bucket must go through the independent drop check.

Reviewer metadata (g22-recipe-creds-specs-tests/f3): suggested bucket: highest (could be in highest: if a pre-upgrade
Farhelm wrote a legacy `ANTHROPIC_BASE_URL` into `~/.claude/settings.json` and lost the connection or crashed before
recording `record.claude`, upgrading and delivering a new Claude token into `agent-credentials` without cleaning
`~/.claude/settings.json` could let Claude prefer the stale URL in settings and send the new token to that old URL);
confidence: confirmed; possible cover: SPEC.md ("Topology", lines 530-531) and SPEC_impl.md ("Named credential catalog",
lines 6007-6008 and 6018-6020).

Independent drop check (Entry D3, cid `374ef90a-343a-4c2b-a274-38c86f0ad7a0`): DISAGREE via
`credential_delivery.rs:92-136,795-876`, `SPEC.md:530-531`, and `SPEC_impl.md:5992-6024`. The ambiguity sentence at
`SPEC_impl.md:6007-6008` is scoped to Codex `auth.json` seeding (where ambiguity causes a retry, not a permanent cleanup
skip), and `SPEC_impl.md:6018-6020` justifies the unstamped branch strictly as preserving image settings rather than
accepting an ambiguous pre-upgrade Farhelm write.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g21-recipe-creds-delivery-core/f1`,
`g22-recipe-creds-specs-tests/f3`.
