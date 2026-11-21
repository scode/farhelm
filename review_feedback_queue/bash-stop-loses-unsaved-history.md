# Stop forces Bash down before it can save shell history

Reviewed commit: 22054a595bc3d502b64af6040ec4b7355b3893da

## TLDR

Stop and Stop and suspend can force Bash down before it saves command history configured to write on exit.

## Details

**highest**. Confirmed by source and contract inspection.

confirmed by source and documented defaults, no runtime observation. Estimated highest for unsaved user command history
whose older entries may be absent from retained terminal output. The delay alone would be high. SPEC_impl.md:4672 and
:4681–4683 describes the shared escalation sequence and offers no product-level exception for losing newly typed Bash
history. Possible_cover: the existing tab hangup code covers only extra terminal tabs. No subsequent runtime change in
the reviewed range. Turn-2 synthesis: shell-permissions/f8 is the same reaper defect in Stop and suspend. Helm
sessions/sandbox.rs:560–592 calls ordinary do_stop_session with the frozen guards before suspending, on both supported
environment providers (SPEC.md:1620–1634 and :5137–5141). The alternative discloses both actions (shell-launcher/f8,
shell-permissions/f5 and shell-presentation/f5). The regular Delete and Replace source-removal path also reaches this
signal policy: supervisor service/teardown.rs:531–561 reaps WholeSession before ending the terminal hosts, and
WholeSession still limits early SIGHUP to tab-marked processes in sweep.rs:1201–1210. This covers paths that use the
supervisor session reaper, not whole-resource destruction by an environment provider. The reviewed implementation
records the distinction between scoped and unscoped cleanup (SPEC.md:2153–2179, SPEC_impl.md:4672–4683). The history
claim is conditional on unwrapped interactive Bash with default signal handling, history enabled and saved on normal
shell exit. No assumption about Zsh, customized traps, or the exact Bash version on any Mac, and no claim of an observed
shutdown. Linux scope and portable reaper source paths support the consequence independently. The wording fix
05ae73f23eb2cbb7ab284ca42473d66a771207a9 describes letting a live shell have a chance to exit in Delete, but changes no
signals or history behavior. Keep estimated highest on the established exit-time-history route, high for the delay
alone.

Stop and suspend invokes the same Stop logic before requesting the freeze, so it also reaches this history-loss path.

Review evidence is recorded under run `328737fe-cec4-4461-9ba9-d042969d54c1` as `shell-permissions/f8`,
`shell-runtime/f4`.
