# Use default can clear a checkout folder that has yet to appear

Reviewed commit: `c3569dc51cdab52fd8cd9e7887dcf1e9b67e8718`

## TLDR

When Farhelm reloads a host's checkout folder after a conflicting save, Use default can clear the newly loaded override
before the editor shows it. Future managed checkouts can then use the inherited folder.

## Details

A refused save triggers a readback that replaces the editor's comparison value and increments its read generation. The
keyboard handling can draw and dispatch the refreshed button before that frame is presented. Its callback carries the
new generation, and the request compares and clears the newly loaded override. Require the frame that supplied the
override to have been presented, as `SPEC.md:5383` requires. Source:
`crates/gui/farhelm-gui/src/settings/checkout_field.rs:354` and `:397`.

The conflict readback was added in `044484d0d`. Confirmed through source inspection. The action changes a persisted
folder choice and leaves existing checkout files alone.

Provenance: partial trailing review `72ff71f6-2b0b-46a7-946c-7f0293f7890d`, g011/l3.
