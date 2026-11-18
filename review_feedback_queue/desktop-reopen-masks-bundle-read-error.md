# Startup can mistake an unreadable installation for removal

Reviewed commit: c9575db61abb31bebe43da0f11a76712e18725c1

## TLDR

Farhelm can say it was uninstalled when it could not read its installation directory.

## Details

Haiku pass, checked by gpt-6.2-sol. Confidence is source-confirmed for the cited locations. The suggested changes were
evaluated by source inspection.

Startup discards the bundle folder’s open error and interprets it as an uninstall when the running version is missing.
Carry the I/O error through and reserve the uninstall advice for a missing folder, a missing record, or a record that
says removal is in progress.

Reviewed locations: `crates/farhelm-desktop/src/bundle.rs:85-116`, :136-143 and :174-176. Haiku out-P1.md §3. Suggested
bucket **other**.

Possible cover is SPEC.md “Healthy local filesystems.” It accepts local failure but leaves the accuracy of this
uninstall instruction open. The queued `automatic-reopen-can-launch-older-replacement-app.md` concerns replacement
identity during a successful dispatch.
