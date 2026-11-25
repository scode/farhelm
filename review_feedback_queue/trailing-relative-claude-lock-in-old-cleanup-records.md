# Older Claude trust cleanup records keep a relative lock path

Reviewed commit: 9d36f490180a53ea5df77db70bb240aee70960af

## TLDR

If someone ran a fresh managed checkout with Claude trust on while their login shell set `CLAUDE_CONFIG_DIR` to a
relative path, and then upgraded, archiving that checkout later may fail to remove Claude's trust for the folder (it
keeps retrying at every supervisor start), or remove it without holding Claude's lock. The fix only stops new records
from having this shape.

## Details

Before this commit `Environment::from_vars` accepted a relative `CLAUDE_CONFIG_DIR`. `resolve()`
(`crates/farhelm-supervisor/src/agent_kind/claude/trust.rs:120-139`) canonicalizes the config path, but `lock_path` is
built from the uncanonicalized selected path plus `.lock` (`trust.rs:130-134`), so it stayed relative. Fresh-checkout
preparation persists that `TrustRecord` durably (`launch.rs:1986-1998`). Removal later runs in the supervisor process:
`edit_with_retry` (`trust.rs:264-281`) passes `record.lock_path` to `ConfigLock::acquire` unchanged; the only early exit
is a missing `config_path`, which does not apply. The supervisor's working directory is not the session folder, so the
relative lock either fails to create (removal fails, record kept, retried at every startup) or is created in the wrong
place and the key is removed without Claude's lock. The commit's own doc comment says config selection and lock must not
depend on either process's working directory; existing records are neither migrated nor refused.

Fix: when loading a `TrustRecord` for removal, refuse (log and drop) a relative `lock_path`, or derive the lock from an
absolute alias; optionally make `resolve()` store an absolute lock path.

Suggested bucket: `other`. Confidence: likely. The code route is established from source. Unverified after
investigation: whether any install ever produced such a record (it needs a relative `CLAUDE_CONFIG_DIR`, which Claude
itself would read relative to each session folder, plus Claude trust and a fresh checkout before this release). The
reviewer suggested dropping it as probably unpopulated; it is recorded because the route is real and no filter fully
matches. Possible cover: SPEC.md "A launch or Delete never fails because this edit failed" and SPEC_impl.md "A busy
vendor lock is logged and leaves the record for a later startup" cover the visible failure, not the stale trust.

Provenance: trailing code review through `953603f1`, run `fee35ad2-3831-4a91-b555-fe8cb1f8f1ae`, reviewer
`g03-helm-trust-requests` record `f1`.
