# Skipped staging leftovers retain an uninstall-resurrection source gap

Reviewed commit: c4faa8adb0ad5ac009de2bc79b7efdce7905a4a1

## TLDR

Normal readable staging leftovers are now consumed before uninstall removes the app. A supported history leaving a
skipped stage that later restores the uninstalled app remains unestablished.

## Details

Historical `crates/farhelm-supervisor/src/installation/install.rs:1005–1023` restores a staged previous app before
checking an updater's expectation. At trailing tip `a2c2f8861418f8222eca096c797b9f31585ecc1a`,
`crates/farhelm/src/uninstall.rs:367–377,251–260` and `uninstall/removal.rs:178–185` run shared cleanup under
installation locks before payload removal. `installation/install.rs:1109–1150` consumes recognized/readable stages or
fails on restoration, flush or deletion errors.

The remaining skip branches at tip `1111–1128` omit unclassifiable, unopenable, unreadable or unrecognized entries.
Missing is a supported successful-uninstall history preserving such a stage, its later becoming eligible, and a stale
updater reaching restoration. Suggested bucket: `other`; confidence: incomplete remainder, with the ordinary route's
later repair confirmed. Possible cover: the unrelated accepted foreign-app deletion exception does not accept
resurrection. Provenance: `g020/f2`, `a005/f3`, g024/g026 synthesis. No generic staging-safety or reproduced
resurrection claim follows.
