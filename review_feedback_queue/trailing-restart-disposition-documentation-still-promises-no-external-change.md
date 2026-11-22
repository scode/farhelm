# Restart disposition documentation still promises no external change

Reviewed commit: 44137d0659d28b9cadf90a8ffe96a65930035aad

## TLDR

The implementation has contradictory descriptions of whether a failed Restart may have run a replacement.

## Details

**other.** Source inspection. Main reference `crates/farhelm-supervisor/src/service/core.rs:2742`.

#4529 changes core.rs:3255–3267 and 12692–12702 from zero external change to replacement absence or confirmed cleanup,
allowing brief execution. The same failure classification still defines Definitive as proof nothing spawned or changed
outside the process at core.rs:2731–2744 and 13121–13123. After HostSpawnFailure::Host, the shipped relaunch calls its
unwind (core/host_restart.rs:277–284). The unwind explicitly allows that the start's child briefly ran, requires
end/cleanup evidence or returns Ambiguous, and otherwise returns Definitive (748–787). This agrees with
SPEC_impl.md:2089–2099 and SPEC.md:2096–2098. Align both remaining definitions with the cleanup-based contract and
retain the separate already-published case. Bucket: other. Confidence: confirmed by source, no actual external effect
observed. Possible coverage: recovery interface.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `recovery_contract/l16`.
