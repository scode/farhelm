# First use of an environment home can remove pre-existing spare files

Reviewed commit: 19f967247150131de3b37127df7f028669ffe269

## TLDR

The first managed checkout in an existing environment can claim and remove files already stored under the reserved spare
directory in its home.

## Details

**highest.** Source inspection. Main reference `crates/farhelm-helm/src/sessions.rs:239`.

initial recipe Folder uses home without a checkout (sessions.rs:1906–1910). The shipped row action opens Replace with on
that same host (GUI session_list/menu.rs:998–1032, 1047–1056. Launcher/draft.rs:502–512). The chooser offers Managed
checkout when the destination is editable (launcher/view/checkout.rs:940–958). sessions.rs:227–241 resolves its root to
'~'. Supervisor core.rs:9317–9391 binds the canonical target and validates it at 9453–9513. With the default spare
setting, the shim claims the slot before inspecting its existing contents and without prior spare ownership evidence
(spare_take.rs:38–87, 304–374). If inspection declines, ordinary clone may continue and leave the claim. The background
preparation may record the owner namespace after clone (service/spare_builder.rs:269–296, 325–353.
Spare_checkouts.rs:411–452). A later startup can sweep the unprotected claim inside a recorded namespace
(service/core.rs:8155–8166, repo_cache.rs:120–139, spare_checkouts.rs:526–557 and 610–618). No preview or suggestions
read itself grants this authority. The path requires a live environment with pre-existing contents at the reserved slot,
a successful namespace registration, and later unprotected claim cleanup. Bucket: highest for possible permanent loss of
user-owned work through that source-supported conditional route. Confidence: confirmed by source, no observed loss. for
the first-activation case. For an already-active checkout root, SPEC.md:5836–5839 says 'anything in that container that
Farhelm did not put there, including a folder removed and recreated, is out of band, and Farhelm may adopt it and delete
it'; SPEC_impl.md:3615–3618 says a take 'renames whatever occupies the ready slot into its claim' and startup removes a
leftover claim. Those clearly accept the established reserved namespace. The case for extending that acceptance to first
activation is SPEC.md:1734–1736, 'A session's own sandbox or container uses its configured checkout folder, with its
home as the fallback,' together with the word 'anything' above. Neither passage explicitly says that content placed
there before home became a checkout root counts as out-of-band manipulation. Retain the first-activation concern for the
commissioner to decide that scope. Require proven namespace/slot ownership on implicit first adoption if those prior
contents must be protected. Possible coverage: replaced_ready_spare_is_not_adopted covers direct record-based eviction,
not the earlier foreground claim or first activation.

Independent check of the proposed exclusion. AMBIGUOUS via 19f9672:sessions.rs:225–243 and 1904–1912,
spare_take.rs:36–65, spare_checkouts.rs:409–438, SPEC.md:1734–1736 and 5836–5839, SPEC_impl.md:3615–3618. The reserve
clauses leave debatable whether the permission to adopt and delete applies to contents that predate home's first use as
a checkout root. Retain the first-activation finding in highest for the reported conditional loss. The excerpts omit the
declined-claim branch, persistence of the namespace record, and final removal predicate. No loss was observed.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `checkout/l7`.
