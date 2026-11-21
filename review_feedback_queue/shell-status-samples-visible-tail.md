# A visibly changing shell screen can read Idle outside the sampled text tail

Reviewed commit: 22054a595bc3d502b64af6040ec4b7355b3893da

## TLDR

A visibly active shell can show Idle when its output changes above the sampled area. This can also suppress the extra
Suspend warning.

## Details

**other**. Confirmed by source and contract inspection.

A shell running a full-screen program can be marked Idle while only visible text near the top of a large terminal
changes. That false Idle can also suppress the extra activity warning before an explicit Suspend. Source-confirmed
conditional behavior, no runtime observation. Ticker.rs:1364–1392 samples only
`terminal.screen_tail(SAMPLE_TAIL_BYTES)`, screen_reader.rs:248 sets a 4096-byte limit, hosts/terminal.rs:2231–2241
keeps the end of visible text, and ticker.rs:584–608 applies the quiet streak when that substring is unchanged. Generic
returns Idle after the streak (screen_reader.rs:364–398). This is the same implementation defect as shell-runtime/f10,
with shell-runtime as owner for the sampler. The shell-specific wording (original SPEC.md:5568–5569 and pinned
:5568–5571 after 05ae73f23eb2cbb7ab284ca42473d66a771207a9) says a continuously redrawn screen reads Running, without
describing the fixed sampling region. The later wording's silent-command allowance does not cover this case
(shell-contract/f4, shell-runtime/f9). Added consequence from shell-runtime/f11, verified in GUI
session_list/environment.rs:252–265, :453–470 and :484–517: a connected environment with a fresh complete listing treats
false Idle as non-working. SPEC.md:350–355 makes the warning depend on the status and says suspension preserves running
processes and sessions. The user still explicitly chooses Suspend and no new source route to termination is established.
Bucket other remains. The implementation-source finding and the specification wording gap should be reconciled in one
item. Possible_cover: SPEC.md:2976–2989 specifies generic best-effort observation and lists existing control exceptions,
but it does not mention the fixed region and the recent shell-specific promise is more specific. Proposed_drop:
consolidation only, because `terminal.screen_tail(SAMPLE_TAIL_BYTES)` is the same underlying mechanism in
shell-runtime/f10. Keep the liveness-warning consequence in that item. No later sampler change through
8bcca67df1afe6eb0f184ef73b9952ed7ebb329f.

A falsely Idle result can also suppress the additional activity warning before Suspend. The explicit Suspend action
still requests a freeze and the affected processes remain available on Resume.

Review evidence is recorded under run `328737fe-cec4-4461-9ba9-d042969d54c1` as `shell-contract/f9`,
`shell-runtime/f10`.
