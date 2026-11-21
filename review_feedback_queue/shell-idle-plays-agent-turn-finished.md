# A shell can play the Turn finished sound without an agent turn

Reviewed commit: 22054a595bc3d502b64af6040ec4b7355b3893da

## TLDR

The optional Turn finished sound can play when a Bash or Zsh screen goes quiet, even while its command is still running.

## Details

**other**. Confirmed by source and contract inspection.

Confirmed conditional source path, no runtime or audio observation. SPEC.md:1033–1036 says a shell has no agent, and
:2865–2869 describes the cue as an agent going from running to idle. Settings/choices.rs:244–246 calls the option
`Turn finished`. Sounds/owner.rs:118–169 collects all listed session statuses without inspecting the launch kind and
plays the selected cue. Sounds/rules.rs:51–63 keeps Running and Idle, :243–255 maps a Running-to-Idle edge to
`Sound::TurnFinished`, and :200–223 and :266–279 apply enabled, listing-view, active-window, priority and mute rules
without a shell exclusion. This only occurs with the preference enabled (off by default at :113–123) and the shell
eligible to ring. It uses a normal generic shell transition (shell-runtime/f8 and f9. Shell-contract/f4), independent of
missed redraws (shell-runtime/f10. Contract/f9). The 05ae73f23eb2cbb7ab284ca42473d66a771207a9 wording at pinned
SPEC.md:5568–5571 makes silence an explicit shell Idle case without changing the cue. Verified there was no later edit
to sounds or the setting through 8bcca67df1afe6eb0f184ef73b9952ed7ebb329f. Estimated bucket other (optional, misleading
cue only). Possible_cover: SPEC.md:2870–2876 governs cue visibility and priority, and SPEC_impl.md:1407–1418 assigns
status sounds to the sound owner. Neither expressly applies an agent-turn cue to shell activity. The same generic
plumbing can predate the feature for custom commands. This finding covers the newly admitted explicitly agentless shell.

Review evidence is recorded under run `328737fe-cec4-4461-9ba9-d042969d54c1` as `shell-contract/f17`.
