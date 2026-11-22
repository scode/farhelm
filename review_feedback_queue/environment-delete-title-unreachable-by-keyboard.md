# Environment Delete can hide part of its target from keyboard users

Reviewed commit: 23ba5ede59ebae6523ad7f7bfd35f4ff7dc74352

## TLDR

A long target title can clip in an environment Delete confirmation where keyboard paging reaches the warning but cannot
reveal the full title.

## Details

**other.** Source inspection.

23ba5ede59ebae6523ad7f7bfd35f4ff7dc74352 crates/gui/farhelm-gui/src/environment_ui.rs:591-606;
1a53d9d35889d7dde96e31320cc1681dd013737d crates/gui/farhelm-gui/src/session_list/delete.rs:164-195,
environment_delete.rs:180-198, prompt.rs:291-295,1063-1118, host_text.rs:402-410,554-580, components/mod.rs:676-730;
vendor/gpui-pre/src/element.rs:445-460, vendor/gpui-pre/src/elements/div.rs:3625-3634

The anchored prompt's destructive warning and quoted target title use separate shrinking regions. Answer-key paging
controls the warning's region. The title has a separate pointer-scroll region without a keyboard scroll stop, so a
sufficiently long title can hide the identifying text from a sighted keyboard user. The accessibility label supplies the
full value when read by assistive technology. Make the complete title reachable by keyboard in the confirmation. See
`session_list/delete.rs:176–195`, `session_list/environment_delete.rs:187–198`, `prompt.rs:1063–1118`, and
`SPEC_impl.md:1023–1025` at the pinned tip.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `session-prompts/SP-04`.
