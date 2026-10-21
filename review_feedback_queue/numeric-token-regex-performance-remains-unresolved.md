# Numeric-token regex performance remains unresolved

Reviewed commit: fb318dd96c596bad4ccc207ae9b945f2b6022c1a

## TLDR

Certain numeric-looking terminal output may pause file-link recognition. The regex has overlapping suffix work, but the
shipped browser engines' evaluation and practical slowdown remain unverified.

## Details

Suggested bucket: provisionally **highest** only for the unresolved security-impacting availability interpretation;
ordinary-output UX severity depends on latency and may be lower. Confidence: **confirmed structural overlap and
synchronous invocation; production-engine traversal is a missing source premise**. No practical denial of service,
latency or ordinary-input frequency was measured.

`crates/farhelm-ui/assets/terminal-files.js:74` permits hyphens in both its numeric component and optional suffix. `1.1`
followed by many hyphens and a final `@` survives cleanup at `63–69`; a conventional backtracking evaluator can retry
long suffixes. Recognition invokes it synchronously (`199–214`). The 8,192-unit input cap (`116–140`) bounds input but
does not itself prove acceptable latency. The neighboring pre-existing extension matcher also has polynomial
near-matches; do not attribute every bounded-input cost to this new guard.

The initial exponential-repartitioning allegation is refuted: mandatory dots delimit components and cannot be consumed
by the following character class. Independent auditor `g122` agreed with that narrow exclusion. It could not establish
the browser regex implementations' relevant optimization/traversal from local sources and stopped the residual
investigation incomplete. The changed Node regression uses another shape and a five-second timeout, not a browser
latency bound (`js-tests/terminal-files.test.js:191–204`).

Possible cover: pinned `SPEC.md:3318–3331`, `SPEC_impl.md:73–87` and `TRIAGE_OUTCOMES.md:11423–11450` explicitly accept
malicious or arbitrarily broken hosts causing denial of service. That exact hostile-host-only consequence is excluded.
It does not settle ordinary bounded output. No other exact coverage was found. Proposed drop: none for the conditional
ordinary-input residual; establish the engine premise before commissioning a performance fix or asserting security
impact.

Provenance: trailing-review run `a7b93df6-ca17-4bff-8608-f443575ae5e6`, `g034/c3`, independently narrowed by `g122`.
