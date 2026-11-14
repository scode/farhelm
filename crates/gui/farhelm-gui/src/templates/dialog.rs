//! The Templates dialog beside New: the saved templates as a list, each with a one-line summary, beside the editor.
//!
//! SPEC.md, "Launch templates": "Templates are created, edited, duplicated, and deleted in the Templates dialog beside
//! New. A list of names and summaries sits beside the selected template's editor". Ported from the old app's
//! `TemplatesDialog` (`list/templates.rs`) and its `.templates-*` rules in `assets/app.css`, both at reference commit
//! `24bc0580`.
//!
//! Here's the TLDR of its rules:
//!
//! - **The frame.** The shared dialog frame with the Templates tile, the title "Templates", the old summary and the
//!   header's Close, 960 pixels wide at most. Its body holds the help paragraph, an optional hint line (set when a
//!   shortcut opened the dialog, [`Hint`]), the two panes and the list's unavailable line. There is no footer: the
//!   editor carries its own action row.
//! - **The list.** Every template the helm pushed, in its order (by name), each drawn as its name over its summary
//!   ([`super::summary`]); a count under the list. Names and the values a summary shows are host-supplied text, since
//!   an agent may have written them, so they are drawn through the host-text element.
//! - **Reading.** Everything comes from the store as the helm pushes it; the dialog keeps no copy. A template that
//!   changes or appears while the dialog is open shows up at once. When the helm could not read its templates the list
//!   says so with the helm's words, and keeps showing the last templates it had.
//! - **⚡ and the number keys.** Each row starts with its bolt and its number-key button ([`super::shortcuts`]). The
//!   bolt marks or unmarks the template; the key button opens a menu of No shortcut and the ten keys, each saying which
//!   template holds it. Both write the helm's shared preferences, one write at a time, and show what the store holds,
//!   never what was just clicked. A refused write is said under the panes in the helm's words.
//!
//! NOTE: Built in pieces. Not here yet, each with its own piece: the editor and its "+ New template" (until it lands
//! the rows' names are not buttons, since there is nothing for them to open), the leave question, and delete. The old
//! list's Retry beside a failed read is not ported: the helm pushes its templates again whenever they change, so there
//! is nothing for the dialog to ask for (recorded in the parity file).
//!
//! NOTE: The old dialog scrolled as one piece, header included. Here the frame keeps its header (title and Close) on
//! screen and scrolls the body, as every other dialog of the app does; at a window too short even for the header, the
//! whole card scrolls (the frame's rule).

use std::collections::HashMap;
use std::rc::Rc;

use farhelm_proto::launcher::LaunchTemplate;
use farhelm_proto::ui_channel::{Availability, Collection};
use gpui::{
    AnyElement, App, AppContext as _, Context, FocusHandle, FontWeight, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, Role, StatefulInteractiveElement as _, Styled as _,
    WeakEntity, Window, div, px, svg,
};

use super::shortcuts::{self, Bindings, KEYS};
use super::summary::{self, Part};
use crate::assets::Icon;
use crate::components::{Button, DialogFrame, HoverHelp, IconButton, MenuItem, Tier};
use crate::focus::{self, Mark, ReturnTo};
use crate::host_text::{self, HostSentence, HostText};
use crate::layers::{self, Anchor, Dialog, Dismissal, LayerId, Popover};
use crate::operations::{Command, OpKey};
use crate::session;
use crate::theme::{self, metrics};

/// `.templates-dialog`'s width: `min(960px, 100%)`.
const WIDTH: f32 = 960.;
/// `.templates-panes`: the list column (`minmax(0, 260px)`), the gap between the panes and their floor.
const LIST_WIDTH: f32 = 260.;
const PANES_GAP: f32 = 24.;
const PANES_MIN_HEIGHT: f32 = 340.;
/// `.templates-sidebar`: the list column's inner gap and its padding before its right rule.
const LIST_COLUMN_GAP: f32 = 12.;
const LIST_COLUMN_PAD_RIGHT: f32 = 18.;
/// `.templates-content`: the gap between the body's blocks.
const CONTENT_GAP: f32 = 14.;
/// `.templates-list`: the gap between rows, and a row's padding (`.templates-row`: 10 pixels by 12).
const ROW_GAP: f32 = 6.;
const ROW_PAD: (f32, f32) = (10., 12.);
/// `.templates-row`: the gap between a row's name and its summary.
const ROW_LINE_GAP: f32 = 4.;
/// `.templates-hint` and `.templates-row-summary`: 12-pixel text at a 1.5 line height.
const SMALL_LINE_HEIGHT: f32 = 18.;
/// The header tile's glyph: the Templates action glyph at its own 14-pixel size, as the old tile drew it (Settings'
/// gear is drawn larger in its tile; the old dialogs differed).
const TILE_GLYPH: f32 = 14.;

/// The header's summary, the old dialog's words.
const SUMMARY: &str = "Saved edits to combine in the session launcher";

/// The header Close's name and hover help, the old dialog's words. The help's promise about unsaved changes comes true
/// with the editor's leave question; until then there is nothing unsaved to ask about.
const CLOSE_LABEL: &str = "Close templates";
const CLOSE_HELP: &str = "Close templates; unsaved changes ask before leaving";

/// Why a shortcut opened the dialog instead of launching, said in a line above the list (the old app's
/// `quick_launch_hint` and `number_shortcut_hint`). The Templates button opens the dialog with neither.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hint {
    /// The ⚡ chord, or a sequence's ⚡, found no usable marked template.
    QuickLaunch,
    /// A number key, alone or in a sequence, has no template bound to it.
    NumberKey,
}

impl Hint {
    /// The line the dialog shows for this hint.
    pub fn text(self) -> String {
        match self {
            Hint::QuickLaunch => format!(
                "Mark a template with ⚡ to launch it directly with {}.",
                super::QUICK_LAUNCH_SHORTCUT_NAME
            ),
            Hint::NumberKey => format!(
                "Give a template a number key with the key button beside its ⚡; {} launches it directly.",
                super::number_key_name("number")
            ),
        }
    }
}

/// The help paragraph above the panes, the old dialog's words with this platform's chords.
fn help_text() -> String {
    format!(
        "A template is a named set of launcher edits. In New session, type tl:name to apply one; it changes only the \
         fields it sets. Templates stack, the later one winning: tl:my-claude then tl:webbuilder starts Claude in a \
         managed checkout of acme/web on build-box. ⚡ marks the template {} launches directly; a number key launches \
         its template with {}. A number key can hold a partial template; press {} and then several keys to combine \
         them, Enter to launch.",
        super::QUICK_LAUNCH_SHORTCUT_NAME,
        super::number_key_name("number"),
        super::SEQUENCE_PREFIX_NAME,
    )
}

/// The Templates dialog. See the module docs.
pub struct TemplatesDialog {
    /// The dialog's layer, once opened, for its Close.
    layer: Option<LayerId>,
    /// The line saying why a shortcut opened the dialog, if one did.
    hint: Option<Hint>,
    /// Each row's number-key button, by template name, as last drawn: what its menu is anchored to when the button is
    /// clicked. A template gone from the list leaves its entry behind, which holds only a weak reference and is
    /// harmless.
    key_anchors: HashMap<String, Anchor>,
    /// The number-key menu that is open, if one is: its template's name and its layer.
    key_menu: Option<(String, LayerId)>,
    /// What this dialog last wrote to the preferences, so a refusal on that key is worded as the write it answers. A
    /// refusal there that some other dialog's write left (a Settings switch) is not this dialog's to show.
    last_write: Option<Write>,
}

/// A preference write this dialog sends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Write {
    Mark,
    Keys,
}

impl Write {
    /// The line a refusal of this write is shown with, before the helm's words.
    fn refused(self) -> &'static str {
        match self {
            Write::Mark => "The quick-launch mark was not saved: ",
            Write::Keys => "The number key was not saved: ",
        }
    }
}

/// Open the Templates dialog, with `hint`'s line when a shortcut opened it rather than the Templates button. `mark` is
/// the [`Mark`] the opener took when the user acted; nothing opens when the user has moved on since, or while the
/// window is inert (it has no store to draw templates from). Closing returns focus to what held it when the dialog
/// opened.
///
/// Returns the dialog's layer, or `None` when nothing opened.
pub fn open(hint: Option<Hint>, mark: &Mark, window: &mut Window, cx: &mut App) -> Option<LayerId> {
    session::current_store(cx)?;
    if focus::has_moved_on(mark, cx) {
        return None;
    }
    let dialog = cx.new(|_| TemplatesDialog {
        layer: None,
        hint,
        key_anchors: HashMap::new(),
        key_menu: None,
        last_write: None,
    });
    let container = cx.focus_handle();
    let layer = layers::open_dialog(
        Dialog {
            view: dialog.clone().into(),
            container: container.clone(),
            // Focus starts on the dialog itself until the editor's "+ New template" exists, where the old dialog
            // started (`DialogFocus::remembered(".templates-new", ..)`).
            start: container,
            subject: None,
            // The dialog has no one main action: Enter on the dialog itself does nothing, as in the old app.
            main_action: None,
            keeps_focus_on_empty_click: false,
            returns_to: ReturnTo::Opener,
            // Stay while this dialog's preference write is in flight, as the old dialog held its Close while busy: closing
            // then would lose the line that reports the write if it is refused (SPEC.md, "One GUI at a time": a failed
            // save is reported). The editor's leave question joins this hook with its own piece.
            on_dismiss: Some(Rc::new({
                let dialog = dialog.downgrade();
                move |_: &mut Window, cx: &mut App| {
                    let writing = dialog
                        .upgrade()
                        .is_some_and(|dialog| dialog.read(cx).writing(cx));
                    if writing {
                        Dismissal::Stay
                    } else {
                        Dismissal::Close
                    }
                }
            })),
            placement: layers::DialogPlacement::Centered,
            dismiss_on_outside_press: false,
        },
        mark,
        window,
        cx,
    )?;
    dialog.update(cx, |dialog, _| dialog.layer = Some(layer));
    Some(layer)
}

/// What the header's Close does: dismiss the dialog through the layer manager, so its dismissal hook (once the editor
/// has one) decides, exactly as Escape does.
fn dismiss(
    dialog: WeakEntity<TemplatesDialog>,
) -> impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static {
    move |_, window, cx| {
        let layer = dialog.upgrade().and_then(|dialog| dialog.read(cx).layer);
        if let Some(layer) = layer {
            layers::dismiss(layer, window, cx);
        }
    }
}

/// A template's summary on one line (`.templates-row-summary`: `white-space: nowrap`): the app's words as text, the
/// template's values through the host-text element. The parts never wrap; a summary longer than the row is cut at the
/// row's edge, without the old ellipsis, since an ellipsis can only end one text element and a summary is several
/// (recorded in the parity file).
fn summary_line(template: &LaunchTemplate) -> gpui::Div {
    div()
        .flex()
        .flex_row()
        .flex_nowrap()
        .overflow_hidden()
        .whitespace_nowrap()
        .children(
            summary::parts(&template.fields)
                .into_iter()
                .map(|part| -> AnyElement {
                    match part {
                        Part::Words(words) => div().flex_none().child(words).into_any_element(),
                        Part::Host(value) => host_text::element(&HostText::from_host(value))
                            .flex_none()
                            .into_any_element(),
                        Part::Mono(value) => host_text::element(&HostText::from_host(value))
                            .flex_none()
                            .font_family(crate::assets::TERMINAL_FONT)
                            .into_any_element(),
                    }
                }),
        )
}

/// `.templates-quick-mark`: the bolt button's column width.
const BOLT_WIDTH: f32 = 28.;
/// `.templates-quick-chord`: the marked row's chord line.
const CHORD_TEXT: f32 = 11.;
/// `.templates-number-key`: the key button's text size.
const KEY_TEXT: f32 = 10.;
/// The number-key menu's width, as wide as the old flyout drew its longest description.
const KEY_MENU_WIDTH: f32 = 260.;

impl TemplatesDialog {
    /// Whether a preference write this dialog sent is in flight.
    fn writing(&self, cx: &App) -> bool {
        self.last_write.is_some()
            && session::current_operations(cx).is_some_and(|operations| {
                operations
                    .read(cx)
                    .in_flight(&OpKey::preferences())
                    .is_some()
            })
    }

    /// One row of the list (`li` of `.templates-list`): the bolt, the number-key button, and the template's name over
    /// its summary, with the ⚡ chord as a third line on the marked row.
    fn row(
        &mut self,
        template: &LaunchTemplate,
        bindings: &Bindings,
        busy: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = theme::palette(cx);
        let name = HostText::from_host(template.name.clone());
        let marked = bindings.is_marked(&template.name);
        let eligible = shortcuts::may_mark(template);
        let bound = bindings.key_of(&template.name);
        let weak = cx.weak_entity();

        let bolt_name = template.name.clone();
        let bolt = IconButton::new(
            format!("template-bolt-{}", template.name),
            if marked { Icon::BoltFilled } else { Icon::Bolt },
            HostSentence::new("template-bolt-label")
                .text("Quick launch ")
                .host(&name)
                .accessible_name(),
            HoverHelp::tooltip(shortcuts::bolt_help(marked, eligible)),
            Role::Button,
        )
        .tier(Tier::QuickMark { lit: marked })
        .pressed(marked)
        // A lit bolt stays clickable so a template that no longer qualifies can still be unmarked (SPEC.md).
        .disabled(!eligible && !marked)
        // Busy rather than disabled while a write is in flight: a disabled button gives up focus, which would fall to
        // the dialog itself (Close is held too), losing the user's place in the list.
        .busy(busy)
        .on_click({
            let weak = weak.clone();
            // What the click means is decided by what the user saw: this frame's bolt, lit or not.
            move |_, _, cx| {
                let _ = weak.update(cx, |dialog, cx| dialog.toggle_mark(&bolt_name, marked, cx));
            }
        });

        // The anchor's element state lives only while it is asked for in every frame, so it is built afresh on each
        // draw and the latest kept for the menu to open on.
        let anchor = Anchor::for_element(format!("template-anchor-{}", template.name), window, cx);
        self.key_anchors
            .insert(template.name.clone(), anchor.clone());
        let key_name = template.name.clone();
        let key = Button::new(
            format!("template-key-{}", template.name),
            // The old button's name: which template's key it is, so rows read apart (its hover help says the key).
            HostSentence::new("template-key-label")
                .text("Number shortcut ")
                .host(&name)
                .accessible_name(),
            HoverHelp::tooltip(shortcuts::key_help(eligible, bound)),
            Role::Button,
        )
        .tier(Tier::NumberKey {
            bound: bound.is_some(),
        })
        .content(
            // `.templates-number-key`: 10-pixel monospace digits.
            div()
                .font_family(crate::assets::TERMINAL_FONT)
                .text_size(px(KEY_TEXT))
                .child(shortcuts::key_button_text(bound)),
        )
        // Open as the layer manager says: Escape or a press outside closes the menu without telling this dialog.
        .expanded(self.key_menu.as_ref().is_some_and(|(open, layer)| {
            *open == template.name && layers::is_open(*layer, window, cx)
        }))
        .busy(busy)
        .on_click(move |_, window, cx| {
            let _ = weak.update(cx, |dialog, cx| {
                dialog.toggle_key_menu(&key_name, window, cx)
            });
        });

        let mut lines = div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap(px(ROW_LINE_GAP))
            .py(px(ROW_PAD.0))
            .px(px(ROW_PAD.1))
            .child(
                div()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_size(metrics::TEXT_MD)
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme::color(palette.fg_0))
                    .child(host_text::element(&name).truncate()),
            )
            .child(
                div()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_size(metrics::TEXT_SM)
                    .line_height(px(SMALL_LINE_HEIGHT))
                    .text_color(theme::color(palette.fg_1))
                    .child(summary_line(template)),
            );
        if marked {
            lines = lines.child(
                div()
                    .text_size(px(CHORD_TEXT))
                    .font_family(crate::assets::TERMINAL_FONT)
                    .text_color(theme::color(palette.fg_2))
                    .child(super::QUICK_LAUNCH_SHORTCUT_NAME),
            );
        }

        div()
            .id(gpui::ElementId::Name(
                format!("template-row-{}", template.name).into(),
            ))
            .role(Role::ListItem)
            .aria_label(
                HostSentence::new("template-row-name")
                    .host(&name)
                    .accessible_name(),
            )
            .flex()
            .items_center()
            .gap(px(4.))
            // The bolt is anchored to the first line, so it does not move when the marked row gains its chord line.
            .child(
                div()
                    .flex_none()
                    .w(px(BOLT_WIDTH))
                    .self_stretch()
                    .child(bolt),
            )
            .child(
                div()
                    .flex_none()
                    .relative()
                    .child(key)
                    .child(anchor.track()),
            )
            .child(lines)
            .into_any_element()
    }

    /// The bolt of `name` was clicked while it was drawn lit (`was_lit`) or not: clear the mark, or give it to `name`
    /// ([`shortcuts::mark_write`] says why the drawn state decides). Nothing is written while another preference write
    /// is in flight (the operation store's one-per-key rule).
    fn toggle_mark(&mut self, name: &str, was_lit: bool, cx: &mut Context<Self>) {
        let stored = Bindings::current(cx).marked;
        if let Some(wanted) = shortcuts::mark_write(name, was_lit, stored.as_deref()) {
            self.write(Write::Mark, shortcuts::mark_command(wanted), cx);
        }
    }

    /// Give `name` the number key `key`, or none, from the map the store holds now, written whole.
    fn choose_key(&mut self, name: &str, key: Option<&str>, cx: &mut Context<Self>) {
        let bindings = Bindings::current(cx);
        let keys = bindings.keys_with(name, key);
        if keys == bindings.keys {
            return;
        }
        self.write(Write::Keys, shortcuts::keys_command(&keys), cx);
    }

    /// Send a preference write as work on the preferences' key.
    fn write(
        &mut self,
        write: Write,
        command: Result<Command, farhelm_helm_client::PathError>,
        cx: &mut Context<Self>,
    ) {
        let (Some(operations), Ok(command)) = (session::current_operations(cx), command) else {
            return;
        };
        let started = operations.update(cx, |operations, cx| {
            operations.start(&OpKey::preferences(), command, cx, |_, _| {})
        });
        match started {
            Ok(()) => {
                self.last_write = Some(write);
                cx.notify();
            }
            Err(not_started) => {
                tracing::debug!(?write, ?not_started, "template shortcut not saved")
            }
        }
    }

    /// The key button of `name` was clicked: open its menu, or close it if it is open. Focus starts on the key the
    /// template holds, or on No shortcut.
    fn toggle_key_menu(&mut self, name: &str, window: &mut Window, cx: &mut Context<Self>) {
        // A menu still open closes first; a second click on its own button only closes it.
        if let Some((open, layer)) = self.key_menu.take()
            && layers::is_open(layer, window, cx)
        {
            layers::close(layer, window, cx);
            if open == name {
                cx.notify();
                return;
            }
        }
        let Some(anchor) = self.key_anchors.get(name).cloned() else {
            return;
        };
        let mark = Mark::take(cx);
        let bound = Bindings::current(cx).key_of(name);
        let focus: Vec<FocusHandle> = (0..=KEYS.len())
            .map(|_| cx.focus_handle().tab_stop(true))
            .collect();
        let start = match bound {
            Some(key) => KEYS
                .iter()
                .position(|candidate| *candidate == key)
                .map_or(0, |index| index + 1),
            None => 0,
        };
        let dialog = cx.weak_entity();
        let view = cx.new(|_| KeyMenu {
            name: name.to_owned(),
            dialog,
            focus: focus.clone(),
        });
        let opened = layers::open_popover(
            Popover {
                view: view.into(),
                container: cx.focus_handle(),
                start: Some(focus[start].clone()),
                anchor,
            },
            &mark,
            window,
            cx,
        );
        self.key_menu = opened.map(|layer| (name.to_owned(), layer));
        cx.notify();
    }
}

/// A template's number-key menu (the old `NumberShortcutMenu`): No shortcut, then 1 to 9 and 0, each saying which
/// template holds it. Choosing one closes the menu and writes the keys.
struct KeyMenu {
    /// The template the menu is for.
    name: String,
    dialog: WeakEntity<TemplatesDialog>,
    /// The items' focus handles, No shortcut first, so the menu can start on the key the template holds.
    focus: Vec<FocusHandle>,
}

impl KeyMenu {
    /// What choosing `key` (or No shortcut) does: close this menu, then write.
    fn choose(
        &self,
        key: Option<&'static str>,
    ) -> impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static {
        let (dialog, name) = (self.dialog.clone(), self.name.clone());
        move |_, window, cx| {
            let _ = dialog.update(cx, |dialog, cx| {
                if let Some((_, layer)) = dialog.key_menu.take() {
                    layers::close(layer, window, cx);
                }
                dialog.choose_key(&name, key, cx);
            });
        }
    }
}

impl Render for KeyMenu {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::palette(cx);
        let bindings = Bindings::current(cx);
        let name = HostText::from_host(self.name.clone());
        let header = div()
            .px(px(10.))
            .py(px(6.))
            .flex()
            .flex_col()
            .child(
                div()
                    .text_size(metrics::TEXT_MD)
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme::color(palette.fg_0))
                    .min_w_0()
                    .child(host_text::element(&name).truncate()),
            )
            .child(
                div()
                    .text_size(metrics::TEXT_SM)
                    .text_color(theme::color(palette.fg_1))
                    .child("Number shortcut"),
            );
        let none = MenuItem::new(
            "templates-key-none",
            Icon::Templates,
            "No shortcut",
            HoverHelp::tooltip("Remove this template's number shortcut"),
            Role::MenuItem,
        )
        .description("Remove this template's number shortcut")
        .focus(&self.focus[0])
        .on_click(self.choose(None));
        let keys = KEYS.iter().enumerate().map(|(index, &key)| {
            let item = MenuItem::new(
                format!("templates-key-{key}-item"),
                Icon::Templates,
                super::number_key_name(key),
                HoverHelp::tooltip(format!(
                    "Give this template {}",
                    super::number_key_name(key)
                )),
                Role::MenuItem,
            )
            .focus(&self.focus[index + 1])
            .on_click(self.choose(Some(key)));
            match bindings.keys.get(key) {
                Some(holder) if *holder == self.name => item.description("This template"),
                Some(holder) => item.description_sentence(
                    HostSentence::new(format!("templates-key-{key}-holder"))
                        .host(&HostText::from_host(holder.clone()))
                        .text(" (moves here)"),
                ),
                None => item.description("No template uses this key"),
            }
        });
        let label = HostSentence::new("templates-key-menu-label")
            .text("Number key for ")
            .host(&name)
            .accessible_name();
        layers::panel(cx)
            .id("templates-key-menu")
            .debug_selector(|| "templates-key-menu".into())
            .role(Role::Menu)
            .aria_label(label)
            .w(px(KEY_MENU_WIDTH))
            .child(header)
            .child(none)
            .children(keys)
    }
}

/// The list's count line under the rows (`.templates-count`): "1 template", "3 templates".
pub(crate) fn count_text(count: usize) -> String {
    if count == 1 {
        "1 template".to_owned()
    } else {
        format!("{count} templates")
    }
}

impl Render for TemplatesDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::palette(cx);
        let bindings = Bindings::current(cx);
        // One preference write at a time: while one is in flight every bolt and key button waits, as the old list's
        // controls did while it was busy.
        let (busy, refused) = match session::current_operations(cx) {
            Some(operations) => {
                let operations = operations.read(cx);
                let refused = self
                    .last_write
                    .zip(operations.failure(&OpKey::preferences()))
                    .map(|(write, failure)| {
                        HostSentence::new("templates-write-refused")
                            .text(write.refused())
                            .host(&failure.message)
                    });
                (
                    operations.in_flight(&OpKey::preferences()).is_some(),
                    refused,
                )
            }
            None => (false, None),
        };
        let store = session::current_store(cx);
        let (templates, unavailable) = match &store {
            Some(store) => {
                let collections = store.read(cx).collections();
                let templates: Vec<LaunchTemplate> = collections
                    .ordered::<LaunchTemplate>()
                    .into_iter()
                    .cloned()
                    .collect();
                let unavailable = match collections
                    .meta(Collection::Templates)
                    .map(|meta| &meta.availability)
                {
                    Some(Availability::Unavailable { error }) => Some(error.clone()),
                    Some(Availability::Available) | None => None,
                };
                (templates, unavailable)
            }
            None => (Vec::new(), None),
        };

        let small = |text: String| {
            div()
                .text_size(metrics::TEXT_SM)
                .line_height(px(SMALL_LINE_HEIGHT))
                .text_color(theme::color(palette.fg_1))
                .child(text)
        };

        // `aside.templates-sidebar`: the rows, then the count pushed to the column's foot.
        let list = div()
            .id("templates-list")
            .role(Role::List)
            .aria_label("Template list")
            .flex()
            .flex_col()
            .gap(px(ROW_GAP))
            .children(
                templates
                    .iter()
                    .map(|template| self.row(template, &bindings, busy, window, cx))
                    .collect::<Vec<_>>(),
            );
        let sidebar = div()
            .flex_none()
            .w(px(LIST_WIDTH))
            .flex()
            .flex_col()
            .gap(px(LIST_COLUMN_GAP))
            .pr(px(LIST_COLUMN_PAD_RIGHT))
            .border_r_1()
            .border_color(theme::color(palette.border_dialog))
            .child(list)
            .child(
                div()
                    .mt_auto()
                    .debug_selector(|| "templates-count".into())
                    .child(small(count_text(templates.len()))),
            );
        // `section.templates-editor`, until the editor exists: an empty pane where it will be.
        let editor = div()
            .id("templates-editor")
            .role(Role::Region)
            .aria_label("Template editor")
            .flex_1()
            .min_w_0();
        let panes = div()
            .flex()
            .gap(px(PANES_GAP))
            .min_h(px(PANES_MIN_HEIGHT))
            .child(sidebar)
            .child(editor);

        let mut body = div()
            .flex()
            .flex_col()
            .gap(px(CONTENT_GAP))
            .child(small(help_text()));
        if let Some(hint) = self.hint {
            body = body.child(
                div()
                    .id("templates-hint")
                    .debug_selector(|| "templates-hint".into())
                    .role(Role::Status)
                    .child(small(hint.text())),
            );
        }
        body = body.child(panes);
        if let Some(refused) = refused {
            // A refused mark or key: the helm's words, under the panes, until the next preference write.
            body = body.child(
                div()
                    .debug_selector(|| "templates-write-refused".into())
                    .text_size(metrics::TEXT_SM)
                    .text_color(theme::color(palette.warn))
                    .child(refused),
            );
        }
        if let Some(error) = unavailable {
            // `.templates-error`: the helm's own words, in the warning color, below both panes.
            body = body.child(
                div()
                    .debug_selector(|| "templates-unavailable".into())
                    .text_size(metrics::TEXT_SM)
                    .text_color(theme::color(palette.warn))
                    .child(
                        HostSentence::new("templates-unavailable")
                            .text("Templates unavailable: ")
                            .host(&HostText::from_host(error)),
                    ),
            );
        }

        let writing = self.writing(cx);
        let weak = cx.weak_entity();
        DialogFrame::new("templates", "Templates", body, Vec::new())
            .summary(SUMMARY)
            .tile(
                svg()
                    .path(Icon::Templates.path())
                    .size(px(TILE_GLYPH))
                    .text_color(theme::color(palette.fg_0)),
            )
            .width(px(WIDTH))
            .close(
                CLOSE_LABEL,
                HoverHelp::tooltip(CLOSE_HELP),
                // Held while a write is in flight, as the dismissal hook holds Escape (the old dialog disabled it).
                writing,
                dismiss(weak),
            )
    }
}
