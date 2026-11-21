//! The GUI's per-harness vocabulary: which mark an agent type (or a legacy session's stored kind) draws, the mark's
//! bundled picture and identity color, the agent's product name, and the words its status line uses for a mode without
//! checks.
//!
//! Everything the GUI does differently per agent harness is answered per harness in a few named places (the map in
//! `crates/farhelm-supervisor/src/agent_kind/mod.rs`, "How the GUI shows it"), never by comparing harnesses where
//! something is drawn. The session row and launcher read the same mark and identity ink here. Keeping those answers
//! at the app level lets other surfaces identify the same harness the same way. Each is one
//! exhaustive match over one enum, under `#[warn(clippy::wildcard_enum_match_arm)]`, so a new harness is a compile
//! error at every decision it needs here and a `_` arm fails the Clippy gate. The mark's geometry is `crate::marks`'s and the shared permission help is
//! `permission/words.rs`'s; nothing here repeats them.
//!
//! Ported from the old app's `list/row.rs` (`harness_glyph`, `harness_name`, `kind_glyph`), `launch_composer.rs`
//! (`screen_yolo_mode`) and the stylesheet's `.harness-glyph` color rules at the reference commit.

use farhelm_proto::{AgentKind, LaunchHarness};

use crate::assets::Icon;
use crate::marks::HarnessGlyph;
use crate::theme::{Palette, Token};

/// The mark an agent type draws: the glyph of the same name. SPEC.md, "Session list": the mark comes from the launch
/// (an agent launch's harness, a command launch's declared type), never from reading the command.
#[warn(clippy::wildcard_enum_match_arm)]
pub(crate) const fn glyph(harness: LaunchHarness) -> HarnessGlyph {
    match harness {
        LaunchHarness::Codex => HarnessGlyph::Codex,
        LaunchHarness::Claude => HarnessGlyph::Claude,
        LaunchHarness::Antigravity => HarnessGlyph::Antigravity,
        LaunchHarness::Muse => HarnessGlyph::Muse,
        LaunchHarness::Cursor => HarnessGlyph::Cursor,
        LaunchHarness::Goose => HarnessGlyph::Goose,
        LaunchHarness::Pi => HarnessGlyph::Pi,
        LaunchHarness::Omp => HarnessGlyph::Omp,
        LaunchHarness::Grok => HarnessGlyph::Grok,
        LaunchHarness::OpenCode => HarnessGlyph::OpenCode,
        LaunchHarness::Bash => HarnessGlyph::Bash,
        LaunchHarness::Zsh => HarnessGlyph::Zsh,
    }
}

/// The mark of a session from before launch kinds, from the integration kind the supervisor stored for it: the agent
/// it was recorded as running, or the Terminal mark for a generic one, which claims no agent. A legacy row has no
/// agent type, so Cursor and OpenCode (which run as generic) can never be told apart from a plain command here.
#[warn(clippy::wildcard_enum_match_arm)]
pub(crate) const fn kind_glyph(kind: AgentKind) -> HarnessGlyph {
    match kind {
        AgentKind::Claude => HarnessGlyph::Claude,
        AgentKind::Antigravity => HarnessGlyph::Antigravity,
        AgentKind::Codex => HarnessGlyph::Codex,
        AgentKind::Goose => HarnessGlyph::Goose,
        AgentKind::Pi => HarnessGlyph::Pi,
        AgentKind::Omp => HarnessGlyph::Omp,
        AgentKind::Grok => HarnessGlyph::Grok,
        AgentKind::Muse => HarnessGlyph::Muse,
        AgentKind::Generic => HarnessGlyph::Terminal,
    }
}

/// An agent type's name as a row's agent and permission description says it (the old `harness_name`). It is the
/// product's own name, so two differ from the launcher's chip labels and the menu header's `AgentKind::display_name`:
/// "Claude Code" and "Muse Code", where those say "Claude" and "Muse".
#[warn(clippy::wildcard_enum_match_arm)]
pub(crate) const fn name(harness: LaunchHarness) -> &'static str {
    match harness {
        LaunchHarness::Codex => "Codex",
        LaunchHarness::Claude => "Claude Code",
        LaunchHarness::Antigravity => "Antigravity",
        LaunchHarness::Muse => "Muse Code",
        LaunchHarness::Cursor => "Cursor",
        LaunchHarness::Goose => "Goose",
        LaunchHarness::Pi => "Pi",
        LaunchHarness::Omp => "OMP",
        LaunchHarness::Grok => "Grok",
        LaunchHarness::OpenCode => "OpenCode",
        LaunchHarness::Bash => "Bash",
        LaunchHarness::Zsh => "Zsh",
    }
}

/// Settings navigation IDs also identify the agent's search results. They stay stable when display names change.
pub(crate) const fn settings_button_id(harness: LaunchHarness) -> &'static str {
    match harness {
        LaunchHarness::Codex => "settings-section-codex",
        LaunchHarness::Claude => "settings-section-claude",
        LaunchHarness::Antigravity => "settings-section-antigravity",
        LaunchHarness::Muse => "settings-section-muse",
        LaunchHarness::Cursor => "settings-section-cursor",
        LaunchHarness::Goose => "settings-section-goose",
        LaunchHarness::Pi => "settings-section-pi",
        LaunchHarness::Omp => "settings-section-omp",
        LaunchHarness::Grok => "settings-section-grok",
        LaunchHarness::OpenCode => "settings-section-opencode",
        LaunchHarness::Bash => "settings-section-bash",
        LaunchHarness::Zsh => "settings-section-zsh",
    }
}

/// The agent's own words for a mode without checks, as its status line shows them, for an agent type the supervisor
/// reads a permission footer for (SPEC.md, "Session list": Claude, Grok, Muse and Cursor); `None` for the others,
/// whose `screen_yolo` evidence the row ignores, so an inconsistent or older peer's flag cannot escalate a shield the
/// supervisor never read. The words are the vendors', quoted in the shield's description; they are not matched here.
///
/// The set of agents with `Some` must track the supervisor's own list of permission-footer readers,
/// `status_line_yolo` in `crates/farhelm-supervisor/src/agent_kind/screen_reader.rs`, which the GUI cannot depend on:
/// a reader added there and not here would have its evidence ignored by the row.
#[warn(clippy::wildcard_enum_match_arm)]
pub(crate) const fn screen_yolo_mode(harness: LaunchHarness) -> Option<&'static str> {
    match harness {
        LaunchHarness::Claude => Some("bypass permissions on"),
        LaunchHarness::Grok => Some("always-approve"),
        LaunchHarness::Muse => Some("YOLO"),
        LaunchHarness::Cursor => Some("Run Everything"),
        LaunchHarness::Codex
        | LaunchHarness::Goose
        | LaunchHarness::Pi
        | LaunchHarness::Omp
        | LaunchHarness::Antigravity
        | LaunchHarness::OpenCode
        | LaunchHarness::Bash
        | LaunchHarness::Zsh => None,
    }
}

/// The bundled drawing of a mark: its geometry from `crate::marks`, generated into an SVG file
/// (the test `every_harness_icon_is_its_mark_rendered` keeps the two the same).
#[warn(clippy::wildcard_enum_match_arm)]
pub(crate) const fn icon(glyph: HarnessGlyph) -> Icon {
    match glyph {
        HarnessGlyph::Cursor => Icon::HarnessCursor,
        HarnessGlyph::Codex => Icon::HarnessCodex,
        HarnessGlyph::Claude => Icon::HarnessClaude,
        HarnessGlyph::Antigravity => Icon::HarnessAntigravity,
        HarnessGlyph::Muse => Icon::HarnessMuse,
        HarnessGlyph::Goose => Icon::HarnessGoose,
        HarnessGlyph::Pi => Icon::HarnessPi,
        HarnessGlyph::Omp => Icon::HarnessOmp,
        HarnessGlyph::Grok => Icon::HarnessGrok,
        HarnessGlyph::OpenCode => Icon::HarnessOpenCode,
        HarnessGlyph::Terminal => Icon::HarnessTerminal,
        HarnessGlyph::Bash => Icon::HarnessBash,
        HarnessGlyph::Zsh => Icon::HarnessZsh,
    }
}

/// A mark's own identity color (`--harness-<token>`), or `None` for a mark that has none and takes the foreground of
/// whatever surface draws it: Codex and Goose, whose owners' terms forbid coloring their marks, and Farhelm's own
/// Terminal mark. The old stylesheet gave those three no color, so they inherited the agent track's (`--fg-1`); which
/// foreground that is belongs to the surface, not here. Identity only: never an activity or a permission. A selected
/// row draws every mark in `--on-fill` instead, which is the caller's choice too.
#[warn(clippy::wildcard_enum_match_arm)]
pub(crate) const fn ink(glyph: HarnessGlyph, palette: &Palette) -> Option<Token> {
    match glyph {
        HarnessGlyph::Claude => Some(palette.harness[0]),
        HarnessGlyph::Muse => Some(palette.harness[1]),
        HarnessGlyph::Cursor => Some(palette.harness[2]),
        HarnessGlyph::Pi => Some(palette.harness[3]),
        HarnessGlyph::Omp => Some(palette.harness[4]),
        HarnessGlyph::Grok => Some(palette.harness[5]),
        HarnessGlyph::OpenCode => Some(palette.harness[6]),
        HarnessGlyph::Antigravity => Some(palette.harness[7]),
        HarnessGlyph::Codex
        | HarnessGlyph::Goose
        | HarnessGlyph::Terminal
        | HarnessGlyph::Bash
        | HarnessGlyph::Zsh => None,
    }
}

#[cfg(test)]
mod tests {
    use std::fmt::Write as _;

    use gpui::AssetSource as _;

    use super::*;
    use crate::assets::Assets;
    use crate::marks::{HarnessMark, Paint};
    use crate::theme;

    /// A mark as the SVG file the app bundles for it, the markup the old app inlined: its paths in `currentColor` (GPUI
    /// draws an SVG as a mask in the one color the row gives it), in one group carrying its recorded transform into the
    /// 12-unit box. The numbers print as Rust prints the recorded `f32`s, which is the literals as written in
    /// `marks.rs` without trailing zeros.
    fn rendered(mark: &HarnessMark) -> String {
        let transform = mark.transform;
        let mut svg = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 12 12\"><g transform=\"translate({} {}) \
             scale({})\">",
            transform.translate_x, transform.translate_y, transform.scale
        );
        for path in mark.paths {
            let paint = match path.paint {
                Paint::Fill => "fill=\"currentColor\"".to_owned(),
                Paint::FillEvenOdd => "fill=\"currentColor\" fill-rule=\"evenodd\"".to_owned(),
                Paint::Stroke { width } => format!(
                    "fill=\"none\" stroke=\"currentColor\" stroke-width=\"{width}\" stroke-linecap=\"round\" \
                     stroke-linejoin=\"round\""
                ),
            };
            let _ = write!(svg, "<path d=\"{}\" {paint}/>", path.d);
        }
        svg.push_str("</g></svg>\n");
        svg
    }

    /// `docs/harness-marks.md` makes `marks.rs` the one source of the marks' geometry, whose official paths must stay
    /// byte for byte what their owners published; a bundled file edited by hand, or left behind when a mark changed,
    /// would draw something the provenance record does not describe. Specified: every glyph's icon file is exactly its
    /// mark rendered by [`rendered`]. When this fails after a change to `marks.rs`, regenerate the files with
    /// [`write_harness_mark_svgs`].
    #[test]
    fn every_harness_icon_is_its_mark_rendered() {
        for glyph in HarnessGlyph::ALL {
            let bytes = Assets.load(icon(*glyph).path()).unwrap().unwrap();
            assert_eq!(
                String::from_utf8_lossy(&bytes),
                rendered(glyph.mark()),
                "{glyph:?}: regenerate with write_harness_mark_svgs"
            );
        }
    }

    /// Writes every glyph's icon file from its mark, the generator [`every_harness_icon_is_its_mark_rendered`] checks
    /// against. Ignored, since a test run must not change the checkout; run it by hand after changing `marks.rs`
    /// (`cargo test -p farhelm-gui --lib write_harness_mark_svgs -- --ignored`) and look at the files it changed.
    #[test]
    #[ignore = "writes the harness mark SVGs into the checkout; run by hand after changing marks.rs"]
    fn write_harness_mark_svgs() {
        for glyph in HarnessGlyph::ALL {
            let path = format!(
                "{}/assets/{}",
                env!("CARGO_MANIFEST_DIR"),
                icon(*glyph).path()
            );
            std::fs::write(path, rendered(glyph.mark())).unwrap();
        }
    }

    /// A token's `0xRRGGBB`, read from its `Debug` form (`Token { value: …, alpha: false }`), since a token's value is
    /// private to the theme and these tests must pin the actual hues, not only which table entry a mark reads.
    fn rgb(token: Token) -> u32 {
        let debug = format!("{token:?}");
        let value = debug
            .split("value: ")
            .nth(1)
            .and_then(|rest| rest.split(',').next())
            .unwrap();
        assert!(debug.contains("alpha: false"), "{debug}");
        value.parse().unwrap()
    }

    /// SPEC.md, "Session list": agent marks use identity colors where their owners' terms allow, and "Codex, Goose and
    /// the ordinary Terminal mark stay grey"; `docs/harness-marks.md`'s Colors table gives each hue. A swapped arm or
    /// table entry (Claude's terracotta on Muse's mark, say) gives one agent another's color, which reads as the wrong
    /// agent. Specified, in both appearances: each colored mark inks with exactly its hue from that table, and Codex,
    /// Goose and Terminal have no color of their own (`None`), so each surface draws them in its own foreground.
    #[test]
    fn each_mark_inks_with_its_own_identity_color() {
        let hues = [
            (HarnessGlyph::Claude, 0xd97757, 0xd25e39),
            (HarnessGlyph::Muse, 0x456bff, 0x456bff),
            (HarnessGlyph::Cursor, 0x2fbfaf, 0x249084),
            (HarnessGlyph::Pi, 0x4d9abf, 0x3e89ad),
            (HarnessGlyph::Omp, 0x9b4dff, 0x9b4dff),
            (HarnessGlyph::Grok, 0xd8b839, 0x987f1e),
            (HarnessGlyph::OpenCode, 0x83b94b, 0x638d37),
            (HarnessGlyph::Antigravity, 0xe85d9a, 0xc73c7a),
        ];
        for (glyph, dark, light) in hues {
            assert_eq!(
                rgb(ink(glyph, &theme::DARK).unwrap()),
                dark,
                "{glyph:?} dark"
            );
            assert_eq!(
                rgb(ink(glyph, &theme::LIGHT).unwrap()),
                light,
                "{glyph:?} light"
            );
        }
        for palette in [&theme::DARK, &theme::LIGHT] {
            for grey in [
                HarnessGlyph::Codex,
                HarnessGlyph::Goose,
                HarnessGlyph::Terminal,
            ] {
                assert_eq!(ink(grey, palette), None, "{grey:?}");
            }
        }
    }

    /// SPEC.md names the observed mode in the agent's own words, and these are the words of each vendor's status line
    /// that the supervisor's reader recognizes; a typo would put a mode no agent shows into the shield's description.
    /// Specified: Claude's, Grok's, Muse's and Cursor's words, and none for the agents the supervisor reads no
    /// permission footer for.
    #[test]
    fn each_status_line_reading_names_its_agents_own_words() {
        let read = [
            (LaunchHarness::Claude, "bypass permissions on"),
            (LaunchHarness::Grok, "always-approve"),
            (LaunchHarness::Muse, "YOLO"),
            (LaunchHarness::Cursor, "Run Everything"),
        ];
        for harness in LaunchHarness::ALL {
            let words = read
                .iter()
                .find(|(read, _)| read == harness)
                .map(|(_, words)| *words);
            assert_eq!(screen_yolo_mode(*harness), words, "{harness:?}");
        }
    }
}
