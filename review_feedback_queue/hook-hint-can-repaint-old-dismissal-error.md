# The hint can repaint an obsolete dismissal failure after successful Settings saves

Reviewed commit: bb31959c2f22c96bbbb72da2e60eee4a0b288a6b

## TLDR

An old dismissal error can reappear after Settings has successfully restored the hint.

## Details

**other.** Source inspection.

bb31959c2f22c96bbbb72da2e60eee4a0b288a6b
crates/gui/farhelm-gui/src/session_pane/hook_band.rs:59-75,78-109,131-137,179-191

After a failed dismissal, the user can successfully switch the offer off and on in Settings, then see the original
failure under the returned hint. This is a stale repaint with ordinary operation delivery. The first hint request
settles as refused and clears saving at hook_band.rs:90-100 (a local Busy can also set the error at :104-106). Only
another band dismissal clears error (:88). The off preference suppresses draw (:131-137) and the restored preference
reveals the hint with the old alert because error is retained by session key alone (:59-75,179-191). Settings exposes
the same choice at crates/gui/farhelm-gui/src/settings.rs:359-361,1595-1602 and inverts the stored dismissal at
settings/choices.rs:199-203,211-219. This path needs neither a lost reply nor a new launch, and the old alert is
pane-local rather than a current supervisor status notice. It is also independent of whether the Settings control can
itself show stale state. Clear the band error when the relevant preference, run or GUI session supersedes it. Pinned
SPEC.md:3098-3101 defines the reversible offer and :5500-5513 scopes transient state on reconnect. The reviewer
suggested other. Confidence confirmed by source at bb31959c2f22c96bbbb72da2e60eee4a0b288a6b, with the same pane code at
pinned tip 1a53d9d35889d7dde96e31320cc1681dd013737d.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `hook-session/HS-03`.
