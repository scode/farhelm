# Interrupted can be superseded by inferred preparation failure

Reviewed commit: 44137d0659d28b9cadf90a8ffe96a65930035aad

## TLDR

The protocol docs promise a definitive launch error before Interrupted changes, while the supervisor can reclassify from
missing or unreadable preparation evidence.

## Details

**other.** Source inspection. Main reference `crates/farhelm-proto/src/lib.rs:868`.

#4529 adds the qualifier at proto/src/lib.rs:865–868. During boot reconciliation a retained Host row can become
Interrupted while preserving its accepted terminal address (core.rs:6648–6667). With no sentinel or usable terminal, the
same reload can check interrupted preparation (6889–6946). store.rs:3582–3597 requires the current generation and
accepted terminal address. Status.rs:520–533 returns details for incomplete, absent or unreadable preparation evidence.
The resulting inferred transition is accepted for Interrupted (store.rs:507–525). This is compatible with the product's
refusal to repeat ambiguous preparation (SPEC.md:1907–1912), but the new protocol wording understates the kinds of
evidence that can change the outcome. The existing preparation assertion at core.rs:37139–37189 verifies the
classification once an accepted host ends. Store.rs:10888–10922 independently checks direct sentinel reclassification
from Interrupted. Neither assertion covers the complete boot-plus-inference route. Specify proved shim failure and the
accepted preparation inference separately. Bucket: other, protocol documentation. Confidence: source supports the
guard/transition path. No observed reclassification. Possible coverage: none.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `recovery_contract/l23`.
