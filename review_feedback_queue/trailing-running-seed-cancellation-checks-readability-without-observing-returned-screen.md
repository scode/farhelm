# Running-seed cancellation checks readability without observing the returned screen

Reviewed commit: 85cf6b2e7715c5dce446bcc95170662c1fbc8bb8

## TLDR

The strengthened assertion detects a stranded or lost emulator after the seed caller cancels. It can still pass if an
empty emulator takes the old one's place and loses the screen.

## Details

**other.** Source inspection. Main reference `crates/farhelm-supervisor/src/service/core/tests/host_terminals.rs:4550`.

host_terminals.rs:4494-4504 says cancellation lets the emulator come back; :4519-4523 waits for a unique line on the
pre-cancellation screen. After the cancel is joined, :4550-4564 reads with a unit callback, accepts any Some seed, and
checks only newly sent output. hosts/terminal.rs:2149-2185 reports success for any live slot, and
farhelm-terminal-emulator/src/snapshot/mod.rs:541-569 returns a valid snapshot even without history, so neither
assertion distinguishes a readable replacement. The production contract at hosts/terminal.rs:2429-2449,3732-3773 returns
the original emulator before :3154-3160 tries to send its reply, and SPEC.md:3281-3289 preserves retained terminal state
across viewer disconnects. Check the original line inside the changed successful read. It is on the live screen and need
not appear in the history-only Restart seed. Earlier revision fcb6fa5e4a40e5e1a4bcc505ad5ffa32dfa95b79 at
host_terminals.rs:4535-4550 already lacked a content assertion. The gap survives the tightened read rather than being
introduced by it. Bucket: other (test witness). Confidence: source-confirmed gap. Current implementation returns the
original emulator and no content loss was observed. Possible coverage: host_terminals.rs:4343-4362 checks seed contents
on the uncancelled path only.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `lifecycle_unit/f19`.
