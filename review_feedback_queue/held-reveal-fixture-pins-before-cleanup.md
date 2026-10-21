# Held-reveal fixture pinning precedes cleanup ownership

Reviewed commit: 56dd4d2d9a9d16a182256c73d8aad2c9e06188e9

## TLDR

If preference pinning fails during setup, the held-reveal browser regression can leave its newly created test session
behind. A failure on that path has not been injected.

## Details

`e2e/tests/clone.spec.ts:686` creates the owned session, then `689` awaits preference pinning before entering the `try`
at `692`. The session's deletion is only in `733–735`. A rejected pin therefore bypasses the cleanup scope. Arm
ownership immediately after creation, before another fallible operation. An older initscript setup was already outside
the guard; the new network operation adds this concrete failure path.

Suggested bucket: **other**, confined to owned fixtures until stack reset. Confidence: **confirmed static failure path;
actual pin failure unobserved**. No user session, security or user-work loss is alleged. Proposed drop: none.

Possible cover: no exact specification acceptance, Planned item, known BUGS entry, queue item, ledger decision or filter
was established. The narrow ownership omission differs from already recorded product selection-history behavior and from
the separate eight-block cleanup decision. Provenance: `session-history-tests-1/f2` in the trailing review through
`fe0348ef8d4fabc3c6140a4fd89fb19dd34178b1`. No runtime reproduction was performed.

Original finding IDs: `session-history-tests-1/f2`.
