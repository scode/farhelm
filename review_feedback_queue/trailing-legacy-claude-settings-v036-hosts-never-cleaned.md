# Claude settings written by Farhelm 0.36.0 on Tensorlake hosts can stay on the sandbox for good

Reviewed commit: ba8dbf0e98a9f2cdff1f7ca533d5ee6baa529d38

## TLDR

Farhelm 0.36.0 set up Tensorlake hosts by writing the recipe's Claude endpoint URL and token straight into Claude Code's
own settings file on the sandbox, and recorded nothing about it. Since 0.44, Farhelm cleans that file only on hosts
where it recorded writing it. A 0.36.0-era host that was not connected while 0.37 to 0.43 was installed (it stayed
suspended, say, or the user upgraded straight past those versions) therefore keeps the old URL and token in Claude's
settings file permanently. That is the token the 0.37 upgrade removed from Farhelm and told the user to re-enter.
Changing or removing the endpoint or login in Farhelm does not reach that file, and per the implementation spec's own
text those leftover settings can take precedence over what Farhelm now supplies at launch, so a new token or login can
end up going to the old endpoint. Whether Claude Code actually lets the file win over the launch environment is not
verified.

## Details

The spec text comes from 9e96b1771d. The cleanup behavior comes from 67d8541dc (first released in v0.44.0) and eddd763f5
(v0.45.0), and the legacy writer is in v0.36.0.

Locations: SPEC_impl.md:6221-6229; `crates/farhelm-helm/src/provisioning/credential_delivery.rs:1120-1166`
(`retire_legacy_settings`: `needs_cleanup = stamp.is_some()` for Claude, then records Cleared) and `:1367-1381`
(`read_record`: a NULL column becomes `Record::default()`, with `claude` None);
`crates/farhelm-helm/src/store.rs:4946-4955` (the schema 76->77 migration only adds the empty
`tensorlake_credential_delivery` column, with no stamp backfill).

v0.36.0's `crates/farhelm-helm/src/provisioning/tensorlake_add.rs` (`configure_agents`, around lines 596-640 at tag
v0.36.0) merges `ANTHROPIC_BASE_URL` and `ANTHROPIC_AUTH_TOKEN` (the token, or an empty string for a tokenless endpoint;
`claude()` in v0.36.0's `agent_endpoints.rs`) into `~/.claude/settings.json` at host setup. v0.36.0 has no
`credential_delivery.rs` and no `tensorlake_credential_delivery` column. v0.37.0 through v0.43.x
(`credential_delivery.rs` at v0.37.0, around lines 420-500) re-delivered settings on first connection whenever
`record.claude` differed from the slot's stamp, including a NULL record, so a v0.36.0 host connected under those
versions got stamped and is cleaned later. 67d8541dc (v0.44.0) changed this to "A missing stamp can describe a fresh
image with its own Claude settings. Only earlier Farhelm writes grant cleanup", and the current `retire_legacy_settings`
keeps that.

So the route is established in shipped code without any crash: set up under v0.36.0 with a Claude endpoint, no
connection under v0.37.0 through v0.43.x, then connect under v0.44 or later. A secondary route: v0.37.0 through v0.43.x
installed settings before committing the stamp (v0.37.0 `credential_delivery.rs`, around lines 476-500), so a crash or
failed record write there also leaves an unstamped write. That is the case the new spec sentence names.

Consequences established from source: the Farhelm-written URL and the old token remain on the sandbox, and later
credential removal or rotation in Farhelm does not reach them. The consequence resting on vendor behavior (unverified):
SPEC_impl.md:6222-6223 already says "An old Claude settings URL can otherwise take precedence and receive the new
token". With a tokenless old endpoint (`ANTHROPIC_AUTH_TOKEN` "") and a new Subscription selection
(`CLAUDE_CODE_OAUTH_TOKEN` in the launch environment, `agent_endpoints.rs:18-19`), whether Claude Code then sends the
login to the old URL depends on its precedence rules.

Verify by reading the three code points above and
`git show v0.36.0:crates/farhelm-helm/src/provisioning/tensorlake_add.rs`. Fix: on a host with a NULL delivery record,
treat a `settings.json` whose env holds the exact legacy key pair (`ANTHROPIC_BASE_URL` with `ANTHROPIC_AUTH_TOKEN`) as
a Farhelm write and clear it, or at least warn in host details. Alternatively, backfill a legacy stamp for hosts whose
rows predate schema 77, since those hosts can only have been set up by a version that wrote settings when their recipe
had a Claude endpoint.

Bucket: highest. The established consequence is that Farhelm's v0.36.0-written Claude endpoint URL and token stay in
Claude's settings on the sandbox for good, and later removal or rotation in Farhelm does not reach them. SPEC_impl.md
itself says such an old settings URL can take precedence and receive the new token, a trust-boundary consequence, so it
is recorded as highest. What would move it down is evidence that Claude Code gives the launch environment precedence
over `settings.json` for these keys. (The reviewer suggested high, moving to highest if Claude Code is shown to send a
newly supplied token or login to the leftover URL; Tensorlake is marked EXPERIMENTAL in the v0.37.0 changelog.)

Confidence: confirmed for the leftover-file route (source at v0.36.0, v0.37.0, 67d8541dc and the tip); likely for
credential exposure, whose unverified premise is Claude Code's precedence between settings env and process env for these
keys.

Possible cover: SPEC_impl.md, "Named credential catalog", accepts "An old Claude settings write whose stamp was never
recorded, for instance after a crash between the remote write and the local commit" (SPEC_impl.md:6224-6226, added by
9e96b1771d). The archived decision behind it (TRIAGE_OUTCOMES_ARCHIVE.md, heading
"trailing-unstamped-host-skips-legacy-claude-settings-cleanup.md") assessed only a crash-then-upgrade route and assumed
a reconnect under an older release would have stamped the host; this route needs no crash, so trigger and scope differ.
An independent drop check agreed the spec text does not cover it. The reviewer had proposed a spec-accepted drop with
that caveat (the words "whose stamp was never recorded" fit, but the example suggests a crash-only premise); the
commissioner rejected the drop.

Provenance: Trailing code review of f6d8d8bb..ba8dbf0e (2026-11-25), reviewer group g24 record f1; claude-opus-5-5
reviewer, source analysis only, nothing executed.
