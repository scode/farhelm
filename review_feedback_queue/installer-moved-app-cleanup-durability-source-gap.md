# Moved-app cleanup retains a crash-durability source gap

Reviewed commit: 0ae92a2a7957de08c23dc6db8a10af281175a5b7

## TLDR

Interrupted cleanup can flush the app currently at the usual location rather than the app moved during installation. A
harmful second-crash history remains unestablished.

## Details

`crates/farhelm-supervisor/src/installation/install.rs:999–1011` opens the canonical app before leftover deletion at
`980–983`. Finder movement can make that a different directory from the interrupted operation's app. The missing premise
is a complete supported move/interruption/persistence/second-crash sequence producing damaging reappearance. The
Finder-movement test at `installation/install/tests.rs:292–322` completes its update and supplies no such history.

Suggested bucket: `other`; a demonstrated destructive durability route could raise it. Confidence: incomplete
source/platform lead. Possible cover: trailing-tip `SPEC_impl.md:4640–4644` permits Finder movement, while `4662–4664`
requires flushing affected directories; neither settles this exact history. Provenance: g011's moved-app prose,
`a004/f3`, later a006 discovery corroboration. No filesystem or crash experiment ran.
