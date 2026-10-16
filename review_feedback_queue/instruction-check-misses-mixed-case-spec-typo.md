# Instruction checker misses its documented specification typo

Reviewed commit: 96ea626d9a502b26a722c900863c49b9a888ff7e

## TLDR

The instruction check can pass when a mandatory pointer to the implementation specification contains a typo.

## Details

Suggested bucket: `other`. Confidence: confirmed statically; synthetic fixture not executed. Source: separate
adversarial trailing reviewer. Proposed drop: none.

`scripts/check-agents-md.py:127–135` explicitly gives `SPEC_impll.md` as a typo it rejects, but the regular expression
accepts uppercase stems only. This spelling fails that expression. Because it also names neither an existing file nor a
tracked top-level entry, `resolve()` returns `None` at `162–164`, and the pointer loop ignores it at `218–220`. The
uppercase-typo fixture at `366–367` misses this case.

Add a mandatory backticked pointer to `SPEC_impll.md` in an otherwise valid synthetic instruction tree and require an
error. Preserve the intentional distinction from lowercase example fragment names when correcting the definite-path
classification. No exact Planned item, BUGS entry, queue item, ledger acceptance or filter was found.
