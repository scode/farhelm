//! Startup overrides for agent builds: the first window's size and a forced appearance, so a screenshot batch can
//! capture the app light and dark, narrow and wide, without a resize or theme command in the bridge.
//!
//! NOTE: Only agent builds (the `agent` feature, which optimized builds refuse) read these values. The rest of this
//! module is compiled into every build, so that the bootstrap has one way to read the overrides and ordinary test runs
//! check the parsing and the bootstrap's use of it, but nothing installs overrides outside an agent build unless a test
//! does. These are test hooks, not settings: the real appearance choice is the user's
//! preference, which the helm's preferences will hold (the maintainer decided on 2026-11-13 that appearance is a helm
//! preference, not a per-device one), and the window's real size is its saved frame or the first-launch default.
//! Nothing here is persisted, and the app never offers it.
//!
//! The harness (`scripts/gui-agent.py start --window-size WxH --appearance light|dark`) passes the values through the
//! app's environment, the same way it passes the desktop app's other agent-build variables
//! (`crates/farhelm-desktop/src/agent.rs`). With both unset an agent build starts like any other. A value that does not
//! parse fails startup rather than being ignored, so a batch never captures the default look under a wrong label.

// The bridge's socket is unauthenticated, and on a Mac the local agent sessions run as the same user, so a build that
// contains it lets any of them click Allow on its own approval card (SPEC_impl.md, "How the rules are enforced"). Shipped
// builds and devbuilds are optimized builds (`--release`, or the `dist` profile that inherits from it), so refusing the
// feature wherever debug assertions are off keeps it out of both without having to name each profile. A debug build
// that turns debug assertions off by hand is refused too, which costs nothing: the bridge is only for development runs.
//
// The guard lives here rather than in the crate root because this module is compiled into every build and is the
// bridge's own, in the refused tier of `docs/gpui/foundation-ownership.md`; the crate root is reported wiring, where
// a change that dropped the guard would be caught only by review.
#[cfg(all(feature = "agent", not(debug_assertions)))]
compile_error!(
    "the `agent` feature (the test-automation bridge) is for debug builds only; it must never reach a release or \
     devbuild, whose optimized profiles turn debug assertions off"
);

use gpui::{App, Global, Pixels, Size, px, size};

use crate::theme::Preference;

/// The first window's size, as `WIDTHxHEIGHT` in logical pixels (for example `900x700`).
pub const WINDOW_SIZE_ENV: &str = "FARHELM_DESKTOP_AGENT_WINDOW_SIZE";

/// The appearance the app starts in, `light` or `dark`, whatever the system's appearance is.
pub const APPEARANCE_ENV: &str = "FARHELM_DESKTOP_AGENT_APPEARANCE";

/// What an agent run asked the app to start with. Installed as a global by the bootstrap before anything reads it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Overrides {
    /// Replaces the first-launch size; still fitted to the display like the default.
    pub window_size: Option<Size<Pixels>>,
    /// Replaces the starting preference (System), so the system's appearance no longer decides the palette.
    pub preference: Option<Preference>,
}

impl Global for Overrides {}

/// The overrides the app's environment asks for. A variable that is set but not valid Unicode, or does not parse, is an
/// error that names it. Agent builds only: an ordinary build, test builds included, never reads these variables.
#[cfg(feature = "agent")]
pub fn from_env() -> anyhow::Result<Overrides> {
    parse(
        var(WINDOW_SIZE_ENV)?.as_deref(),
        var(APPEARANCE_ENV)?.as_deref(),
    )
}

/// One variable's value, `None` when unset.
#[cfg(feature = "agent")]
fn var(name: &str) -> anyhow::Result<Option<String>> {
    match std::env::var(name) {
        Ok(value) => Ok(Some(value)),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(std::env::VarError::NotUnicode(_)) => anyhow::bail!("{name} is not valid Unicode"),
    }
}

/// Parse the two values. Kept apart from [`from_env`] so tests never touch the process environment.
pub fn parse(window_size: Option<&str>, appearance: Option<&str>) -> anyhow::Result<Overrides> {
    let window_size = window_size
        .map(|text| {
            let parsed = text
                .split_once('x')
                .and_then(|(w, h)| Some((w.parse::<u16>().ok()?, h.parse::<u16>().ok()?)))
                .filter(|&(w, h)| w > 0 && h > 0);
            match parsed {
                Some((w, h)) => Ok(size(px(f32::from(w)), px(f32::from(h)))),
                None => {
                    anyhow::bail!("{WINDOW_SIZE_ENV} must be WIDTHxHEIGHT in pixels, not {text:?}")
                }
            }
        })
        .transpose()?;
    let preference = appearance
        .map(|text| match text {
            "light" => Ok(Preference::Light),
            "dark" => Ok(Preference::Dark),
            _ => anyhow::bail!("{APPEARANCE_ENV} must be light or dark, not {text:?}"),
        })
        .transpose()?;
    Ok(Overrides {
        window_size,
        preference,
    })
}

/// The installed overrides, or none when nothing installed them: every build but an agent build, and a test that did
/// not install its own. Optional by design, unlike the foundations' globals ([`crate::bootstrap::installed`]).
pub fn current(cx: &App) -> Overrides {
    cx.try_global::<Overrides>().copied().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use gpui::WindowAppearance;
    use gpui_component::{Theme, ThemeMode};

    use super::*;
    use crate::theme;

    /// A batch labels each capture with the theme and size it asked for, so the values must reach the app exactly.
    #[test]
    fn both_values_parse() {
        let overrides = parse(Some("900x700"), Some("dark")).unwrap();
        assert_eq!(overrides.window_size, Some(size(px(900.), px(700.))));
        assert_eq!(overrides.preference, Some(Preference::Dark));
        assert_eq!(
            parse(None, Some("light")).unwrap().preference,
            Some(Preference::Light)
        );
    }

    /// An agent build launched without the variables (by hand, say) starts exactly like an ordinary build.
    #[test]
    fn nothing_set_overrides_nothing() {
        assert_eq!(parse(None, None).unwrap(), Overrides::default());
    }

    /// The override replaces the first-launch size the main window asks for, so a batch's narrow and wide captures
    /// really are those sizes (the test display is larger than both).
    #[gpui::test]
    fn the_main_window_asks_for_the_requested_size(cx: &mut gpui::TestAppContext) {
        cx.update(|cx| {
            let narrow = size(px(900.), px(700.));
            cx.set_global(Overrides {
                window_size: Some(narrow),
                preference: None,
            });
            let options = crate::bootstrap::window_options(cx);
            let bounds = options.window_bounds.unwrap().get_bounds();
            assert_eq!(bounds.size, narrow);
            cx.set_global(Overrides::default());
            let options = crate::bootstrap::window_options(cx);
            let bounds = options.window_bounds.unwrap().get_bounds();
            assert_eq!(bounds.size, crate::bootstrap::DEFAULT_WINDOW_SIZE);
        });
    }

    /// A forced dark appearance wins over a light system, through the bootstrap's own install: a batch's dark captures
    /// must be dark on a machine (or a headless compositor) that reports light. Before this test, dropping the override
    /// at the theme's install left every test passing; only the captures showed it.
    #[gpui::test]
    fn a_forced_dark_appearance_wins_over_a_light_system(cx: &mut gpui::TestAppContext) {
        cx.update(|cx| {
            // The premise: the test platform reports light, so without the override the launch would be light.
            assert_eq!(cx.window_appearance(), WindowAppearance::Light);
            assert!(!theme::resolve(Preference::System, cx.window_appearance()).dark);
            cx.set_global(Overrides {
                window_size: None,
                preference: Some(Preference::Dark),
            });
            crate::bootstrap::install_app_state(cx).unwrap();
            assert!(theme::palette(cx).dark);
            assert_eq!(
                Theme::global(cx).mode,
                ThemeMode::Dark,
                "Kit agrees with the palette"
            );
            // What `open_main_window` does once the window exists and reports its own (light) appearance.
            theme::follow_system(WindowAppearance::Light, cx);
            assert!(
                theme::palette(cx).dark,
                "the window's appearance must not undo it"
            );
        });
    }

    /// A typo must fail the launch: ignoring it would capture the default look under the label the batch asked for.
    #[test]
    fn a_malformed_value_is_an_error_naming_its_variable() {
        for bad in ["900", "900x", "x700", "0x700", "900x-1", "900X700", "wide"] {
            let error = parse(Some(bad), None).unwrap_err().to_string();
            assert!(error.contains(WINDOW_SIZE_ENV), "{bad}: {error}");
        }
        for bad in ["system", "Dark", ""] {
            let error = parse(None, Some(bad)).unwrap_err().to_string();
            assert!(error.contains(APPEARANCE_ENV), "{bad}: {error}");
        }
    }
}
