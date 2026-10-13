# Newly created checkout roots lack parent durability barriers

Reviewed commit: d2f4508e01366c15ade8a84dff988a6400ef16ba

## TLDR

After the first checkout creates its configured folder hierarchy, a machine crash could leave Farhelm remembering an
allocation whose enclosing folders did not survive. A checkout containing work could become inaccessible or be lost.

## Details

Suggested bucket: `highest`, provisionally because the claimed consequence includes user work. Confidence: missing
barriers confirmed by inspection; the consequence is likely and depends on filesystem crash behavior and work reaching
the checkout before its ancestor entries become durable. No power-loss reproduction was performed. Source: adversarial
checkout reviewer, independently checked by a fresh reviewer. Proposed drop: the independent reviewer found no exact
acceptance justifying omission.

`781b4a6c7da9ede894d79845099039ba4183975e` (#1831) adds first-use `create_dir_all` for a missing checkout root and its
parents (`crates/farhelm-supervisor/src/service/core.rs:7685–7702`). None of those newly created directory entries has
its parent synced. Allocation subsequently syncs only the root before committing Allocated and membership
(`crates/farhelm-supervisor/src/working_copies.rs:1310–1370`). That protects the checkout's entry **inside** the root,
but does not explicitly persist the root's own entry or a newly created ancestor chain. The allocation comment at
`1310–1312` identifies the existing reason for its durability barrier: the database must not claim a directory that
never reached disk.

Verify on an owned filesystem/crash fixture with multiple missing root components, controlling the crash after
allocation has committed but before unrelated writeback makes the ancestors durable. A deterministic seam can first
establish which directory barriers precede publication. Ensure newly created root components have the required parent
barriers before publishing allocation; preserve the existing identity and symlink revalidation rules.

Possible cover: SPEC.md's precisely timed crash exception (`982–992`) names cleanup/resume outcomes, not lost checkout
folders. The attachment power-loss acceptance (`1461`, and TRIAGE_OUTCOMES.md's
`published-uploads-lack-directory-fsync.md`) applies to uploaded attachments. No exact Planned item, known bug, queue
finding or accepted decision for newly created checkout ancestors was found.

Do not cite SPEC_impl.md:1698–1700 as a blanket durability promise for cloned files. That parent-directory barrier
protects preparation evidence under the supervisor state directory (`core.rs:13980–14019`). Ordinary Git clone's own
durability limitations also prevent claiming demonstrated user-file loss here. This finding is the separate omission in
the enclosing directory hierarchy and allocation boundary, with its crash consequence still unverified.
