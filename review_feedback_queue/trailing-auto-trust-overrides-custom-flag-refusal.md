# Automatic disposable trust can override a user's Custom-flag trust refusal

Reviewed commit: cc9bec6cf70ab293d6f8b2c2115bc170f69c8d01

## TLDR

A user who starts Pi with Custom permission flags that include `--no-approve`, or Codex with a Custom `-c` override
marking the folder untrusted, has said they do not want the repository's project settings, extensions or hooks approved.
In a session's own sandbox or container, Farhelm now adds its own trust flag after the user's words anyway, and nothing
in the launch shows the conflict. If the agent honors the last flag, the repository's code is approved against the
user's explicit choice, in an environment that holds delivered credentials. SPEC.md says only "Custom settings flags
still take precedence", which a user can read as covering this.

## Details

`workspace_trust.rs:142-152` grants trust for any supporting harness on a disposable destination unless
`launches::workspace_trust_blocked_by_selection` says otherwise. That function (`launches.rs:837-856` at the tip) blocks
only Antigravity's Custom `--settings`/`--settings=`; every other harness returns false. The compiler appends the trust
segment after the Custom permission words (`compile`, `launches.rs:679-698`: Custom words first, then
`workspace_trust_arguments` spliced at `argv.len() - trust_custom_tail_len`; trust words from `launches.rs:898-930`: Pi
`--approve`, Codex `-c <trusted-cwd marker>`, Muse `--trust-workspace`). The test
`stored_trust_replacement_keeps_custom_words_and_command_launches` (`launches.rs:3262-3277`) shows custom trust-looking
words survive beside the compiler's segment. Before this commit the pairing needed the user to turn the switch on
themselves; now it is automatic on every session-owned host. SPEC_impl.md scopes the precedence to Antigravity
`--settings`, but the new SPEC.md sentence does not.

Fix: extend `workspace_trust_blocked_by_selection` to treat Pi `--no-approve`/`--approve` and Codex trust-level
overrides in Custom flags as the user's own choice, as Antigravity's `--settings` already is; or make the SPEC.md
sentence say it covers only Antigravity's `--settings`.

Suggested bucket: `high`. Per "Priority buckets" (2026-10-22), it is bucketed by what is established: the flag order and
the missing refusal check are confirmed from source. It would be `highest` once the vendor CLIs are shown to apply the
later flag (that route overrides an explicit opt-out of repository code approval beside credentials), and `other` if the
maintainer reads "Custom settings flags" as Antigravity only. Confidence: likely; unverified after investigation that Pi
resolves `--no-approve ... --approve` in favor of the later flag and that Codex lets the later `-c` win (this review ran
no vendor CLIs). Possible cover: SPEC_impl.md "Custom `--settings` and `--settings=...` options take precedence over
Farhelm's opt-in" (Antigravity only). The automatic-trust policy itself was also raised in this review and dropped as
spec-accepted (SPEC.md "Managed checkouts": "Structured launches in a session's own sandbox or container automatically
trust ... Custom settings flags still take precedence"); this item is about the user's own refusal, which that sentence
appears to promise to honor.

Provenance: trailing code review through `953603f1`, run `fee35ad2-3831-4a91-b555-fe8cb1f8f1ae`, reviewer
`g04-disposable-trust-impl` record `f10`.
