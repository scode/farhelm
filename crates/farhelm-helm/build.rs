//! Convert a release-populated UI directory into a compile-time
//! `include_dir!` tree. No directory means a normal development build with
//! no web UI embedded (D12/D13).
//!
//! Provisioning payloads (the musl `farhelm`/tmux binaries) are no longer
//! embedded here (D2): they are downloaded or staged from a directory at
//! runtime — see `src/provisioning/payloads.rs`. This file used to also
//! read a build-time payload-directory variable and generate byte
//! inclusions for them; that whole path was removed rather than kept
//! dormant, so a build cannot silently resurrect the old embedding by
//! exporting it. D18 retires that variable's NAME outright for the same
//! reason: the runtime replacement (`--payload-dir`, `FARHELM_HELM_PAYLOAD_DIR`)
//! deliberately does not reuse it, so an environment still exporting the old
//! one cannot silently select the new directory source either.

use std::path::PathBuf;

/// The build-time switch for D12/D13: the `dx`-built web UI's output
/// directory, so a release binary carries its own UI with no `--ui-dist`
/// needed at runtime. Unset in an ordinary developer build.
///
/// Release CI sets it: `.github/dist-build-setup.yml` builds the web bundle
/// and exports this variable into every cargo-dist build job. (That file sits
/// beside the workflows rather than among them — cargo-dist reaches it through
/// a deliberately relative `../dist-build-setup.yml`, because GitHub would try
/// to run a step list left in `.github/workflows/` as a workflow.) A
/// developer who sets it locally is deliberately making a release-shaped
/// build in the sense D13 defines, with the payload-download default that
/// comes with it.
///
/// MUST be an absolute path — `cargo` runs this build script from the crate
/// directory (`crates/farhelm-helm`), not the workspace root, so a
/// workspace-relative value would resolve under the wrong directory instead
/// of failing outright. [`embed_ui`] panics rather than silently embedding
/// whatever a relative path happened to land on.
const UI_ENV: &str = "FARHELM_UI_DIST";

fn main() {
    let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR"));
    embed_ui(&out);
}

/// Emit `$OUT_DIR/embedded_ui.rs`, `include!`d by `src/embedded_ui.rs`.
///
/// The real mechanism: this function turns on the `farhelm_embedded_ui` cfg,
/// exports `FARHELM_UI_DIST`'s canonical path as the compile-time
/// environment variable `FARHELM_EMBEDDED_UI_DIR` (via `cargo:rustc-env`,
/// not a Rust string literal — see the comment further down on why that
/// distinction matters), and writes the generated file holding one FIXED
/// line: `include_dir::include_dir!("$FARHELM_EMBEDDED_UI_DIR")`. That
/// literal is the same on every build; only the environment variable it
/// substitutes changes. So generating source here is a choice, not a
/// necessity of `include_dir!()` itself — a `#[cfg(farhelm_embedded_ui)]`
/// static with that same fixed literal could just as well live directly in
/// `src/embedded_ui.rs`. It is generated anyway so the whole embedding
/// decision — cfg, environment variable, and literal together — lives
/// entirely in this file. That cfg (`farhelm_embedded_ui`) is the only thing
/// `embedded_ui()` reads; nothing else in the crate inspects `FARHELM_UI_DIST`
/// directly, so this function is the sole place that decides whether a build
/// carries a UI at all.
///
/// The embedding is WHOLESALE: every file `include_dir!` finds under
/// `FARHELM_UI_DIST` at build time is compiled in, with no filtering by
/// name, extension, or freshness. That includes any stale content-hashed
/// generation `dx` itself leaves behind in `public/` from a previous build
/// it never cleaned up — `dx` names each asset after its own content hash
/// precisely so an old and a new build can coexist on disk, but this
/// function has no way to tell "the generation this `index.html` actually
/// references" from "leftover cruft" and embeds both. Whoever sets
/// `FARHELM_UI_DIST` is therefore responsible for pointing it at a freshly
/// produced `dx` output: `dist-build-setup.yml` removes `target/dx` before
/// running `dx build` for exactly this reason, and a developer producing a
/// release-shaped build locally (D13) should do the same first.
fn embed_ui(out: &std::path::Path) {
    println!("cargo:rerun-if-env-changed={UI_ENV}");
    // `farhelm_embedded_ui` is read via `#[cfg(...)]` in `embedded_ui.rs`, so
    // `rustc` must be told the name is intentional or an `unexpected_cfgs`
    // warning fires on every build that leaves `FARHELM_UI_DIST` unset —
    // which is every developer build. `farhelm_release_build` is read the
    // same way by `lib.rs`'s `run_with_ready` (`cfg!(farhelm_release_build)`,
    // D13): it is the one fact `production_payloads` needs to pick a
    // download source by default instead of `NoPayloads`.
    println!("cargo:rustc-check-cfg=cfg(farhelm_embedded_ui)");
    println!("cargo:rustc-check-cfg=cfg(farhelm_release_build)");

    let Some(dir) = std::env::var_os(UI_ENV) else {
        // Unset: write an empty file. `src/embedded_ui.rs` still `include!`s
        // it unconditionally, so this file must always exist even when it
        // has nothing to say.
        std::fs::write(out.join("embedded_ui.rs"), "").expect("writing empty embedded UI stub");
        return;
    };
    let dir = PathBuf::from(dir);
    // `cargo` runs every build script with its crate directory as the
    // process cwd, not the workspace root — so a relative `FARHELM_UI_DIST`
    // would silently resolve under `crates/farhelm-helm` rather than
    // wherever the caller actually meant, and `dir.join("index.html")`
    // below would either miss a real directory or, worse, hit an unrelated
    // one that happens to have its own `index.html`. Refusing a relative
    // value outright is the only way to fail loudly instead of embedding
    // the wrong tree.
    if !dir.is_absolute() {
        panic!(
            "{UI_ENV} must be an absolute path (got {}); cargo runs build scripts from the \
             crate directory, so a workspace-relative path would resolve under \
             crates/farhelm-helm",
            dir.display()
        );
    }
    // Checked here, not left for `include_dir!` to discover: that macro's
    // own errors point at generated code in `$OUT_DIR`, not at the
    // `FARHELM_UI_DIST` a CI operator actually set.
    if !dir.join("index.html").is_file() {
        panic!("{UI_ENV} is set but {} has no index.html", dir.display());
    }
    let canonical = dir
        .canonicalize()
        .unwrap_or_else(|error| panic!("canonicalizing {}: {error}", dir.display()));
    // `include_dir!`'s own literal-expansion step (see this file's
    // `cargo:rustc-env` line below) works only with UTF-8 text, so a path
    // that canonicalizes to anything else has to be refused here rather
    // than mangled into something that quietly points at the wrong tree.
    let canonical = canonical.to_str().unwrap_or_else(|| {
        panic!(
            "{UI_ENV} names a path that is not valid UTF-8 ({}); include_dir! requires a \
             UTF-8 path",
            canonical.display()
        )
    });
    // Every directive below this point embeds `canonical` verbatim into a
    // line of cargo's build-script protocol, which is itself line-oriented
    // (one directive per line of stdout) — a `\r` or `\n` inside the path
    // would either be swallowed as a spurious line break in the middle of a
    // directive or start a bogus directive of its own, both silently rather
    // than as a build failure. Refusing it here, before the first
    // path-bearing directive is printed, turns that into a loud panic
    // instead.
    if canonical.contains('\r') || canonical.contains('\n') {
        panic!(
            "{UI_ENV} names a path containing a line break ({canonical:?}); cargo's \
             build-script protocol is line-oriented and cannot carry it"
        );
    }

    println!("cargo:rustc-cfg=farhelm_embedded_ui");
    // A release build is exactly a build that embedded a UI (D13) — the one
    // fact `FARHELM_UI_DIST` establishes that later steps (payload download
    // defaults) need under a name of its own, decoupled from "a UI exists"
    // in case the two ever need to diverge.
    println!("cargo:rustc-cfg=farhelm_release_build");
    // `include_dir!` does not itself track the tree it reads, so without
    // this a change to any UI asset would go unnoticed until something
    // else forced a rebuild.
    println!("cargo:rerun-if-changed={canonical}");
    // The canonical path travels to `include_dir!` through the COMPILE-TIME
    // ENVIRONMENT, not baked into the generated source as a Rust string
    // literal. `include_dir!` takes a `$VAR`-bearing literal's token text
    // verbatim and substitutes each `$VAR` with `std::env::var(VAR)` — it
    // does not run Rust's own escaping/unescaping over either the literal
    // or the substituted value. A literal built by formatting the path with
    // `{:?}` would therefore come out wrong for any path containing `\`,
    // `"`, or `$`: `{:?}}`'s escaping (e.g. `\` -> `\\`) survives into the
    // macro's unwrapped token text unresolved, since nothing downstream
    // interprets escapes; and a literal `$` in the path would be
    // misread as the START of a substitution the macro cannot resolve.
    // Routing the path through `rustc-env` instead sidesteps all of that:
    // cargo hands rustc (and, through it, this proc macro) the raw path
    // bytes as an actual OS environment variable, with no token-level
    // escaping step to get wrong.
    println!("cargo:rustc-env=FARHELM_EMBEDDED_UI_DIR={canonical}");
    std::fs::write(
        out.join("embedded_ui.rs"),
        "pub static UI: include_dir::Dir<'static> = \
         include_dir::include_dir!(\"$FARHELM_EMBEDDED_UI_DIR\");\n",
    )
    .expect("writing embedded UI manifest");
}
