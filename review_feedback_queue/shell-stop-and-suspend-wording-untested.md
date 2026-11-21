# Stop and suspend tests omit the visible shell-specific warning

Reviewed commit: 22054a595bc3d502b64af6040ec4b7355b3893da

## TLDR

Stop and suspend tests can pass if its confirmation goes back to calling Bash or Zsh an agent.

## Details

**other**. Confirmed by source and contract inspection.

The existing Stop and suspend tests can pass if the confirmation goes back to calling Bash or Zsh an agent.
Source-confirmed test gap: environment_tests.rs:699-743 tests the dedicated question's title, preservation of other
processes, and the captured request guards, with no assertion that the prompt identifies the stopped shell. Its inline
case at :746-761 pins agent wording and does not exercise the shell-specific copy. The changed source at
session_list/environment.rs:442-447, :523-541 and :554-569 chooses the program from the drawn row and supplies it to
both sets of text. Environment_ui.rs:775-786 draws it. SPEC.md:1997-2003 requires shell wording for Stop, and :1620-1634
and :5137-5141 require the Stop-and-suspend order and provider-wide consequence. A shell case for each answer path can
check that the visible warning names the shell and accurately describes which processes continue. Bucket other for
coverage only. Possible_cover: tests cited above already cover the shared dispatch and freeze warning. F6 is the
distinct launch approval test gap. The process-scope issue shell-runtime/f14 and shell-contract/f19 is separately
established by source. This record does not claim it is merely missing coverage. No later edit to the relevant
environment prompts or tests through pinned tip 8bcca67df1afe6eb0f184ef73b9952ed7ebb329f. No tests executed.

Review evidence is recorded under run `328737fe-cec4-4461-9ba9-d042969d54c1` as `shell-permissions/f16`.
