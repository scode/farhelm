# records config parse errors bypass the remote and account-id redaction

Reviewed commit: 7f703a44e069e2d53a626ffa0941c2ab37aa6f44

## TLDR

The records tool promises that its error messages never contain the records repository's URL or the Cloudflare account
id, because those messages go into agents' logs and reports, which may be published. A small mistake in the host's
records config (a misspelled key, a stray quote, a duplicated key, or an account id with a stray character) makes every
records command, the nightly backup included, print the config line itself, URL or account id included, into exactly
those logs.

## Details

Priority bucket: `other`.

Provenance: trailing code review of `953603f1a036eb5846ebf251d1564c5efd9c8e67..f0c042c9efea4a46bc02e0ad880c1a30aae3c85f`
(run `70f0f28c-538d-461e-b80e-783ddaafaadb`), reviewer record `t15c-records-tool` `f1` (Claude Code `claude -p`,
`claude-opus-5-5`, effort high). Findings were validated by source and contract analysis; nothing was built, run or
reproduced.

### Recording notes

Reviewer's suggested bucket kept: it leaks an account identifier, not a credential, into maintainer-tooling logs and
only after an operator config mistake. No coverage source covers it.

### Reviewer record

Location: `xtask/src/records/config.rs:113`

Revision (turn 2): the backup route, previously marked unchecked, is now checked and confirmed to share the defect; the
suggested fix's toml API is confirmed to exist. Everything else unchanged.

xtask/src/records/mod.rs:58-60 (reviewed commit) states the contract: 'Failure messages never quote the configured
remote ... or the Cloudflare account id'; xtask/src/records/error.rs `redact_remote` documents why (messages end up in
logs and reports that may be published). Route 1: config.rs:113-114
`toml::from_str(text).map_err(|e| Failure::failed(e.to_string()...))`. With toml 1.2.3 (xtask/Cargo.lock),
`toml::de::Error`'s Display (toml-1.2.3/src/de/error.rs:116-140) writes 'TOML parse error at line N, column M', then the
full source line containing the error span (`input.split('\n').nth(line)`), then a caret. With `deny_unknown_fields` on
RawConfig/RawArtifacts, a typo such as `remote_url = "https://<account>.artifacts.cloudflare.net/..."` or
`acount_id = "<account>"`, a duplicate key, or an unterminated string on the remote line, produces a message quoting
that line. Route 2: config.rs:166-176 `artifacts()` echoes the rejected value
(`[artifacts] account_id `{value}` must be ...`), so a real id with a stray character (e.g. a trailing space or a pasted
`/`) is printed. Neither route is redacted on any command: mod.rs `path_command`/`push_command` call
`config::load(...)?` before `.map_err(|f| f.redact_remote(&config.remote))`, and xtask/src/records/backup.rs:64-77
(`run`) likewise returns `config::load(...)` errors directly, applying `redact_remote` and the `<account id>`
replacement only to `run_with` failures. Redaction could not help anyway: no remote or account id has been parsed yet,
and an account_id line does not contain the full remote string. The message goes to stderr (mod.rs `main`), which the
unattended flows capture into their logs, and backup is meant to run on a schedule whose output the maintainer collects.
Verify: read the toml Display impl cited above, config.rs:113 and backup.rs:64-77; source-established, not executed.
Fix: build the message from `e.message()` plus the line/column derived from `e.span()` (both public on toml 1.2.3
`de::Error`, error.rs:64 and :69) instead of `e.to_string()`, and do not echo rejected `[artifacts]` values (name the
key only); add a unit test that a malformed config line containing a fake account id does not appear in the failure
message.

Suggested bucket: other; it leaks an account identifier (not a credential) only after an operator config mistake, into
maintainer-tooling logs; it would move up only if the account id were treated as a secret enabling access, which the
token-based auth does not suggest.

Confidence: confirmed (source-established: toml 1.2.3 Display prints the source line; config.rs maps the error with
to_string(); neither mod.rs nor backup.rs redacts config-load failures). Not executed.

proposed_drop: none

possible_cover: none
