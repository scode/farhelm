# Replace with and Restart with can hide the distinguishing part of a held host name

Reviewed commit: 30441792a456f98227643624e65ae76620a5fb64

## TLDR

Replace with and Restart with can clip the source host's name with no visual way to reveal its full spelling.

## Details

**other.** Source inspection.

30441792a456f98227643624e65ae76620a5fb64 crates/gui/farhelm-gui/src/launcher/view.rs:129-136,1511-1549,2503-2507;
crates/gui/farhelm-gui/src/launcher/view/host_picker.rs:382-394,433-467

Long host names or connection-status suffixes can be clipped to a short prefix in Replace with or Restart with, leaving
look-alike hosts hard to tell apart. At parent 82c5c23129a913833d0a343026870ccbbc9579e5 host_picker.rs:322-334 the held
name is not forced into a fixed-width truncating segment. The address gives it 188 px less mark and padding and clips
the text, while the editable Host control retains the full name in help. At that commit the separate target summary
still provides a visual hover fallback (launcher/view.rs:1758-1790). At pinned tip
1a53d9d35889d7dde96e31320cc1681dd013737d host_picker.rs:386-398 still truncates the held value and
launcher/view.rs:2667-2698 names the full target only in the action’s accessible name, with generic action help on a
valid draft. Provide full hover help on the held value or let it wrap. SPEC.md:2113,2120 requires the held host to
remain visible. The reviewer suggested other, confirmed source-level visual affordance regression for sufficiently long
names; the exact clipping threshold depends on font and contents. There is no evidence of a wrong-target request.
Synthesis attribution: 30441792a456f98227643624e65ae76620a5fb64 adds the truncating held segment;
f6b5e5b1a39cb2b69c817a9c1f4d1153ee1f67c6 deletes the footer summary and its hover affordance. At the tip the held Host
itself still has an accessible label with the full escaped name (host_picker.rs:386-398 through host_text.rs:429-457),
in addition to the button’s accessible name (launcher/view.rs:1680-1702,2684-2698). The mode title is generic
(launcher/view/search.rs:76-82). The pinned contracts separately require held text in the form
(SPEC.md:1423-1427,2113-2121) and make the footer target accessible through the action name (SPEC_impl.md:8940-8950). A
local hover on the held destination would restore the visual route without requiring a footer summary. Launcher-address
has no matching held-host record. LA-07 concerns preserving source-liveness consent, a distinct concern.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `launcher-form/held-host-name-clipped`,
with `launcher-hosts/launcher-hosts-held-name-clipped`.
