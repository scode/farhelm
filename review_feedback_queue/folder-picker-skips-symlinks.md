# The folder picker leaves out symlinked folders

Reviewed commit: ff6b96c9c58bdbe9698cb8e9cbb08885c18abfc3

## TLDR

The create dialog's folder picker never shows folders that are symlinks (for example `~/src` pointing at another disk),
even though typing the path works, and a single entry that vanishes mid-listing makes the whole listing fail.

## Details

Found by pre-pr-review-swarm run `20261001-0226-ff6b96c-5e95` (whole-repo audit, correctness and security reviewers
only) as `F28 / COR-BROWSE-SYMLINK`, tagged **possible**. Anchor and title:
`crates/farhelm-supervisor/src/service/core.rs:4684` — the directory picker leaves out symlinked folders, and one bad
entry fails the whole browse.

The create dialog's folder picker asks the target host's supervisor for the child folders of a directory. The supervisor
answers in `browse_directory_blocking` (`core.rs:4654-4712`). It reads the directory and keeps an entry only if the
entry's own type is "directory" (`entry.file_type().is_dir()`, `:4684-4690`). That type does not follow symlinks, so a
link like `~/src -> /data/src` is never offered, even though typing the path by hand works. Nothing tells the user
something was left out, because the reply's "truncated" flag is not set for these omissions. Separately, if getting the
type of a single entry fails (for example, a file deleted between the directory read and the type lookup), the `?` on
that call fails the entire browse instead of skipping that entry.

SPEC.md promises a directory picker for the target host. Symlinked project roots and mount points are common, and
elsewhere the supervisor explicitly treats a symlink to a directory as usable (the checkout-root resolution,
`core.rs:7249`). No spec text, comment or test says the exclusion is intended, but this is tagged possible because that
intent cannot be ruled out. If it is intended, SPEC_impl.md's BrowseDirectory paragraph should say so (a
documentation-only fix). Otherwise the suggested change is: for symlink entries, check the target with
`std::fs::metadata` and keep the entry under the link's own path when it points at a directory. Skip broken or
unreadable links and per-entry type failures instead of failing the whole browse. Add a browse test with a symlinked
child.
