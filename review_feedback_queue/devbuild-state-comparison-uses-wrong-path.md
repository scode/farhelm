# Devbuild can apply the update default to the wrong state directory

Reviewed commit: 702e7db538e13b56124c9b6cf18477eee1159f0b

## TLDR

Equivalent paths can apply different automatic-update defaults in a devbuild. This can save an opt-out in the installed
app’s state, or leave updates on in separate development state.

## Details

The launcher compares paths at `scripts/devbuild-mac.sh:154–167` and `:317–324`. If the installed state directory does
not exist, a trailing slash can make equivalent paths compare unequal. If an override contains a symlink and a parent
component, the shell’s logical navigation can choose a different directory from the desktop’s physical traversal. The
desktop passes the selected spelling to its state opener, which seeds an unset choice only when the marker is set.
Normalize missing tails and use path semantics consistent with the desktop. The conditional classification and write
scope follow from source. The effect is established through the launcher and startup source.

Suggested bucket: other. Source confidence and remaining conditions are described above.

Provenance: trailing review 116de16d-0880-420e-9e7e-87f8b9869bf3, `devbuild-policy/f2`, `devbuild-policy/l4`.
