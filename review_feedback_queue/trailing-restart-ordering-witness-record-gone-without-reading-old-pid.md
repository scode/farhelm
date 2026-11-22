# Restart ordering witness can record gone without reading the old PID

Reviewed commit: 76ebed7af03dbd2a1ed896993d7a5b945f317e33

## TLDR

The test can accept a failed fixture PID read as evidence that the previous agent tree had already stopped before the
replacement host ran.

## Details

**other.** Source inspection. Main reference `crates/farhelm/tests/e2e/host_terminals.rs:362`.

host_terminals.rs:393-397 reads and verifies the fixture before Restart and clears the initial start's log, but the
replacement wrapper independently reads both PID files at :359-368. It neither checks cat nor distinguishes empty or
invalid PID from an absent process. Failed reads leave no usable operand, so the ps/zombie check does not establish a
state, and any failing kill -0 is logged as gone. The actual host executable runs after the log (:368-369), which is an
appropriate time to make the observation. The test's exact gone-gone assertion and later checks of tree.child and
tree.grand (:403-408) do not prove those were the PIDs sampled before exec. SPEC_impl.md:2065-2078 requires the stop
before replacement launch. Bind the replacement's observations to the verified fixture identities and make setup read
failures fail the witness. The initial create, which runs before the fixture writes the PID files, needs a separate
unarmed mode. Bucket: other, test coverage. Confidence: confirmed by source for the witness's classification and
ordering. No failed read or early relaunch was observed. Possible coverage: none within this changed test. f10 concerns
Stop's reply timing, not this pre-exec witness. The exact #4534 parent also treated a failing kill -0 after command
substitution as gone. #4534 adds zombie classification and a tool-availability check, so this is a retained
observability gap in a changed ordering witness, not a newly introduced product bug.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `lifecycle_e2e/f13`.
