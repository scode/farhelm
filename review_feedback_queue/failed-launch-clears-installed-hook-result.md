# A retained failed create can say hook setup did not happen after Farhelm wrote the files

Reviewed commit: 1a53d9d35889d7dde96e31320cc1681dd013737d

## TLDR

A failed create can later say hook setup was skipped even though Farhelm wrote the persistent files.

## Details

**other.** Source inspection.

1a53d9d35889d7dde96e31320cc1681dd013737d
crates/farhelm-supervisor/src/service/core.rs:11679-11683,11788-11822,11852-11867,14679-14684,14718-14729,15015-15030,15074-15100;
crates/farhelm-supervisor/src/store.rs:4117-4159; crates/farhelm-proto/src/lib.rs:959-980;
crates/gui/farhelm-gui/src/session_list/restart.rs:79-90,104-107,134-140; SPEC.md:3975-3979;
SPEC_impl.md:4508-4515,4525-4528

A managed-checkout create can complete hook setup and reach its terminal, fail its database confirmation, then reliably
end that terminal while keeping an error row for the allocated checkout. The refusal transaction clears both warning
eligibility and the verified setup result. The error row is published with the resulting NotRequested value and no
captured conversation, so its disabled Restart explanation says Farhelm did not set up hooks and tells users to turn
setup on. The successful installation is independent of terminal confirmation and the already written files persist, so
that advice is false. The same reset loses the specific reason if hook installation failed. This finding is limited to
incorrect recovery advice on a definitive failed create. The setup was authorized, the terminal need not survive, and
disarming its silent-hook warning is appropriate. It is separate from HS-03 and HS-01, which require a surviving
terminal. The reviewer suggested other. Confidence confirmed by source for the stated branch. Keep the observed setup
result separate from warning eligibility on definitive failure, or suppress per-launch hook advice for a discarded
launch.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as
`hook-persistence/hp-definitive-create-clears-setup`.
