# Provisioning replaces a symlinked destination with a regular file

Reviewed commit: 7cc06814956a1e9b6ec41f29e2e57ec57b5d2178

## TLDR

When setup or Update installs a new farhelm binary or unit file over a destination that is a symlink, the symlink is
replaced by a regular file, so the user's link layout is rewritten and the link's original target keeps the old bytes.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Narrowed from the original finding (pre-pr-review-swarm run `20260925-0602-2b597e9-f96c`, `F13 / COR-SYMLINK-MODE`). The
mode half is fixed: the remote metadata check (`provisioning/backend.rs`, the ssh branch of the artifact inspection) now
uses `stat -L`, so an unchanged symlinked destination no longer reports mode 777 and triggers a chmod through the link
or an EPERM failure on every run.

What remains: when the content differs, the install step's `mv -f tmp <destination>` replaces the symlink itself.
Symlinked destinations are ordinary (the recorded binary path comes from `command -v farhelm`, often a symlink into a
versioned directory; dotfile repositories symlink unit files). The finding suggests deciding explicitly what a symlinked
destination means: resolve it and install at the target, or refuse with a clear message. Which one is a product
decision.
