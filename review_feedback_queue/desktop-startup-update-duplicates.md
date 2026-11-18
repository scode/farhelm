# Desktop startup and updates repeat shared logic

Reviewed commit: c9575db61abb31bebe43da0f11a76712e18725c1

## TLDR

Desktop startup and updates maintain separate copies of the same checks, formatting, and reopen command construction.

## Details

Haiku pass, checked by gpt-6.2-sol. Confidence is source-confirmed for the cited locations. The suggested changes were
evaluated by source inspection.

### Share the detached bundle opener

Both shell helpers poll for the old process to exit and then call the bundle opener. Share the wait-and-exec helper with
explicit options for the wait limit and opener arguments, keeping the reopen marker and updater environment handling at
their call sites.

Reviewed locations: `crates/farhelm-desktop/src/bundle.rs:214-241`, `crates/farhelm-desktop/src/updater.rs:339-346` and
:367-397. Haiku out-P1.md §1. Suggested bucket **other**.

### Use one hello probe at startup

Both startup waits pass the same hello-based discovery predicate. Share that predicate and keep the waits’ distinct
child checks, deadlines, and error handling.

Reviewed locations: `crates/farhelm-desktop/src/lib.rs:574-585` and :600-609. Haiku out-P1.md §1. Suggested bucket
**other**.

### Share the saved-window size check

Both restore paths require a 320×240 minimum and a size that fits one monitor. Put those dimensional checks in one
predicate, with the platform-specific position checks and centering at the call sites.

Reviewed locations: `crates/farhelm-desktop/src/window_state.rs:144-151` and :175-183. Haiku out-P1.md §1. Suggested
bucket **other**.

### Validate stable release versions in one place

The latest-release and revocation parsers both check that a parsed SemVer has neither a prerelease nor build metadata.
Share the stable-version predicate and retain each parser’s diagnostics.

Reviewed locations: `crates/farhelm-desktop/src/updater.rs:1394-1405`,
`crates/farhelm-desktop/src/updater/advisories.rs:84-99`. Haiku out-P1.md §1. Suggested bucket **other**.

### Build updater state-file dependencies once

The updater’s production startup and file-backed test rig assemble the same three state-file closures. Give them one
constructor so the tests exercise the production wiring as well as the readers and writer.

Reviewed locations: `crates/farhelm-desktop/src/updater.rs:432-445` and :2558-2575. Haiku out-P1.md §1. Suggested bucket
**other**.

### Desktop diagnostics and the build readout repeat peer text formatting

Both escape the same text and spell the empty and whitespace-only cases identically. Move the formatter next to the
existing shared escaping functions so both crates can call it.

Reviewed locations: `crates/farhelm-desktop/src/local_host.rs:38-51`,
`crates/gui/farhelm-gui/src/update_readout.rs:301-317`. Haiku out-P9.md §1. Suggested bucket **other**.
