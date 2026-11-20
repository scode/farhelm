# Check for updates can leave a newer release waiting for another check

Reviewed commit: af9cd816af24b2f617ce0c4d59d4b3bd3f445b9d

## TLDR

When an older update is waiting to restart, **Check for updates** can join an automatic check just after it decides to
leave that update in place. The manual check then finishes without installing the newer eligible release.

## Details

The worker reads whether there is manual interest when it decides whether to honor the automatic hold. Later it reads
the installed build and publishes completion. A manual check between those steps still sees the worker as active and
joins without scheduling another check. Completion consumes that interest using the earlier automatic result. Preserve a
manual request until a decision that took it into account completes.

At `c3569dc51cdab52fd8cd9e7887dcf1e9b67e8718`, see `crates/farhelm-desktop/src/updater.rs:803`,
`crates/farhelm-desktop/src/updater.rs:985` and `crates/farhelm-desktop/src/updater.rs:1173`. The Settings caller added
in the reviewed commit exposes this path from **Check for updates**.

Suggested bucket **other** because the user can check again. Source-confirmed from trailing reviewer g058/f4.

Provenance: partial trailing review `72ff71f6-2b0b-46a7-946c-7f0293f7890d`, g058/f4.
