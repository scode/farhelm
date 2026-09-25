# The checkout name scan is case-sensitive on case-insensitive filesystems

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

On a Mac, a folder like `Bar-1` in the checkout root makes every unnamed `gh:owner/bar` launch fail with a conflict
however often the preview is refreshed.

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0443-2b597e9-b20b` (audit of Area 5, Delete, owned-checkout archival and
attachment removal) as `F10 / COR-CASE-INSENSITIVE`, tagged **possible**. Anchors and title:
`working_copies.rs:743-781`, `crates/farhelm-proto/src/github_checkout.rs:309-345`, `working_copies.rs:933-951` — On
case-insensitive filesystems the name scan misses case-variant folders, so preview proposes a name mkdir always refuses

Candidate folder names are always lowercase ASCII (for example `bar-1`), because the repository identity is lowercased
during validation. The occupancy scan (`occupied_related_names_from_entries`) compares directory entry names byte for
byte, so an existing entry `Bar-1` or `BAR-1` does not count as taking `bar-1`.

On a case-insensitive filesystem, such as macOS's default APFS, `mkdir bar-1` then fails with `EEXIST` because `Bar-1`
is there. (Per SPEC.md, the Mac runs a normal supervisor.) The create is refused as a conflict telling the user to get a
new preview, and the new preview runs the same scan and proposes `bar-1` again. For untitled clones this never ends. A
titled clone whose slug matches such an entry fails the same way until the user changes the title. Nothing is damaged;
the user just cannot create the checkout.

SPEC.md says existing entries occupy a name and that a conflict leads to a new preview. Here the new preview repeats a
path that can never be created, and the message implies that another create took it.

Open premise: this needs a supervisor on a case-insensitive volume with a case-variant entry in the root. It was not run
on macOS.

Suggested change: lowercase each entry name before comparing. Candidates are always lowercase ASCII, so on a
case-sensitive filesystem this only skips a few names that were actually free. Optionally, when `mkdir` reports a
conflict on a name the scan reported as free, name the blocking entry. Add a test with `Bar-1`.
