# A later Codex or GitHub delivery failure hides a pending legacy Claude cleanup warning

Reviewed commit: 67d8541dce597e3a70dc37857167ed39b6db2d68

## TLDR

On an upgraded host whose old Claude settings file cannot be cleaned up yet, if Codex or GitHub setup also fails on that
same host (for example, because gh is missing for private GitHub clones), Farhelm hides the Claude cleanup warning and
shows only the Codex or GitHub error. Replacing the host's Claude credential then reports that unrelated Codex or GitHub
error as the reason the Claude credential was not delivered.

## Details

**other.** Source inspection. Main reference `crates/farhelm-helm/src/provisioning/credential_delivery.rs:631`.

In crates/farhelm-helm/src/provisioning/credential_delivery.rs:631-634, CredentialDelivery::apply captures
`pending_cleanup: Option<String>` from `apply_claude` when clearing the legacy keys from `~/.claude/settings.json`
fails, intentionally continuing to Codex config (lines 635-651), Codex auth (lines 652-703), and GitHub (lines 707-764)
so those independent slots can still finish. However, every later slot returns via `?` (`Err(String)`), discarding
`pending_cleanup` whenever any later slot fails (for example, line 749 returning
`Err("GitHub CLI (gh) is missing; private clones need gh installed")` on a reused image without `gh`, or a malformed
`~/.codex/config.toml`). As a result:

1. On connect (`connect`, line 389) and setup (`setup`, lines 411-416), `entry.warning` records only the later
   Codex/GitHub error and drops `pending_cleanup`, leaving no indication that Claude launch credentials were withheld.
2. On replacing a Claude credential (`push_one`, lines 558-568), `result` is `Err(later_error)`, so
   `SavedCredential.delivery_failures` reports the unrelated Codex/GitHub failure instead of `pending_cleanup` as the
   reason the Claude credential was not delivered, violating SPEC_impl.md ('Named credential catalog', lines 6021-6023:
   'A failed settings merge keeps the legacy stamp for retry, withholds the new environment, and leaves Codex and GitHub
   delivery eligible to finish. It appears in the host's warning and in the result of a replacement for a credential the
   host's Claude selection uses.'). Fix direction: Carry `pending_cleanup` alongside any later-slot error out of `apply`
   (e.g., returning both outcomes or combining them into the host warning while surfacing `pending_cleanup` for
   `uses_claude` replacements and the later error for non-Claude replacements). Verified by source analysis only.

Reviewer metadata (g21-recipe-creds-delivery-core/l2): suggested bucket: other (cannot be highest: Claude launch
credentials remain safely withheld while legacy settings cleanup is pending, so no credential misrouting, data loss, or
security exposure occurs); confidence: confirmed; possible cover: none.

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` as `g21-recipe-creds-delivery-core/l2`.

## Extension from the trailing review through e159ffc7

Reviewed commit: 107915bb765efe671d5012f461ed3c340cca43af

Still present at the tip `e159ffc79ab147be44e92a92b59571650a4f322d` after #4751 reworked delivery, and wider than the
original record: the same loss applies to a pending legacy **Codex** settings cleanup, not only Claude's. `apply`
(`crates/farhelm-helm/src/provisioning/credential_delivery.rs:800-826`) now gets `DeliveryWarnings` from
`apply_agent_environment`; when the launch lookups succeed but `retire_legacy_settings` (lines 1015-1041) returns a
cleanup warning, `warnings.claude`/`warnings.codex` hold it with `lookup_aborted` false. If `apply_codex_login` or
`apply_github_token` then fails, line 824 builds
`DeliveryError { lookup_warnings: warnings.lookup_aborted.then_some(warnings) }`, i.e. `None`, so `connect` (line 535)
and `push_one` (lines 719-724) report only the later reason. SPEC_impl.md "Named credential catalog" (lines 6220-6222)
still requires the host warning to name the affected agent. Fix: carry all non-empty warnings into `DeliveryError` (drop
the `lookup_aborted` filter) and have `push_failure`/`message_for` select cleanup reasons as they already do on the `Ok`
path; add a test with a failed cleanup plus a deleted GitHub selection. Reviewer suggested bucket: other; confidence:
confirmed. Evidence retained under run `71907637-41f3-4337-888c-8515d58c5dbb` as `g11-credentials/f2`.
