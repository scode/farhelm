# Retiring SSH help removes a shared focus test

Reviewed commit: e2d8e71523c7d0542e8a33b25239b22afc0f8179

## TLDR

A test no longer checks that keyboard focus moves to a remaining control when a dialog’s focused scroll region stops
being a tab stop. Trash still uses that handoff.

## Details

The removed test in the parent revision at `crates/gui/farhelm-gui/src/hosts/ssh_help/tests.rs:374–447` proved the
scroll region had focus, completed a pending read, and checked the focus destination and next Tab. The surviving Trash
case in `crates/gui/farhelm-gui/src/trash/dialog/tests.rs:612–654` checks that the scroll stop disappears without first
focusing it. The general focus test uses a disappearing button. Extend the surviving Trash or shared dialog fixture with
the missing focused-region transition. The regression is in coverage.

Suggested bucket: other. Source confidence and remaining conditions are described above.

Provenance: trailing review 116de16d-0880-420e-9e7e-87f8b9869bf3, `manual-help-tests/l2`.
