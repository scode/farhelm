# Interrupted Restart with can display a hook setup result from a launch that never started

Reviewed commit: 1a53d9d35889d7dde96e31320cc1681dd013737d

## TLDR

An interrupted Restart with can recover the previous terminal and display the hook setup result for a replacement that
never started.

## Details

**other.** Source inspection.

1a53d9d35889d7dde96e31320cc1681dd013737d crates/farhelm-supervisor/src/store.rs:4709-4744,4823-4839;
crates/farhelm-supervisor/src/service/core.rs:12418-12449,12543-12568,12601-12608,14679-14684,14718-14729,7194-7228;
crates/farhelm-supervisor/src/service/core/host_launch.rs:417-433,463-503;
crates/farhelm-supervisor/src/service/core/host_handover.rs:93-107,767-793;
crates/gui/farhelm-gui/src/session_list/restart.rs:104-107,134-157,165-177; SPEC.md:2104-2122,3975-3979;
SPEC_impl.md:4525-4528

A structured Grok or Antigravity launch with a resume command and no capture can start a named Restart with. The path
stops the old agent and durably clears setup when claiming the new generation; the old value is saved only for a
same-process abort. A supervisor crash before the replacement host has a launch record lets startup take the older
terminal back for its history, but the UI still reads the new generation’s hook result. If the crash is before
replacement preparation, this shows NotRequested even when the previous launch had Installed or Failed. After
preparation records the new result, a crash before the new host record can instead attach the never-started
replacement’s result to the old terminal. Disabled Restart reads this state even when the old session has ended. No live
old agent or conversation loss is required. The reviewer suggested other for incorrect recovery advice; source verifies
both ordering windows. Carry the prior result across a durable handover until recovery can tell which terminal the
result belongs to.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as
`hook-persistence/hp-restart-crash-restores-old-terminal`.
