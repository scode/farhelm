# The rotation driver's command-echo check counts the menu row's own command name, so the echo and the menu are not independent evidence, despite what its docs and unit tests claim

Reviewed commit: 197fcf86ad658224b2de09ebf5fe4a3a1cc6ae55

## TLDR

Before pressing Enter on `/clear` or `/new`, the rotation drift test means to confirm two things separately: the agent's
input box shows the typed command, and the agent's command menu has matched it. In practice the menu row prints the
command name beside its description, so the menu alone satisfies both checks. If the screen is sampled while only part
of the command has rendered (`/` or `/n`), the test can press Enter on whatever command the menu has highlighted. For
Muse, whose `/clear` and `/new` rows have identical descriptions, a run meant to cover `/new` could run `/clear` instead
and still pass. A vendor regression in `/new` would then go unnoticed.

## Details

`typing_step` (conversation_rotation.rs at 197fcf86, the snapshot's lines 238-247) sets `echoed` when
`screen.matches(rotation.command).count() > baseline.command`, and `Matched` when that and `Baseline::menu_open` both
hold. A vendor's slash menu renders each row as the command plus its description. The module's own fixture in
`slash_command_evidence_is_counted_against_the_screen_before_typing` shows this:
`"  /clear  start a new session with empty context\n❯ /clear\n"`. So one menu row adds one to both counts, even when the
composer echo is missing or partial (`❯ /cl`). `pre_prompt_checks_agree_with_the_declared_capture_timing` asserts
`!rotation.menu.contains(rotation.command)` and its docstring says this keeps "the echo and the menu ... independent
evidence". That only checks the configured description string, not the rendered row.
`an_open_menu_without_a_counted_echo_is_not_retyped` uses a menu line with no command name
(`"  start a new session with empty context\n❯ /clear\n"`), a shape the real menus do not draw. So the unit tests pin a
property the live screen does not have. Consequences, all inferred from source and not observed: (1) A sample taken
mid-burst (the module's own KEY_RETRY doc records keystroke bursts vanishing on a loaded machine) can reach `Matched`
with the composer holding a prefix, and Enter runs the menu's highlighted entry. For Codex or Claude that is usually a
different command, which fails the case later (a false failure). (2) For Muse, `rotation_spec` gives `/clear` and `/new`
the same `menu` string, "clear terminal and start a fresh session". With a bare `/` in the composer, the `/new` row
supplies the command count and the `/clear` row supplies the menu count. If `/clear` is highlighted, Enter rotates
through `/clear`, every later check passes, and the `/new` case passes without `/new` having run. To verify: read
`typing_step` and the two unit-test fixtures named above, and compare them with any captured vendor menu screen. Fix:
count the echo only on the composer line (for example, the line holding the profile's composer glyph, or the last line
ending with the command), or require the composer line to end with the full command before Enter. Then make the unit
fixtures draw the command in the menu row, as the real menus do.

Reviewer revision notes: Revision: confirmed at the tip b2d540a8 that typing_step, Baseline and run_command are
unchanged by c382d4ed (conversation_rotation.rs:388-397 and 793-850 at the tip), so the finding applies to the resulting
code too; the conclusion is unchanged.

### Triage notes

- Recorded bucket: `other`. g38/c4 checked the same weakness on the later /resume step: it turns into a clear failure
  there, not a false pass.
- Suggested bucket (reviewer): other. An opt-in vendor-drift test's robustness and coverage claim. The false-pass path
  needs a partial-render race plus Muse's highlight order; it is not a product defect.
- Confidence: likely. Unverified premise: that a vendor's slash menu can be sampled while the composer holds a strict
  prefix of the typed command (one write of the command bytes), and, for the Muse false pass, that `/clear` is
  highlighted with only `/` or a short prefix in the composer.
- possible_cover: none
- proposed_drop: none

### Related reviewer records

#### g38/c4 (candidate): Concerns g37/f2: a partial '/resume' could run Pi's '/reload' instead; the new picker driver turns that into a clear false failure, not a false pass

Location: crates/farhelm/tests/e2e/real_agent_capture/drift/conversation_rotation.rs:401

Bears on known finding g37/f2 (the menu row alone satisfies the command 'echo'), not re-recording it. For the commands
this group added, a composer left holding a prefix such as '/re' would still show the '/resume' menu line, so
run_command would press Enter on whatever row the vendor highlights. Pi has a reload command (pi-conversation-v1.ts
lists 'reload' among session_start reasons), so that Enter could run reload rather than resume. Consequence in g38's
code: pick() then never sees the resume picker title and fails with 'the picker never opened'; settle_fork likewise
fails on a missing notice. Neither path can pass on the wrong command, because the resume requires a return to the first
conversation's resume target and the fork requires the vendor's notice plus a moved target. Set aside as adding no new
failure mode beyond g37/f2; not highest-bucket.

### Provenance

Trailing code review of `dbb153ec94149a598a747af8ab306a3df0514641..b2d540a8dacfb94140e5eb0927d78e447ce46111` (run
`ef8bcf48-a7d4-4d80-9e04-457b0ff30acc`), Claude Code `claude-opus-5-5` at high effort under persistent-review.

- g37/f2 (finding), group g37 "Conversation rotation drift: the new module", findings log line 7.
- g38/c4 (candidate), group g38 "Conversation rotation drift: pi and omp fork and resume", findings log line 5.
