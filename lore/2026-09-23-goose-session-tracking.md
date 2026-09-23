# Goose session tracking: preserved strict design and a smaller path

This note records why the stricter Goose foreground-ownership implementation was set aside, while preserving its
history and the research that challenged its complexity. The strict implementation is available at the immutable
`goose-capture-complex-2026-09-23` tag. It was not merged into `main`.

## The problem

Farhelm's basic Goose reporter learns the session ID from an MCP extension. Goose native subagents can inherit that
extension and run in the same operating-system process with a different session ID. A separate Goose or another
harness can also be launched with the same reporter credential. The credential identifies the Farhelm session, but it
does not prove that the reported Goose session is the foreground root. A child report can therefore replace the saved
restart target unless admission has an independent ownership check.

The basic support remains intentionally available with that limitation. It is useful for ordinary foreground sessions,
but it does not promise child-session isolation.

## What the preserved strict implementation did

The tagged design kept the MCP reporter and process attribution, then opened Goose's live store to validate the exact
reported row. It accepted only a `user` session with no parent link, while checking the runtime store path and a known
schema shape. Its custom read-only SQLite VFS attempted to prevent database, WAL, SHM, directory, migration, and
checkpoint side effects, with operation, result, and read-size budgets. Resume revalidated the saved row before offering
it again.

That design addressed two separate concerns: identifying a root conversation at report admission, and refusing to resume
when saved metadata could no longer be proved. The second concern is a freshness policy; it is not required merely to
stop a child report from overwriting a parent.

## Research result

Source research against pinned and current Goose found no reliable role signal in today's MCP interface. Native child
agents receive their own session ID and can inherit the reporter. Goose's lifecycle hooks suppress native subagent hooks,
but `/new` ends the old session without immediately emitting a new `SessionStart`; a hooks-only reporter would miss an
idle `/new` transition. `GOOSE_STATUS_HOOK` has no session-ID payload, and the existing MCP metadata does not identify a
root versus subagent role. A database-independent replacement is therefore not available today.

The likely smaller solution is to keep the exact parent-role lookup and process proof, but replace the custom VFS with a
small ordinary read-only SQLite reader. It should query only the reported session's role and parent link, with bounded
time, result, and CPU work. This preserves the ownership decision while accepting SQLite's normal WAL/SHM coordination
side effects. `mode=ro` does not promise zero side effects for a live WAL database: ordinary SQLite may create or update
coordination files. The reader must still avoid application writes, migrations, discovery scans, and immutable snapshots.

An owned helper subprocess is a possible additional isolation boundary if supervisor deadlines require it, but it adds
lifecycle and concurrency work without restoring the no-side-effect guarantee. The cleanest eventual design would be an
upstream Goose role field or an explicit extension setting that prevents native-subagent inheritance; neither exists in a
supported release today.

The research also found that pre-resume demotion after one transient store-read failure is a separate availability tradeoff.
Preserving a verified binding and reporting a retryable error may be simpler, but that would relax the current stale-target
policy and needs a product decision. No runtime validation was performed for the proposed smaller reader.

## Decision

Keep basic Goose support on `main`, document its child-session limitation, and defer stronger ownership checks to the
per-harness TODO. Preserve the strict implementation under the tag above so its tested history, tradeoffs, and migration
work remain available if the product later requires child isolation.
