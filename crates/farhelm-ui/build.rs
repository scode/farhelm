//! Defines the `native_desktop` cfg: the `desktop` feature on a non-wasm
//! target, which is the one configuration that has a native window, a
//! filesystem, and processes to own.
//!
//! The predicate it replaces, `all(feature = "desktop", not(target_arch =
//! "wasm32"))`, was spelled out at every desktop-only item in the crate, two
//! conditions each time. One named cfg means a new desktop-only item cannot
//! carry only half of it. The wasm half is not redundant: it is what lets
//! both renderer features be on at once, with a wasm build taking the web
//! path (see `src/main.rs`).
//!
//! Cargo's `[target.'cfg(...)'.dependencies]` tables are evaluated before
//! build scripts run and cannot see this cfg, so `Cargo.toml` still spells the
//! predicate out where it selects dependencies.

fn main() {
    println!("cargo::rustc-check-cfg=cfg(native_desktop)");
    let desktop = std::env::var_os("CARGO_FEATURE_DESKTOP").is_some();
    let wasm = std::env::var("CARGO_CFG_TARGET_ARCH").is_ok_and(|arch| arch == "wasm32");
    if desktop && !wasm {
        println!("cargo::rustc-cfg=native_desktop");
    }
}
