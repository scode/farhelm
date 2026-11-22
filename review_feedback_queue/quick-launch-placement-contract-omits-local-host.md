# Quick launch keeps the deliberate placement exception that the product contract omits

Reviewed commit: 1a53d9d35889d7dde96e31320cc1681dd013737d

## TLDR

The spec promises New's placement defaults, while quick launch beside a disposable session deliberately defaults to the
helm's own host.

## Details

**other.** Source inspection.

1a53d9d35889d7dde96e31320cc1681dd013737d SPEC.md:1293-1305,1405-1412,1575-1578; SPEC_impl.md:8720-8733,8739-8742;
crates/gui/farhelm-gui/src/launcher/entry.rs:24-34,89-121,171-181,490-537;
crates/gui/farhelm-gui/src/templates/dispatch.rs:414-449

A template with no placement edit uses the helm-local host beside an open disposable session, whereas New and named New
prefill a fresh environment at the source’s recipe and place. This is an expressly chosen implementation difference,
recorded in entry.rs:24-34 as a 2026-11-16 launcher-area decision to avoid making an unintended environment or replacing
a folder with its home. The direct shortcut starts from new_prefill, whose normal host lookup excludes owned hosts and
otherwise uses the helm-local host. It uses the ordinary folder default for that host and applies any template edits
before submission. Manual New calls an additional helper that seeds the fresh environment. SPEC.md:1300-1301 still says
exactly New’s defaults, and SPEC_impl refers to the shared base prefill without describing the additional manual seed.
SPEC.md:1575-1578 applies the same confirmation to recipe templates that explicitly choose a sandbox. It does not
require quick launch to implicitly choose one. SPEC.md:1409-1411 separately accepts undetectable replacement of
identity-less hosts, which is unrelated to the placement exception. The implementation choice already exists at the
exclusive range start 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e entry.rs:24-34. Treat LC-09 as a documented-behavior
mismatch, with a supported destination surprise but no demonstrated unconsented billing, disclosure, or data loss.
Preserve the deliberate behavior and clarify the product wording if retained. The proposed scope drop applies if only
directly edited contract paragraphs qualify.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `launcher-contract/LC-09`.
