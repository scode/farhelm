# A shell-only template clears a prompt that selecting the same shell leaves intact

Reviewed commit: 22054a595bc3d502b64af6040ec4b7355b3893da

## TLDR

Applying a Bash or Zsh template can clear a draft prompt that selecting the same shell by hand would keep.

## Details

**other**. Confirmed by source and contract inspection.

Source-confirmed. SPEC.md:71–73 says applying templates is `exactly the same as making its edits by hand`; :1186–1189
requires `the same effects` and :1196–1198 says omitted fields keep whatever the launcher holds. LaunchDraft keeps the
text while unsupported (GUI launcher/draft.rs:320–328), and the hand switch at :726–755 changes the selected harness
without changing the prompt. The template path starts from that very text (templates/apply.rs:267–271), but proto
launcher.rs:748–779 clears it as soon as the chosen harness does not support it, even when the template omits the prompt
field. The GUI applies the fold's final prompt (templates/apply.rs:228–232), matching the express implementation rule at
SPEC_impl.md:8863–8864. This is a product-versus-implementation contract discrepancy, regardless of which behavior is
chosen as intended. It predates the feature for prompt-incompatible agents and now covers the newly documented
`agent: bash` and `agent: zsh` templates. Distinct from shell-launcher/f19, which concerns undisclosed submission of a
deliberately retained prompt. It concerns loss of draft text even if the user never launches. Estimated other for form
text, no higher impact established. Possible_cover: SPEC_impl.md:8863–8864 expressly specifies the template behavior,
but does not harmonize the product promise. The follow-up and pinned tip 8bcca67df1afe6eb0f184ef73b9952ed7ebb329f leave
both paths and the two spec lines unchanged.

Review evidence is recorded under run `328737fe-cec4-4461-9ba9-d042969d54c1` as `shell-contract/f22`.
