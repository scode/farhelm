//! Convert a release-populated payload directory into compile-time byte
//! inclusions. No directory means a normal development build with no foreign
//! artifacts.

use std::fmt::Write as _;
use std::path::PathBuf;

const PAYLOAD_ENV: &str = "FARHELM_PAYLOAD_DIR";
const FILES: &[(&str, &str, &str)] = &[
    ("farhelm-x86_64-unknown-linux-musl", "Farhelm", "X86_64"),
    ("tmux-x86_64-unknown-linux-musl", "Tmux", "X86_64"),
    ("farhelm-aarch64-unknown-linux-musl", "Farhelm", "Aarch64"),
    ("tmux-aarch64-unknown-linux-musl", "Tmux", "Aarch64"),
];

fn main() {
    println!("cargo:rerun-if-env-changed={PAYLOAD_ENV}");
    let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR"));
    let root = std::env::var_os(PAYLOAD_ENV).map(PathBuf::from);
    let mut source = String::from(
        "struct EmbeddedPayload {\n    filename: &'static str,\n    kind: PayloadKind,\n    arch: PayloadArch,\n    bytes: &'static [u8],\n}\n\nconst EMBEDDED_PAYLOADS: &[EmbeddedPayload] = &[\n",
    );
    if let Some(root) = root {
        for (filename, kind, arch) in FILES {
            let path = root.join(filename);
            println!("cargo:rerun-if-changed={}", path.display());
            if !path.is_file() {
                panic!("release payload is missing: {}", path.display());
            }
            writeln!(
                source,
                "    EmbeddedPayload {{ filename: {filename:?}, kind: PayloadKind::{kind}, arch: PayloadArch::{arch}, bytes: include_bytes!({path:?}) }},",
                path = path
                    .canonicalize()
                    .unwrap_or_else(|error| panic!("canonicalizing {}: {error}", path.display()))
                    .to_string_lossy(),
            )
            .expect("writing generated Rust into a String cannot fail");
        }
    }
    source.push_str("];\n");
    std::fs::write(out.join("embedded_payloads.rs"), source)
        .expect("writing embedded payload manifest");
}
