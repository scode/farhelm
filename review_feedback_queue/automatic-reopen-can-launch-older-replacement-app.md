# Automatic reopen can launch an older replacement app

Reviewed commit: c4faa8adb0ad5ac009de2bc79b7efdce7905a4a1

## TLDR

An older bundle substituted during automatic reopening can launch instead of being refused. Native dispatch and timing
remain unmeasured; no work-loss consequence is established.

## Details

`crates/farhelm-ui/src/desktop/bundle.rs:61–76,192–200,229–239` checks eligibility, then opens a mutable bundle
pathname. Enforcement depends on the launched app honoring `FARHELM_DESKTOP_REOPENED`. The v0.28.2 source at
`e6f7e0a75bdb4d935c21eba93b22b08526853c5a:crates/farhelm-ui/src/desktop/bundle.rs:28–34,51–71` lacks that check. A newer
app's guard therefore does not authenticate whichever older app ultimately opens.

Suggested bucket: `other`. Confidence: confirmed marker absence and conditional composition; Launch Services dispatch is
not proved by the stand-in opener test. This branch precedes this startup's supervisor and helm. Possible cover: no
exact acceptance found; replacement or intentional downgrade alone does not establish older state carried across
upgrade, and a qualifying human subsecond trigger was not established. Provenance: `g021/g021-f1`. No native
reproduction ran.
