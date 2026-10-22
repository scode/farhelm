# Taking a spare adopts a replaced ready folder

Reviewed commit: 1d2ff45e4e5a2de2a6ff9bcc1aac9a9eba647199

## TLDR

If someone removes and recreates a prepared-checkout folder, a later launch can move the replacement files into
Farhelm's private cleanup area. A later supervisor startup can delete them. This conditional sequence is established
from source; it has not been reproduced at runtime.

## Details

Suggested bucket: **highest**, for deletion of replacement user files. Confidence: **confirmed conditional source
sequence**. Ordinary replacement is the premise; no replacement inside a check-and-move window is required. Occurrence
frequency is unmeasured.

`crates/farhelm-supervisor/src/spare_take.rs:325–348` renames the ready basename into a generated claim without checking
the ready identity in the recovery record. Structural inspection follows the rename (`80–85`); even a non-Git
replacement can therefore become a refused claim. At the same revision, `spare_checkouts.rs:247–276` requires the stored
identity for ready names but adopts the current identity of dead private claims; `318–330` removes them recursively.
Confining that removal to the selected directory does not establish ownership of its contents.

At pinned tip `2ea2826caa0eec867c4e2bddc01badbac7bbe6c7`, runtime discard protects all claims and preserves the recovery
record (`spare_checkouts.rs:148–164,540–554`). Startup protects live clone stages, then sweeps the others
(`service/core.rs:6953–6964,7079–7086`; `repo_cache.rs:80–100`). Finishing the visible clone releases its cache lock
before that later startup. Neither runtime retention nor refill repairs the original ownership transfer. Tests of
ready-name cleanup and destination replacement exercise different boundaries.

The introducing revision stages normal creation off. The unchanged take and recovery modules become active at the pinned
tip: `crates/farhelm-helm/src/store.rs:487–503,3904–3919` defaults the setting on, `sessions.rs:1170` passes it, and
supervisor `service/core.rs:10017–10035,10591–10595` retains it and registers creates.

Possible cover: pinned `SPEC.md:3505–3514` excludes out-of-band interference but explicitly retains ordinary
removed/recreated-folder guards. `1064–1067` denies replaced directories deletion authority. `SPEC_impl.md:2413–2414`
says namespace ownership never adopts a replaced ready spare, while `2438` says take never reads the recovery record.
That is a contract conflict to settle, not established acceptance of this consequence. No exact Planned, BUGS, queue,
ledger or FILTER coverage was found. Proposed drop: none. Validate ready ownership before transferring it into a name
whose cleanup authority comes from the namespace.

Provenance: trailing-review run `30f46c42-a021-475f-96a8-eac06930471b`, `g012/f1`; corroborated by `g009/f5`, `g010/f2`
and `g015/f3`, and independently retained by `g046/f1`. These identify one defect.
