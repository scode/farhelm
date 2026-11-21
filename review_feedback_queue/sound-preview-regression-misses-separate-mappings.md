# The sound preview test covers shared dispatch through one row and leaves three independent mappings uncovered

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

The preview test exercises one sound and can miss a wrong mapping on the three other Play controls.

## Details

SPEC.md:2550–2551 requires each row to play its own sound. crates/gui/farhelm-gui/src/test_support.rs:264–270 clicks the
center of exactly one named control. settings/tests.rs:1113–1133 names only Turn finished, and its fake records only the
Sound handed to it. At settings/choices.rs:365–373, each row selects one enum value through its own match arm. Lines
656–681 instantiate one handler per row, capture that selected Sound, and pass it to the shared sounds::play.
sounds.rs:101–107 dispatches only that value to the installed player, with no iteration or validation of other rows.
Automatic status playback also uses play after its own selection (sounds/owner.rs:148–169), so it does not assert the
Settings mapping. The neighboring switch test, settings/tests.rs:1135–1186, covers preference-field writes, not preview
handlers. The reviewed feature change and pinned tip have the same mappings and preview test. Exercise each Play button
against its expected Sound with the existing fake. Keep the no-write check and the case showing that a disabled
preference still permits preview.

Confidence: confirmed by inspection.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `previews/l4`.
