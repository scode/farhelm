# Desktop menu markers disclose native allocation addresses

Reviewed commit: 1c671d26b1a5afdfc7a12d0fb986f349578a5250

## TLDR

Opening a desktop menu exposes a native Rust allocation address to script in the app's page. This could assist a
separate native exploit after a page compromise. Neither prerequisite nor an exploit or loss of user work was
demonstrated; this is a hardening concern.

## Details

`crates/farhelm-ui/src/menu_panel.rs:317–321` formats `wiring.handles.id()` with `Debug`. The result enters
`data-menu-focus` at `app_bar.rs:499`, `hosts.rs:3610`, `list/row.rs:2196`, and `list/templates.rs:217` under the same
crate's `src/` directory.

The pinned `dioxus-signals` and `generational-box` versions are 0.7.10. In those dependencies,
`dioxus-signals/src/signal.rs:271–273` and `copy_value.rs:140–142` lead to `generational-box/src/lib.rs:83–85,344–351`.
`GenerationalBoxId` contains a storage pointer and generation (`26–28`); its formatter prints both (`35–38`), including
outside debug builds. This is an address within the storage allocation, not necessarily the signal payload's address.
Native Rust publishes its process address into the webview DOM. This need not be an address in the renderer engine's
process; an exploit would need a separate vulnerable native operation that benefits from this particular process's
layout. None was established. The browser build exposes a WASM linear-memory offset instead of a native host virtual
address.

An opaque menu identity could retain component-remount, opening and action freshness without publishing the pointer. Any
repair must preserve those distinctions rather than substituting only a position or an opening counter shared across
components.

Suggested bucket: provisionally **highest**, for conditional security relevance. Confidence: **confirmed static address
disclosure; exploit usefulness unverified**. Remote terminal text alone does not obtain page-script capability. The
evidence establishes no arbitrary memory access, reliable ASLR bypass, native execution or user-data loss.

Possible cover: compromised page script already has the device secret and broad helm API authority, including terminal
input (`TODO.md:860–871`, `TRIAGE_OUTCOMES.md:6027–6041` at the final review tip). That limits incremental impact but
does not establish equivalence to native memory-layout information. `SPEC.md:3191–3198`, “Client hardening,” wants
proportionate native hardening against hypothetical flaws. The deferred filesystem fallback in TODO's Maybe later bucket
and the ledger's `desktop-protocol-filesystem-fallback.md` concern a different capability and a repair requiring a
patched dependency. They do not explicitly accept pointer publication. No exact Planned, BUGS, queue or filter coverage
was established.

Provenance: trailing review through `fe0348ef8d4fabc3c6140a4fd89fb19dd34178b1`; menu implementation and contract
reviewers corroborated the dependency trace. A fresh independent auditor's later synthesis disagrees with excluding the
disclosure solely because the renderer already has broad authority. No runtime test was performed.

Original finding IDs: `menu-focus-contracts-1/f1`, `menu-focus-implementation-1/f1`, `menu-focus-implementation-2/f1`,
`menu-focus-tests-1/f1`, `audit-menu-native-address/f1`.
