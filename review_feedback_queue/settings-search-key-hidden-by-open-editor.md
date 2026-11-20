# Settings search can miss the Tensorlake key field

Reviewed commit: `c3569dc51cdab52fd8cd9e7887dcf1e9b67e8718`

## TLDR

If a credential editor is already open, choosing the Tensorlake API key result under Credentials can clear your query
and land back on that editor. The key field stays hidden.

## Details

Opening the result preserves the current Credentials editor, which replaces the overview containing the key field. The
search action clears the query before it resolves a focus handle. When the key field has no handle, the fallback focuses
Search without scheduling a later reveal. Preserve the credential draft and make the result reveal the key or explain
why it cannot. The search contract is in `SPEC.md:2398`. Source: `crates/gui/farhelm-gui/src/settings.rs:945` and
`:979`.

Introduced by `2b69c3e6c`. Source-confirmed. You can still reach the setting through ordinary navigation.

Provenance: partial trailing review `72ff71f6-2b0b-46a7-946c-7f0293f7890d`, g059/l4.
