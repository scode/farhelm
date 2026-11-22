# Incomplete-template banners point below the form

Reviewed commit: 1a53d9d35889d7dde96e31320cc1681dd013737d

## TLDR

The incomplete-template message appears below the form and asks the user to complete the form below it.

## Details

**other.** Source inspection.

1a53d9d35889d7dde96e31320cc1681dd013737d SPEC.md:1336-1339,1370-1373; SPEC_impl.md:8819-8828,8940-8950;
crates/gui/farhelm-gui/src/templates/dispatch.rs:370-394,429-445;
crates/gui/farhelm-gui/src/launcher/view/search.rs:673-690;
crates/gui/farhelm-gui/src/launcher/view.rs:2583-2622,2729-2734,2741-2764

An incomplete quick launch still displays its guidance below the buttons and asks the user to complete the form below
the message. The actual form is above it. At the pinned tip, dispatch builds Note::Banner for incomplete setups, with
this direction for numbers, sequences, the marked template, the list and the switcher. The command header explicitly
excludes Banner, and the frame puts the banner in its footer notices under the buttons in both layouts. SPEC.md promises
a top banner for incomplete number-key launches. SPEC_impl says top of the notices region, which matches placement
inside the footer but does not resolve either the product promise or the backwards direction. The issue was first logged
against the command-header spec change at f6b5e5b1a39cb2b69c817a9c1f4d1153ee1f67c6, with the inherited footer placement
recorded. Keep open. No consequence relies on clipping or a rendered viewport measurement.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `launcher-contract/LC-04`.
