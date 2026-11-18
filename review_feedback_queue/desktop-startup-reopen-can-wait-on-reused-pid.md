# The startup reopen helper can wait on a reused process number

Reviewed commit: c9575db61abb31bebe43da0f11a76712e18725c1

## TLDR

If the old Farhelm process number is reused during a handoff, the reopen helper can wait indefinitely and fail to open
the replacement window.

## Details

Haiku pass, checked by gpt-6.2-sol. Confidence is source-confirmed for the cited locations. The stalled condition is
conditional and was checked by source inspection.

The app exits once it spawns the helper, which can keep polling a reused PID forever without opening the replacement
app. Give the reopen helper a bound like Restart to update, and log when the handoff expires.

Reviewed locations: `crates/farhelm-desktop/src/bundle.rs:93-108` and :214-241,
`crates/farhelm-desktop/src/updater.rs:321-346`. Haiku out-P1.md §4. Suggested bucket **other**.

`automatic-reopen-can-launch-older-replacement-app.md` covers a changed bundle rather than a stuck old-process check.
The rare-glitch filter is a possible cover for the failed reopen itself, but the detached helper can remain stuck even
after the user reopens manually.
