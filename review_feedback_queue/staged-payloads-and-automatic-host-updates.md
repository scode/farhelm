# Whether automatic host updates can push operator-staged payloads unasked is not established

Reviewed commit: c9575db61abb31bebe43da0f11a76712e18725c1

## TLDR

A desktop app launched with an operator-staged payload directory provisions hosts from unverified local files, which
SPEC accepts because the operator chose the directory. Whether the automatic remote-host update preference can then push
those files to hosts without a per-host action was not checked.

## Details

A lead raised by the independent drop check of the GUI migration's final review (Haiku pass, 2026-11-17), while it
agreed to drop H-P1-3 (the staged payload directory skips signature checks) on the basis that "the override alone
initiates no remote change". Not investigated. Suggested queue bucket **other**: the staged directory is an explicit
launch-time operator choice (SPEC_impl.md:7790-7793, "treated as operator-trusted and are not verified, on ANY build"),
so even an automatic push stays within what the operator selected.

To settle it: trace what `auto_update_remote_hosts` (the preference, `crates/farhelm-helm/src/preferences.rs:119`)
triggers and whether that path takes payloads from `HelmArgs::payload_dir`
(`crates/farhelm-desktop/src/bundle.rs:13-32`, `crates/farhelm-desktop/src/lib.rs:457`). If it does, H-P1-3's drop basis
is narrower than stated, and SPEC should say whether automatic updates may use a staged directory; if it does not, drop
this item with that citation.
