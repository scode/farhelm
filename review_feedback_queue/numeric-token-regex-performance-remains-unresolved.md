# Bounded numeric-token matching can pause recognition

Reviewed commit: fb318dd96c596bad4ccc207ae9b945f2b6022c1a

## TLDR

Long numeric-looking terminal text can slow file-link recognition even within its input cap. Isolated Chromium and
WebKit measurements now establish the bounded synthetic slowdown. Ordinary-output frequency and practical security
impact remain unestablished.

## Details

Suggested bucket: the existing provisionally **highest** bucket is preserved for triage of the unresolved
security-impacting availability interpretation. The new independent assessment suggests **other** for the demonstrated
per-call cost alone; no practical denial of service has been established. Confidence: **confirmed bounded synthetic
performance cost and synchronous invocation**. Engine behavior is no longer a missing premise. These observations do not
establish ordinary occurrence, repeated-call amplification or production Mac latency.

`crates/farhelm-ui/assets/terminal-files.js:74` permits hyphens in both its numeric component and optional suffix. `1.1`
followed by many hyphens and a final `@` survives cleanup at `63–69`. Recognition invokes the guard synchronously
(`199–214`), after the 8,192-unit input cap (`116–140`). The neighboring pre-existing extension matcher also has
near-match costs; not every bounded-input cost comes from the new guard.

The continuation loaded the exact reviewed asset into owned blank headless browsers, without a Farhelm instance, server
or network request. It measured five guard calls and five full recognizer calls per input at lengths 256, 1,024, 4,096
and 8,192, plus short controls. The long near-match was `1.1` plus enough hyphens and a final `@` to reach the selected
length; the paired control began `x.1`.

At length 8,192, Chromium 156 measured **52.7–63.9 ms** for the guard and **111.5–124.9 ms** for full recognition; the
paired control's full recognition was **42.8–51.2 ms**. WebKit 27.2 measured **61–79 ms** for the guard and **85–124
ms** for full recognition, versus **24–26 ms** for the control. Short controls were at the timers' 0–0.1 ms resolution.
Review agents shared the machine during measurement. These samples establish a synthetic bounded slowdown, not a
universal latency bound or an estimate of how often a person encounters it. Recorder runs
`13e7413b-6c66-46d7-beae-f41196836cde` and `9fddc99e-3fe5-44a8-8a81-6f49a5523eec` both exited successfully with complete
output and no skipped substrate.

The initial exponential-repartitioning allegation remains refuted: mandatory dots delimit components and cannot be
consumed by the following character class. Auditor `g122` agrees with that narrow exclusion and has now completed later
synthesis and a separate empty discovery pass after receiving the runtime evidence. The changed Node regression uses
another shape and a five-second timeout (`js-tests/terminal-files.test.js:191–204`).

Possible cover: the original pinned `SPEC.md:3318–3331`, `SPEC_impl.md:73–87` and `TRIAGE_OUTCOMES.md:11423–11450`
explicitly accept malicious or arbitrarily broken hosts causing denial of service. That exact hostile-host-only
consequence remains excluded. It does not settle ordinary bounded output. No other exact coverage was found. Proposed
drop: none for the measured residual. Triage should judge its practical severity from the bounded measurements without
carrying forward the earlier exponential or unproved security claim.

Provenance: original trailing-review run `a7b93df6-ca17-4bff-8608-f443575ae5e6`, `g034/c3` and `g122`; same-identity
continuation and runtime evidence in run `30f46c42-a021-475f-96a8-eac06930471b`, `legacy-g122`, including the separately
audited runtime manifest. The filename is retained so existing references stay valid.
