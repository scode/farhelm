# Automatic legacy update can overwrite a later Terminal choice

Reviewed commit: c4faa8adb0ad5ac009de2bc79b7efdce7905a4a1

## TLDR

An app kept open across a Terminal downgrade can choose an older installer that ignores its expected installation state,
then replace a newer Terminal installation completed while it downloads.

## Details

`crates/farhelm-ui/src/desktop/updater.rs:197–204` permits release prereleases to update. Selection compares the latest
stable release with Installed (`1153–1180,1191–1206`), permitting a target below the already running app. The updater
fetches that target's own installer (`1511–1522`). The v0.31.0 script at
`3e3e17ddf2baee1412bc0e1e5fe80e6f93f03350:scripts/install.sh:398–414,1175–1205,1256–1291` accepts the extended ownership
record but ignores the expectation and does not refuse replacing a newer version. A later Terminal install completed
during the automatic downloads can consequently be overwritten.

Suggested bucket: `high`. Confidence: confirmed conditional source composition, independently retained by `a005`. No
containing prerelease is claimed to have been published; the exact deployment history remains conditional. No security,
process-loss, database corruption, bricking or user-work-loss route was established. Occurrence was not measured.

Possible cover: trailing-tip `SPEC_impl.md:4542–4548,4820–4821` permits intentional downgrades and documents historical
expectation ignorance. Neither explicitly permits an automatic update to replace a later Terminal choice. Ledger
`TRIAGE_OUTCOMES.md:12697–12712` discarded an earlier broad issue as superseded by redesign; its same-finding scope
remains a triage question. Surviving an intentional downgrade alone does not establish the older-state-across-upgrade
filter.

Provenance: `g022/f1`, `a005/f1`. Modern installers' differing-tuple refusal remains valid; it does not settle the
selected historical installer. No runtime reproduction ran.
