# A nested OMP conversation can be accepted for an unreadable Bun foreground

Reviewed commit: 6943f94c24a193bad9433e718bbe4e1fb6b237dc

## TLDR

A nested OMP conversation can be accepted for an unreadable Bun foreground.

## Rebase context

At rebase onto `eb71c32e0f14f4569938e7da293e366aa07eae8a`, conversation reports carry ancestry recorded by the hook and
anchored to the current pane (`crates/farhelm-supervisor/src/service/report_files.rs:299–323`). Admission checks that
recorded chain once (`crates/farhelm-supervisor/src/service/core/vendor/omp.rs:194–225`); the repeated live ancestry
check described below belongs to the reviewed revision. The installed-`omp`-only safeguard and Bun/npm pane exemption
remain (`crates/farhelm-supervisor/src/procs/omp.rs:336–353`). The finding and its unverified nested-runtime premise
remain.

## Details

`F52 / COR-OMP-BUN-PANE-PROOF` — **possible** — `crates/farhelm-supervisor/src/procs/omp.rs:335` — A nested OMP
conversation can be accepted for an unreadable Bun foreground

An OMP session launched through a custom Bun or npm command could record a nested conversation as its own when the main
pane process's arguments cannot be read. OMP is the agent integration here; its reporter tells Farhelm which
conversation should be offered for Resume. New command launches declared as OMP receive the reporter through
`{farhelm_args}`, including commands classified as Bun or npm launches. The old injection path only hooked the installed
`omp` command.

To accept a report, the supervisor checks that the launch installed the current reporter and walks the reporting
process's ancestry back to the owned terminal pane. A Bun process counts as an OMP runtime only when readable arguments
identify its OMP entry point. Unreadable or over-64-KiB arguments therefore prevent the main pane from being counted. If
a readable nested interactive OMP below it loads the current reporter, that nested process becomes the only recognized
OMP runtime. The pane itself is exempt from the ordinary intermediary checks. A separate safeguard rejects a
non-reporting Bun or Node pane only for launches classified as installed `omp`, leaving Bun and npm launches admitted
under this shape. Repeating the ancestry check does not help if that same shape remains stable.

Farhelm can consequently store the nested conversation as ownership-proven and later Resume the wrong work. Require
readable evidence of a supported package launcher whenever an interpreted pane is above, rather than itself being, the
reporting runtime, across Bun and npm launch classifications. Cover the unreadable-pane refusal with a regression and
retain a genuine package-launcher positive control so supported launches still work.

Suggested bucket: highest

Possible cover: TRIAGE_OUTCOMES.md:5870–5889 records the related safeguard, but its completion criteria explicitly cover
installed `omp` launches only.

Caveats: No live OMP reproduction established that the foreground starts a separate interactive child loading the
current reporter. Ordinary delegated children are suppressed by the reporter's interactive-context gate; the finding
needs a separately interactive child that passes it. Old plain Bun launches lacked the required reporter provenance, so
the new command-launch injection creates this exposure. SPEC.md's allowance for partial OMP integration excludes
resuming the wrong conversation.

Restater note: The command-launch injection, provenance gate, unreadable-argument handling, and installed-`omp`-only
pane safeguard confirm the conditional acceptance path. The required vendor process tree and child reporter remain
unverified.

## Review context

Review: seven-day main diff `7cc0681495159e4a6abe543dffdff1acc0720675..6943f94c24a193bad9433e718bbe4e1fb6b237dc`,
correctness and security only, nofix.

Confidence as retained: **possible**. Suggested queue bucket: **highest**.

Originating reviewers and passes: `runtime_trust p2`.

Possible cover recorded during collection: Ledger5870–5889 relatedguard installedomp only..

Collection caveats: Vendorforeground startinginteractivechild withcurrentreporter unverified; no liveOMP repro;
oldplainBun lacked provenance soexposure new.

## Filed reviewer metadata

- `runtime_trust p2`: confidence as filed: `possible`. Code-path confidence: `confirmed`; end-to-end trigger is `likely`
  only if the named open premise holds: the foreground starts a distinct interactive OMP child directly, and that child
  loads the current gated reporter. No live OMP reproduction was performed. Suggested bucket as filed: `highest`.
