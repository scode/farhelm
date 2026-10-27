# ID-less installation expectation is parsed as a CLI option

Reviewed commit: c4faa8adb0ad5ac009de2bc79b7efdce7905a4a1

## TLDR

The installer CLI rejects its documented ID-less expectation when the shell supplies it as a separate argument. An
ordinary published updater history reaching that boundary remains unestablished.

## Details

`crates/farhelm-supervisor/src/installation/mod.rs:689–695` formats a missing ID as `-:0:<version>`.
`scripts/install.sh:1189–1190` passes it separately after `--expect`, while `crates/farhelm/src/main.rs:1440–1443` does
not allow hyphen-prefixed values. Matching Clap 4.6.7 source documents rejection before the custom parser at
`1466–1471`. The direct installation test at `installation/install/tests.rs:461–474` bypasses that CLI boundary. Passing
`--expect=<value>` would preserve the valid form.

Suggested bucket: `other`. Confidence: confirmed internal contract defect. Older installers can produce two-field
ID-less records, but selecting v0.31.0 runs its own expectation-ignoring installer. That does not establish the new
handoff. Missing is a release-shaped running app selecting an eligible stable target containing this handoff while
observing the ID-less record; no containing prerelease publication is assumed. Possible cover: `SPEC_impl.md:4803–4821`
requires the form. Provenance: `g023/f1`; targeted later reconciliation preserves the missing
publication/selected-target premise. No ordinary upgrade failure or runtime pass is claimed.
