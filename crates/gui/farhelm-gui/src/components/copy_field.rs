//! The copy field: a host value drawn as itself, which copies its raw bytes to the clipboard when clicked and says
//! "✓ Copied" for a moment afterwards. The old details bar's directory and command line (`.header-copy`,
//! `session_view.rs` and `app.css` at reference commit `24bc0580`).
//!
//! SPEC.md, "Terminal experience": "Directory and command line are muted click-to-copy buttons that take the width
//! their values need and ellipsize before the session name or host when the bar runs out of room; a click confirms
//! locally for about 1.5 seconds." Here's the TLDR of how it behaves:
//!
//! - **It looks like its value.** No fill and no edge, the quiet ink, the value in the monospace face, cut with an
//!   ellipsis on one line once its row runs out of room. It shrinks three times as fast as an ordinary item beside it
//!   (`flex: 0 3 auto`, the old field's). That alone does not keep a row's other parts whole: flex shrinking is
//!   weighted by each item's width, so beside a field a long title still gives up some of its own. A row that must
//!   give up the fields' width first (SPEC.md's details bar) puts them in a box of their own that gets only the room
//!   the rest leaves (`session_pane::details`); the factor then only splits the cut between the fields. Under the
//!   pointer (as GPUI's hover reads it: not after a key press, and not under a layer that covers the field) or keyboard
//!   focus it takes the control hover fill and the primary ink, and shows a clipboard glyph before the value.
//! - **An empty value still shows something.** The owner may give the field app wording to draw in place of a value
//!   that would show nothing (the old `(empty)` and `(whitespace only: N characters)`, [`CopyField::placeholder`]), so
//!   the field is never an unnamed sliver of a button. It still copies the raw bytes.
//! - **What it shows is not what it copies.** The caller hands it the value to draw (the directory with its home
//!   folded to `~`, say) and, separately, the raw bytes to copy. A copy that differed from the recorded value would be
//!   useless, and a shown value that kept every byte could be unreadable; so both are kept.
//! - **The confirmation is per click.** A click copies, then shows "✓ Copied" in place of the glyph and value for
//!   [`CONFIRMATION`], in the bright ink, and is named "✓ Copied" meanwhile. A second click starts the time over, and an
//!   earlier click never cuts a later one's confirmation short (the old app's plain flag let the first of two quick
//!   clicks clear the flag the second had just set): each click's deadline goes with its confirmation, so a replaced
//!   one is cancelled. The confirmation says the user clicked, not that the platform took the text; clipboard failures
//!   are silent ([`super::copy_button`]'s `write`).
//! - **The owner holds the confirmation.** It lives in a [`CopyFeedback`] the owner makes and keeps, not in the
//!   field's element state, for two reasons. The owner decides how long a confirmation belongs to what it describes
//!   (the details bar drops it with the open session). And the owner may need to say more about a click than the field
//!   can fit: the details bar warns, outside the field, when the bytes held a hidden character
//!   ([`CopyFeedback::copied_hidden`]), because a field that shrinks and ellipsizes could clip that warning, and the
//!   warning is the one thing such a copy must not leave unsaid.
//!
//! It is a push button wherever it sits, so the constructor takes no role, as [`super::copy_button`]'s does not. It is a
//! tab stop like any button, under the focus coordinator's click rule ([`focus::track`]).

use std::rc::Rc;
use std::time::Duration;

use gpui::{
    App, Context, ElementId, Entity, InteractiveElement, IntoElement, ParentElement, RenderOnce,
    Role, SharedString, StatefulInteractiveElement, Styled, Window, div,
    prelude::FluentBuilder as _, px, svg,
};

use super::{Handler, HoverHelp};
use crate::assets::Icon;
use crate::host_text::{self, HostText};
use crate::theme;
use crate::{assets, focus, ticker};

/// How long a copy field confirms a click: the old bar's `sleep_ms(1500)`.
pub const CONFIRMATION: Duration = Duration::from_millis(1500);

/// What a copy field shows, and is named, while it confirms a click.
pub const COPIED: &str = "✓ Copied";

/// The field's text size: a browser's default for a button that sets none (13.333 pixels), which `.header-copy` did
/// not override.
const TEXT_SIZE: f32 = 13.333;
/// The text line: the browser's `line-height: normal` for that size in the interface face, rounded.
const LINE_HEIGHT: f32 = 16.;
/// `.header-copy`'s padding, vertical then horizontal.
const PAD: (f32, f32) = (2., 4.);
/// `.header-copy`'s `flex-shrink`: three times an ordinary item's.
const SHRINK: f32 = 3.;
/// The clipboard glyph's side (the old emoji was about one text line wide) and its `margin-right`.
const GLYPH: f32 = 13.;
const GLYPH_GAP: f32 = 4.;

/// One copy field's confirmation state, held by the field's owner in an entity and handed to the field each frame. Its
/// second user is Settings → Tensorlake's registration command, whose Copy ([`super::copy_button_then`]) confirms
/// through one of these and draws "Command copied" while it lasts.
///
/// The field reads it while drawing, so a change notifies the window that drew it, and the deadline that ends a
/// confirmation needs no view to hold anything. Dropping it drops a pending deadline with it.
#[derive(Default)]
pub struct CopyFeedback {
    /// The confirmation showing now, if any.
    confirming: Option<Confirming>,
}

/// The click a field is confirming: what its bytes held, and the deadline that ends it. Replacing it drops the
/// deadline, which cancels it, so a deadline only ever ends the confirmation it was started for.
struct Confirming {
    hidden: bool,
    _deadline: ticker::Deadline,
}

impl CopyFeedback {
    /// Whether the field is confirming a click now.
    pub fn confirming(&self) -> bool {
        self.confirming.is_some()
    }

    /// Whether the field is confirming a click whose copied bytes held a presentation-unsafe character, one the screen
    /// shows escaped (`<U+202E>`, a line break), so the clipboard does not hold what the value reads like. Decided from
    /// the bytes that click copied: a value that changes while the confirmation shows does not change the answer.
    pub fn copied_hidden(&self) -> bool {
        self.confirming
            .as_ref()
            .is_some_and(|confirming| confirming.hidden)
    }

    /// Record a click that copied `copied`: confirm it for [`CONFIRMATION`] from now, replacing any confirmation
    /// already showing. What a field's click does after writing the clipboard, and how a screenshot scene shows a
    /// confirmation without a click.
    pub(crate) fn confirm(&mut self, copied: &str, cx: &mut Context<Self>) {
        let feedback = cx.weak_entity();
        let deadline = ticker::after(CONFIRMATION, cx, move |cx| {
            let _ = feedback.update(cx, |feedback, cx| {
                feedback.confirming = None;
                cx.notify();
            });
        });
        // The earlier click's confirmation goes here, and its deadline with it, cancelled before it could fire.
        self.confirming = Some(Confirming {
            hidden: copied
                .chars()
                .any(farhelm_proto::text::is_presentation_unsafe),
            _deadline: deadline,
        });
        cx.notify();
    }
}

/// A host value that copies itself. See the module docs.
#[derive(IntoElement)]
pub struct CopyField {
    id: SharedString,
    label: SharedString,
    help: HoverHelp,
    value: HostText,
    copied: SharedString,
    feedback: Entity<CopyFeedback>,
    /// App wording drawn in place of `value` ([`CopyField::placeholder`]).
    placeholder: Option<SharedString>,
}

impl CopyField {
    /// A copy field. `id` must be stable and unique among its siblings (it keys the field's focus handle and tooltip
    /// owner, and tests and the bridge find it by it); `label` is its accessible name at rest, the shown value's
    /// accessible spelling (a [`crate::host_text::HostSentence`]'s name), since "✓ Copied" replaces it while a click is
    /// confirmed; `help` is its hover help and accessible description; `value` is what it draws; `copied` is the bytes
    /// a click puts on the clipboard; `feedback` is the owner's record of its confirmation.
    pub fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        help: HoverHelp,
        value: &HostText,
        copied: impl Into<SharedString>,
        feedback: &Entity<CopyFeedback>,
    ) -> Self {
        CopyField {
            id: id.into(),
            label: label.into(),
            help,
            value: value.clone(),
            copied: copied.into(),
            feedback: feedback.clone(),
            placeholder: None,
        }
    }

    /// Draw `words`, the app's own, in place of the value, for a value that would show nothing or only blanks (the old
    /// `display_peer`'s `(empty)`). `None` draws the value. The field is then named by the same words in place of its
    /// label, so what it shows and what it is called cannot disagree, and it is never named with nothing. It still
    /// copies its bytes and keeps its help.
    pub fn placeholder(mut self, words: Option<SharedString>) -> Self {
        self.placeholder = words;
        self
    }
}

impl RenderOnce for CopyField {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let palette = theme::palette(cx);
        let confirming = self.feedback.read(cx).confirming();
        let focus = focus::control_handle(
            ElementId::Name(format!("{}-focus", self.id).into()),
            window,
            cx,
        );
        let ring = focus::shows_ring(&focus, window);
        // `.header-copy.copied` comes after `:hover` and `:focus-visible` in the old sheet, so its bright ink wins over
        // theirs.
        let ink = if confirming {
            palette.fg_bright
        } else if ring {
            palette.fg_0
        } else {
            palette.fg_1
        };
        let (hover_fill, hover_ink) = (palette.control_hover, palette.fg_0);
        // Whether the pointer is over the field, as GPUI's own hover reads it (not after a key press, not under a
        // layer that covers the field, and checked again as each frame paints, so a field that moves out from under a
        // resting pointer lets go). Kept in the element's state, so a field not drawn for a frame starts unhovered.
        // The field's one hover listener is its hover help's, so a child over the whole field listens instead.
        let hovered = window.use_keyed_state(
            ElementId::Name(format!("{}-hovered", self.id).into()),
            cx,
            |_, _| false,
        );
        let pointer_over = *hovered.read(cx);

        let content = if confirming {
            div().child(COPIED)
        } else {
            // The glyph is drawn only under the pointer or keyboard focus (`display: none` otherwise), so at rest the
            // value starts at the field's padding, as the old one did. It is decided here from the hover state above,
            // not with GPUI's group hover: a group hover that changes layout breaks GPUI's frame, since it reads the
            // hover from the element's state while laying out and from the group's hitbox while painting, and in the
            // frame the pointer arrives a glyph laid out as absent is asked to paint (`must call prepaint before
            // paint`). It shows only in the two states that draw the primary ink, so it takes that ink (an SVG does
            // not inherit its parent's).
            let glyph = (ring || pointer_over).then(|| {
                div().flex_none().mr(px(GLYPH_GAP)).child(
                    svg()
                        .path(Icon::Clipboard.path())
                        .size(px(GLYPH))
                        .text_color(theme::color(hover_ink)),
                )
            });
            div()
                .flex()
                .flex_row()
                .items_center()
                .min_w_0()
                .children(glyph)
                .child(
                    match &self.placeholder {
                        Some(words) => div().child(words.clone()),
                        None => host_text::element(&self.value),
                    }
                    .min_w_0()
                    .truncate()
                    .font_family(assets::TERMINAL_FONT),
                )
        };

        let copied = self.copied.clone();
        let feedback = self.feedback.clone();
        let handler: Handler = Rc::new(move |_, _, cx| {
            super::copy_button::write(&copied, cx);
            feedback.update(cx, |feedback, cx| feedback.confirm(&copied, cx));
        });

        let field = div()
            .id(ElementId::Name(self.id.clone()))
            .relative()
            // Tests find a control by its id; this records nothing outside test builds.
            .debug_selector(|| self.id.to_string())
            .role(Role::Button)
            .aria_label(if confirming {
                SharedString::from(COPIED)
            } else {
                self.placeholder
                    .clone()
                    .unwrap_or_else(|| self.label.clone())
            })
            .flex()
            .flex_row()
            .items_center()
            // Content-sized and never grown, so each field takes the width its value needs; shrinking below that
            // needs `min-width: 0`, as in the old sheet.
            .min_w_0()
            .flex_shrink(SHRINK)
            .py(px(PAD.0))
            .px(px(PAD.1))
            .whitespace_nowrap()
            .text_size(px(TEXT_SIZE))
            .line_height(px(LINE_HEIGHT))
            .text_color(theme::color(ink))
            .when(ring, |field| field.bg(theme::color(hover_fill)))
            .hover(move |style| {
                let style = style.bg(theme::color(hover_fill));
                if confirming {
                    style
                } else {
                    style.text_color(theme::color(hover_ink))
                }
            })
            // The old global `:focus-visible` ring: two pixels of accent, one pixel clear of the field's edge, with
            // square corners, since `.header-copy` had none rounded. Drawn as an element because GPUI has no outline;
            // the field does not clip, so the ring outside its box shows.
            .when(ring, |field| {
                field.child(
                    div()
                        .absolute()
                        .top(px(-3.))
                        .left(px(-3.))
                        .right(px(-3.))
                        .bottom(px(-3.))
                        .border_2()
                        .border_color(theme::color(palette.accent)),
                )
            })
            .child(content)
            .child(
                div()
                    .id(ElementId::Name(format!("{}-hover", self.id).into()))
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .on_hover(move |over, _, cx| {
                        hovered.update(cx, |hovered, cx| {
                            *hovered = *over;
                            cx.notify();
                        })
                    }),
            );
        let field =
            super::with_hover_help(field, &self.id, &self.help, true, Some(&focus), window, cx);
        let field = focus::track(field, &focus, focus::OnPress::Control);
        super::activate_on_click(
            super::activate_on_enter_or_space(field, Some(handler.clone())),
            Some(handler),
        )
    }
}
