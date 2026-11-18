# A stuck installer can prevent later desktop update checks

Reviewed commit: c9575db61abb31bebe43da0f11a76712e18725c1

## TLDR

A stalled installer can leave the desktop updater waiting indefinitely. Later checks cannot run, and a user-requested
check can stay on Installing.

## Details

Haiku pass, checked by gpt-6.2-sol. Confidence is source-confirmed for the cited locations. The stalled condition is
conditional and was checked by source inspection.

The only update worker waits synchronously for the installer process with no overall limit. If the child stalls, the
readout can stay on Installing and later checks never run. Report a stalled install after a bounded wait while
preserving the installer’s ability to finish and release its lock.

Reviewed locations: `crates/farhelm-desktop/src/updater.rs:448-454`, :914-939 and :1592-1596. Haiku out-P1.md §4.
Suggested bucket **high**.

SPEC_impl.md:7404–7415 describes the single worker and joining ongoing work. It does not set a child-run bound. SPEC.md
“Healthy local filesystems” accepts failures from broken local storage, but this wait does not require a filesystem
failure. `BUGS.md` documents a different hang in an SSH transfer. Preserve the installer lock and avoid starting
competing installer work after the GUI's wait expires.
