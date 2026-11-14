//! Headless screenshot goldens: one per component, per dialog frame and per overlay kind (tooltip, popover, the prompt
//! over its veil), the keyboard focus ring, host-supplied text over its spoofing corpus and the notice banner, in both
//! appearances, compared pixel for pixel.
//!
//! These are test references, not documentation media: they pin what each component draws, so a change to a token, a
//! padding or a font shows up as a failing test with the new picture beside the old one, instead of drifting unnoticed.
//! They render with the bundled fonts only and the platform's headless renderer (software Vulkan on Linux), so the same
//! tree gives the same pixels from run to run; a different renderer version can still move antialiased edges, which is
//! why references are per platform and rewritten deliberately. References live under `tests/goldens/<os>/`. They are
//! ignored in ordinary test runs because they need that renderer selected; run them with `scripts/check-gui-goldens.py`
//! (`--update` rewrites the references after an intended change).

use std::path::Path;

use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, FocusHandle, IntoElement, ParentElement,
    Render, Styled, Window, WindowAppearance, div, point, px, size,
};
use gpui_component::Root;

use crate::assets::Icon;
use crate::components::{
    Button, DialogFrame, HoverHelp, IconButton, Role, TextInput, TextInputState, Tier,
};
use std::cell::RefCell;
use std::rc::Rc;

use crate::focus::{self, Mark};
use crate::host_text::{self, HostSentence, HostText};
use crate::layers::{self, Anchor, Popover, TooltipOwner};
use crate::notices::Notice;
use crate::prompt::{self, Answer, Question};
use crate::test_support;
use crate::theme::{self, Preference};

/// How many frames a scene may take to settle before the test gives up: icons decode on the background executor, so
/// the first frame can lack them. Two consecutive identical captures is the readiness signal; a scene that never
/// settles is a failure that says so, not a flaky pixel mismatch.
const MAX_SETTLE_FRAMES: usize = 6;

/// One scene: a name, its size in logical pixels, what it draws, and what it sets up once its window exists.
struct Scene {
    name: &'static str,
    size: (f32, f32),
    draw: fn(&SceneState, &mut Window, &mut App) -> AnyElement,
    setup: fn(&SceneState, &mut Window, &mut App),
}

/// State a scene keeps across frames: a text input's editable state, the owner token a tooltip scene shows its tooltip
/// for (it must stay alive, or the layer manager takes the tooltip down), the anchor a popover scene opens its menu
/// below (recorded when the scene draws), and the focus handle the focus-ring scene's button takes.
#[derive(Clone)]
struct SceneState {
    input: TextInputState,
    tooltip_owner: Entity<TooltipOwner>,
    anchor: Rc<RefCell<Option<Anchor>>>,
    ring: FocusHandle,
}

/// The view a scene is drawn in: the window's ground, then the scene, centered, with the layer manager's overlay.
struct SceneView {
    scene: &'static Scene,
    state: SceneState,
    _layers: layers::Attachment,
    focus: focus::Attachment,
}

impl Render for SceneView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::palette(cx);
        focus::root(div(), &self.focus)
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .p(px(12.))
            .bg(theme::color(palette.bg_0))
            .font_family(crate::assets::UI_FONT)
            .child((self.scene.draw)(&self.state, window, cx))
            .children(layers::overlay(window, cx))
    }
}

/// Scenes with nothing to set up.
fn no_setup(_: &SceneState, _: &mut Window, _: &mut App) {}

/// Every button tier, a disabled main action and an icon button, in one row.
fn buttons(_: &SceneState, _: &mut Window, _: &mut App) -> AnyElement {
    let help = || HoverHelp::tooltip("Golden scene control");
    div()
        .flex()
        .gap(px(8.))
        .items_center()
        .child(Button::new("ghost", "Ghost", help(), Role::Button))
        .child(Button::new("neutral", "Cancel", help(), Role::Button).tier(Tier::Neutral))
        .child(Button::new("primary", "Save", help(), Role::Button).tier(Tier::Primary))
        .child(
            Button::new("primary-off", "Save", help(), Role::Button)
                .tier(Tier::Primary)
                .disabled(true),
        )
        .child(
            Button::new("danger", "Delete", help(), Role::Button)
                .icon(Icon::Trash)
                .tier(Tier::Danger),
        )
        .child(IconButton::new(
            "more",
            Icon::More,
            "More actions",
            help(),
            Role::Button,
        ))
        .into_any_element()
}

/// Put a value in the text input, so the golden pins the font and size Kit's input draws with, not only its box.
fn text_input_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    state.input.set_text("api-refactor", window, cx);
}

/// A labeled text input holding a value.
fn text_input(state: &SceneState, _: &mut Window, _: &mut App) -> AnyElement {
    div()
        .w(px(280.))
        .child(TextInput::new(
            "title",
            &state.input,
            "Session title",
            HoverHelp::tooltip("The session's new title"),
        ))
        .into_any_element()
}

/// The dialog frame with a title, a summary, a short body and a pinned Cancel/Delete pair.
fn dialog(_: &SceneState, _: &mut Window, _: &mut App) -> AnyElement {
    DialogFrame::new(
        "delete",
        "Delete session?",
        div().child(
            "The session's terminal and its history are removed. The working directory stays.",
        ),
        vec![
            Button::new(
                "cancel",
                "Cancel",
                HoverHelp::tooltip("Keep the session"),
                Role::Button,
            )
            .tier(Tier::Neutral),
            Button::new(
                "confirm",
                "Delete",
                HoverHelp::tooltip("Delete the session"),
                Role::Button,
            )
            .tier(Tier::Danger),
        ],
    )
    .summary("api-refactor, on this machine")
    .into_any_element()
}

/// The tooltip scene's help: long enough to wrap inside the tooltip's 280-pixel cap.
const RESTART_HELP: &str = "Stop the agent and start it again in the same terminal, keeping the session's history and working directory.";

/// A neutral button, and its tooltip as the layer manager draws it (shadow, padding, wrapping inside the 280-pixel cap).
fn tooltip(_: &SceneState, _: &mut Window, _: &mut App) -> AnyElement {
    div()
        .flex()
        .items_start()
        .w(px(380.))
        .h(px(120.))
        .child(
            Button::new(
                "restart",
                "Restart",
                HoverHelp::tooltip(RESTART_HELP),
                Role::Button,
            )
            .tier(Tier::Neutral),
        )
        .into_any_element()
}

/// Show a long tooltip as if the pointer had come to rest over the button.
fn tooltip_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    layers::show_tooltip_at(
        &state.tooltip_owner,
        RESTART_HELP.into(),
        point(px(40.), px(30.)),
        window,
        cx,
    );
}

/// A neutral button and its ring, as a keyboard user sees the button after tabbing to it.
fn focus_ring(state: &SceneState, _: &mut Window, _: &mut App) -> AnyElement {
    div()
        .flex()
        .gap(px(12.))
        .child(
            Button::new(
                "cancel",
                "Cancel",
                HoverHelp::tooltip("Close"),
                Role::Button,
            )
            .tier(Tier::Neutral)
            .focus(&state.ring),
        )
        .child(
            Button::new("other", "Save", HoverHelp::tooltip("Save"), Role::Button)
                .tier(Tier::Primary),
        )
        .into_any_element()
}

/// Press Tab as the keyboard would, so the first button takes focus with the keyboard as the last input.
fn focus_ring_setup(_: &SceneState, window: &mut Window, cx: &mut App) {
    window.activate_window();
    window.dispatch_event(
        gpui::PlatformInput::KeyDown(gpui::KeyDownEvent {
            native_key: None,
            keystroke: gpui::Keystroke {
                key: "tab".into(),
                ..Default::default()
            },
            is_held: false,
            prefer_character_input: false,
        }),
        cx,
    );
}

/// The connection-lost notice as the main view draws it, full width, with its Dismiss button focused from the keyboard
/// (the scene's setup is the focus-ring scene's Tab), so the golden pins the banner's red band and the white ring its
/// button wears there instead of the accent ring.
fn notice_banner(_: &SceneState, _: &mut Window, cx: &mut App) -> AnyElement {
    div()
        .w_full()
        .child(crate::main_view::banner(Notice::ConnectionLost, cx))
        .into_any_element()
}

/// A ghost button that a menu opens below.
fn popover(state: &SceneState, window: &mut Window, cx: &mut App) -> AnyElement {
    let anchor = Anchor::for_element("menu-anchor", window, cx);
    state.anchor.replace(Some(anchor.clone()));
    div()
        .w(px(300.))
        .h(px(160.))
        .child(
            div()
                .relative()
                .w(px(90.))
                .child(Button::new(
                    "actions",
                    "Actions",
                    HoverHelp::tooltip("More actions"),
                    Role::Button,
                ))
                .child(anchor.track()),
        )
        .into_any_element()
}

/// The popover's content: a menu panel with three items.
struct MenuScene;

impl Render for MenuScene {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let item = |id: &'static str, label: &'static str| {
            Button::new(id, label, HoverHelp::tooltip(label), Role::MenuItem)
        };
        layers::panel(cx)
            .w(px(180.))
            .child(item("rename", "Rename…"))
            .child(item("restart", "Restart"))
            .child(item("delete", "Delete…"))
    }
}

/// Open the menu below the anchor the scene drew.
fn popover_setup(state: &SceneState, window: &mut Window, cx: &mut App) {
    let Some(anchor) = state.anchor.borrow().clone() else {
        return;
    };
    let view = cx.new(|_| MenuScene);
    let mark = Mark::take(cx);
    let container = cx.focus_handle();
    layers::open_popover(
        Popover {
            view: view.into(),
            container: container.clone(),
            start: container,
            anchor,
        },
        &mark,
        window,
        cx,
    );
}

/// Nothing on the page: the prompt scene draws everything through the layer manager.
fn empty(_: &SceneState, _: &mut Window, _: &mut App) -> AnyElement {
    div().into_any_element()
}

/// A delete confirmation, asked through the prompt over its veil.
fn prompt_setup(_: &SceneState, window: &mut Window, cx: &mut App) {
    let mark = Mark::take(cx);
    let question = Question::new(
        "Delete session?",
        Answer {
            label: "Delete".into(),
            help: HoverHelp::tooltip("Delete the session"),
            tier: Tier::Danger,
        },
        "api-refactor".to_string(),
    )
    .summary("api-refactor, on this machine")
    .line("The session's terminal and its history are removed. The working directory stays.");
    prompt::ask(question, None, &mark, |_, _, _| {}, window, cx);
}

/// The host-text element over the spoofing corpus, one value per line, then a sentence mixing the app's words with two
/// host values: every hidden or direction-changing character shows as `<U+XXXX>`.
///
/// Only the corpus entries the bundled Latin fonts can draw are pictured: the goldens render with the bundled fonts
/// alone (so a font installed on the machine cannot change a result), and goldens of fallback fonts are not in scope
/// yet, so a CJK or Hebrew value would only picture missing-glyph boxes. The unit tests in `host_text` cover the whole
/// corpus.
fn host_text(_: &SceneState, _: &mut Window, _: &mut App) -> AnyElement {
    let title = HostText::from_host("invoice\u{202E}fdp.exe");
    let host = HostText::from_host("build\u{200B}box");
    div()
        .flex()
        .flex_col()
        .gap_1()
        .children(
            host_text::tests::SPOOFING_CORPUS
                .iter()
                .filter(|(_, shown)| shown.chars().all(|ch| ch < '\u{0250}'))
                .map(|(raw, _)| host_text::element(&HostText::from_host(*raw))),
        )
        .child(
            HostSentence::new("host-text-sentence")
                .text("Delete ")
                .host(&title)
                .text(" on ")
                .host(&host)
                .text("?"),
        )
        .into_any_element()
}

static SCENES: [Scene; 9] = [
    Scene {
        name: "buttons",
        size: (560., 64.),
        draw: buttons,
        setup: no_setup,
    },
    Scene {
        name: "text-input",
        size: (320., 64.),
        draw: text_input,
        setup: text_input_setup,
    },
    Scene {
        name: "dialog",
        size: (540., 300.),
        draw: dialog,
        setup: no_setup,
    },
    Scene {
        name: "tooltip",
        size: (400., 140.),
        draw: tooltip,
        setup: tooltip_setup,
    },
    Scene {
        name: "focus-ring",
        size: (260., 64.),
        draw: focus_ring,
        setup: focus_ring_setup,
    },
    Scene {
        name: "popover",
        size: (340., 200.),
        draw: popover,
        setup: popover_setup,
    },
    Scene {
        name: "prompt",
        size: (560., 320.),
        draw: empty,
        setup: prompt_setup,
    },
    Scene {
        name: "host-text",
        size: (420., 440.),
        draw: host_text,
        setup: no_setup,
    },
    Scene {
        name: "notice-banner",
        size: (760., 64.),
        draw: notice_banner,
        setup: focus_ring_setup,
    },
];

/// Draw and capture `window` until two consecutive captures match, at most [`MAX_SETTLE_FRAMES`] times.
fn settle(
    cx: &mut gpui::HeadlessAppContext,
    window: gpui::AnyWindowHandle,
    name: &str,
) -> anyhow::Result<image::RgbaImage> {
    let mut previous = None;
    for _ in 0..MAX_SETTLE_FRAMES {
        cx.run_until_parked();
        cx.update_window(window, |_, window, cx| window.draw(cx).clear(cx))?;
        let capture = cx.capture_screenshot(window)?;
        if previous.as_ref() == Some(&capture) {
            return Ok(capture);
        }
        previous = Some(capture);
    }
    anyhow::bail!("{name} never settled: {MAX_SETTLE_FRAMES} frames, no two in a row the same")
}

/// Every scene in both appearances matches its reference exactly. Run through `scripts/check-gui-goldens.py`, which
/// selects the headless renderer; `FARHELM_UPDATE_GOLDENS=1` (set by that script's `--update`) rewrites references.
#[test]
#[ignore = "needs the headless renderer selected; run scripts/check-gui-goldens.py"]
fn component_goldens() -> anyhow::Result<()> {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let references = manifest.join("tests/goldens").join(std::env::consts::OS);
    // Captures go beside the test binary, inside the build directory, so a failure leaves the new picture to compare.
    let captures = std::env::current_exe()?
        .parent()
        .map(|deps| deps.join("gui-goldens"))
        .ok_or_else(|| anyhow::anyhow!("the test binary has no directory"))?;
    let update = std::env::var_os("FARHELM_UPDATE_GOLDENS").is_some_and(|value| value == "1");
    std::fs::create_dir_all(&captures)?;
    if update {
        std::fs::create_dir_all(&references)?;
    }
    let mut failures = Vec::new();
    for scene in &SCENES {
        for (appearance, preference) in [("light", Preference::Light), ("dark", Preference::Dark)] {
            let name = format!("{}--{appearance}.png", scene.name);
            let mut cx = test_support::headless()?;
            cx.update(|cx| {
                theme::follow_system(WindowAppearance::Light, cx);
                theme::set_preference(preference, cx);
            });
            let mut view = None;
            let window =
                cx.open_window(size(px(scene.size.0), px(scene.size.1)), |window, cx| {
                    let state = SceneState {
                        input: TextInputState::new(window, cx),
                        tooltip_owner: cx.new(|_| TooltipOwner),
                        anchor: Rc::new(RefCell::new(None)),
                        ring: cx.focus_handle().tab_stop(true),
                    };
                    let scene_view = cx.new(|cx| SceneView {
                        scene,
                        state,
                        _layers: layers::attach(window, cx),
                        focus: focus::attach(window, cx),
                    });
                    view = Some(scene_view.clone());
                    cx.new(|cx| Root::new(scene_view, window, cx))
                })?;
            let view =
                view.ok_or_else(|| anyhow::anyhow!("{name}: the scene view was not built"))?;
            // One frame first, so what the scene draws (an anchor's bounds, a focusable button) exists for its setup.
            cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))?;
            cx.update_window(window.into(), |_, window, cx| {
                // Cheap clones: handles and shared cells.
                let staged = view.read(cx).state.clone();
                (scene.setup)(&staged, window, cx);
            })?;
            let screenshot = settle(&mut cx, window.into(), &name)?;
            screenshot.save(captures.join(&name))?;
            let reference = references.join(&name);
            if update {
                screenshot.save(&reference)?;
                continue;
            }
            match image::open(&reference).map(|expected| expected.into_rgba8()) {
                Ok(expected) if expected == screenshot => {}
                Ok(_) => failures.push(format!(
                    "{name} differs from its reference (capture in {})",
                    captures.display()
                )),
                Err(error) => failures.push(format!("{}: {error}", reference.display())),
            }
        }
    }
    anyhow::ensure!(
        failures.is_empty(),
        "golden failures: {}",
        failures.join("; ")
    );
    Ok(())
}
