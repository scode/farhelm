//! The operating system's reduced-motion setting, read once at startup for the GPUI app's ticker.
//!
//! The GUI crates never query the platform (SPEC_impl.md, "Animation time: the ticker" takes reduced motion as an
//! input), so the desktop binary, which owns the platform glue, reads it and hands it over. It asks the same place the
//! platform's own settings write to: on macOS the Accessibility pane's "Reduce motion" (`com.apple.universalaccess`
//! `reduceMotion`), on Linux GNOME's "Animations" switch (`org.gnome.desktop.interface` `enable-animations`), which
//! GTK apps follow on other desktops too. Anything that cannot be read counts as motion allowed: animation is the
//! ordinary default, and a failed read must not stop startup.
//!
//! NOTE: Read once per launch. A change while the app runs takes effect at the next launch; following it live needs a
//! platform notification the app does not subscribe to yet.
//!
//! On the test Mac (macOS 26.6, 2026-11-13) the read finished in well under 10 ms and needed no permission prompt;
//! with Reduce motion never switched on, the key is absent and `defaults` exits 1, which counts as motion allowed. That
//! run was over ssh, not from the app's own launch, and only the "never set" case was seen.

use std::process::Command;

/// Whether the user asked the operating system to reduce motion.
pub fn read() -> bool {
    // By absolute path, where macOS always installs it, so whatever comes earlier on the launching shell's PATH cannot
    // stand in for it.
    #[cfg(target_os = "macos")]
    let (program, args): (&str, &[&str]) = (
        "/usr/bin/defaults",
        &["read", "com.apple.universalaccess", "reduceMotion"],
    );
    #[cfg(not(target_os = "macos"))]
    let (program, args): (&str, &[&str]) = (
        "gsettings",
        &["get", "org.gnome.desktop.interface", "enable-animations"],
    );
    match Command::new(program).args(args).output() {
        Ok(output) if output.status.success() => {
            reduced_from(&String::from_utf8_lossy(&output.stdout))
        }
        _ => false,
    }
}

/// Interpret the setting's printed value. macOS prints `1` when Reduce motion is on; GNOME prints `false` when
/// animations are off. Every other answer, including an unset key, means motion is allowed.
fn reduced_from(printed: &str) -> bool {
    let value = printed.trim();
    if cfg!(target_os = "macos") {
        value == "1"
    } else {
        value == "false"
    }
}

#[cfg(test)]
mod tests {
    use super::reduced_from;

    /// The two platforms spell "reduce motion" oppositely (macOS sets a reduce flag, GNOME clears an enable flag), so
    /// a swapped reading would animate exactly for the users who asked it not to.
    #[farhelm_testtrace::test]
    fn the_printed_setting_is_read_the_platforms_way_round() {
        if cfg!(target_os = "macos") {
            assert!(reduced_from("1\n"));
            assert!(!reduced_from("0\n"));
        } else {
            assert!(reduced_from("false\n"));
            assert!(!reduced_from("true\n"));
        }
        assert!(!reduced_from(""));
    }
}
