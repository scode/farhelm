# Switching agents can retain an older custom model

Reviewed commit: `c3569dc51cdab52fd8cd9e7887dcf1e9b67e8718`

## TLDR

Changing the agent in a launch draft can retain its old custom model choice because the new agent offers an older
catalog entry with the same name.

## Details

The shared reconciliation gives a matching destination catalog row priority over the stored custom model owner.
Restoring older catalog rows in `456558a85` exposed this for custom spellings that were previously absent from the
destination catalog. The custom-ownership contract in `SPEC_impl.md:8692` says an agent change clears the old custom
model. Keep the owner check when the source choice was custom. Source: `crates/farhelm-proto/src/launcher.rs:186`.

Source-confirmed. The retained choice stays visible in the launch draft and still requires a launch action.

Provenance: partial trailing review `72ff71f6-2b0b-46a7-946c-7f0293f7890d`, g002/l4.
