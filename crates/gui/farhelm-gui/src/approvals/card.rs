//! The expanded approval card, drawn from the card model ([`super::model::Card`]).
//!
//! SPEC.md, "Agent-spawned sessions": the "Agent request" badge and the action as the heading, the waiting count, the
//! compact From line with Copy ID, one aligned list of labelled facts (labels stacking above values in a narrow pane),
//! single-line values that scroll sideways within themselves, URLs and permission flags wrapping in full, commands and
//! prompts in bounded boxes that keep all their text, the Permissions chips, the notes, and the answer bar with Deny
//! apart at the right and the buttons wrapping in a narrow pane. Ported from the old app's `ApprovalCard`,
//! `Requester`, `CardDetail` and `PermissionChips` and their rules in `app.css` (`.approval-card` and the rules after
//! it, lines 8835-9147) at the reference commit.
//!
//! The card is drawn afresh from the store on every frame, the way every store-fed view is: the region builds a
//! [`Card`] from the pushed request each time it draws and hands it here, so nothing the card shows can go stale. What
//! the user did (an answer in flight, the answer bar's pause) arrives as [`CardState`]. Every value an agent wrote is
//! drawn through the host-text element, each in its own element beside the app's words, never formatted into them.
//!
//! The card does not decide what an answer does: each answer button calls the region's `on_answer`, which sends it.

use std::rc::Rc;

use farhelm_proto::approvals::ApprovalAnswer;
use gpui::{
    AnyElement, App, ElementId, FontWeight, InteractiveElement as _, IntoElement, ParentElement,
    Role, SharedString, StatefulInteractiveElement as _, Styled, Window, div,
    prelude::FluentBuilder as _, px, svg,
};

use super::model::{
    Answer, AnswerHelp, AnswerLabel, COPY, COPY_HELP, COPY_ID, Card, Chips, FROM_COPY_NAME,
    FROM_HOST, FROM_ID_LABEL, Fact, LeadNote, ON_HOST_ONLY, OWN_HOST_HELP, Requester, Shape,
    TRUSTED_WORKSPACE, count, sentence_case,
};
use crate::assets;
use crate::components::{Button, HoverHelp, ScrollBox, StatusMark, Tier, copy_button};
use crate::host_text::{self, HostSentence, HostText};
use crate::theme::{self, Palette, metrics};

/// The card's widest: `min(660px, pane - 24px)` in the old layout script. The region applies the pane half.
pub const MAX_WIDTH: gpui::Pixels = px(660.);

/// At or below this card width labels stack above their values and the From line wraps (the old `@container
/// (max-width: 420px)` rule).
pub const NARROW_WIDTH: gpui::Pixels = px(420.);

/// What the region knows that the request itself does not: where the card sits (its width and the waiting count) and
/// what the user did (an answer in flight or paused, and the last answer's refusal).
pub struct CardState {
    /// The card's width, for the narrow layout.
    pub width: gpui::Pixels,
    /// The fleet's waiting count, the collapsed requests included.
    pub waiting: usize,
    /// The answer bar is paused, or an answer to this request is in flight: every answer button is busy.
    pub busy: bool,
    /// The last answer's refusal, which leaves the request waiting.
    pub refusal: Option<HostText>,
}

/// What an answer button does when pressed: the region sends the answer.
pub type OnAnswer = Rc<dyn Fn(ApprovalAnswer, &mut Window, &mut App)>;

/// The old `.approval-card-row`'s label column.
const LABEL_WIDTH: gpui::Pixels = px(112.);

/// The card's text size: the old page's default, which `.approval-card` inherited.
const BODY_TEXT: gpui::Pixels = px(16.);

/// The card's line height: the old page's `line-height: normal` for 16-pixel Inter, as the reference screens measure
/// it. GPUI's own default is about 1.6 em, which made every row of the card a quarter taller than the old one.
const BODY_LINE: gpui::Pixels = px(19.4);

/// The line height of the card's 12-pixel parts (the From line, the badge, the count, the chips and their
/// descriptions): `normal` for 12-pixel Inter, as the host list's details measure it.
const SMALL_LINE: gpui::Pixels = px(15.);

/// A box's label line: `.approval-card-block-row dt`'s `line-height: 1.4` at [`BODY_TEXT`].
const BOX_LABEL_LINE: gpui::Pixels = px(22.4);

/// A box's line height: `.approval-card-block`'s `line-height: 1.6` at [`BODY_TEXT`].
const BOX_LINE: gpui::Pixels = px(25.6);

/// The expanded card for `card`.
pub fn card(card: &Card, state: &CardState, on_answer: OnAnswer, cx: &App) -> AnyElement {
    let palette = theme::palette(cx);
    let narrow = state.width <= NARROW_WIDTH;
    let id = |part: &str| SharedString::from(format!("approval-{}-{part}", card.id));
    let facts = div().flex().flex_col().flex_none().gap(px(5.)).children(
        card.facts
            .iter()
            .enumerate()
            .map(|(index, fact)| row(card, index, fact, narrow, palette)),
    );
    let boxes = card
        .boxes
        .iter()
        .enumerate()
        .map(|(index, fact)| long_text(card, index, fact, palette));
    let lead = card.lead_note.as_ref().map(|note| lead_note(card, note));
    // The old card announced the refusal (`role="alert"`): it arrives after the user pressed, away from the button.
    let refusal = state.refusal.as_ref().map(|reason| {
        div()
            .id(ElementId::Name(id("refusal")))
            .role(Role::Alert)
            .flex_none()
            .text_color(theme::color(palette.danger))
            .child(host_text::element(reason))
    });
    let notes = card.notes.iter().map(|words| note(words));
    let body = div()
        .id(ElementId::Name(id("body")))
        .flex()
        .flex_col()
        .flex_shrink_1()
        .min_h_0()
        .gap(px(8.))
        .overflow_y_scroll()
        .child(facts)
        .children(boxes)
        .children(lead)
        .children(refusal)
        .children(notes);
    div()
        .id(ElementId::Name(id("card")))
        .role(Role::Group)
        .aria_label(SharedString::from(card.heading))
        .flex()
        .flex_col()
        .flex_shrink_1()
        .min_h_0()
        .w(state.width)
        .max_w(MAX_WIDTH)
        .gap(px(8.))
        .pt(px(12.))
        .px(px(12.))
        .overflow_hidden()
        .bg(theme::color(palette.bg_2))
        .border_2()
        .border_color(theme::color(palette.warn))
        .rounded(metrics::RADIUS_MD)
        .shadow(crate::layers::menu_shadow(palette))
        // The page's default size (16 pixels), which the old card inherited; only the From line, the badge, the count
        // and the chips are smaller.
        .text_size(BODY_TEXT)
        .line_height(BODY_LINE)
        .text_color(theme::color(palette.fg_0))
        .child(header(card, state, palette))
        .child(from_line(card, &card.from, state.width, palette))
        .child(body)
        .child(answer_bar(card, state, on_answer, palette))
        .into_any_element()
}

// ---- The header and the From line ----------------------------------------------------------------------------------

/// The badge, the heading and the waiting count (`.approval-card-header`).
fn header(card: &Card, state: &CardState, palette: &Palette) -> impl IntoElement {
    div()
        .flex()
        .flex_none()
        .flex_wrap()
        .items_center()
        .gap(px(8.))
        .child(
            div()
                .px(px(6.))
                .py(px(2.))
                .border_1()
                .border_color(theme::color(palette.warn))
                .rounded(metrics::RADIUS_SM)
                .bg(theme::color(palette.warn_badge_bg))
                .text_color(theme::color(palette.warn))
                .text_size(metrics::TEXT_SM)
                .line_height(SMALL_LINE)
                .font_weight(FontWeight::BOLD)
                .child("Agent request"),
        )
        .child(
            // The old heading was an `h2`.
            div()
                .id(ElementId::Name(SharedString::from(format!(
                    "approval-{}-heading",
                    card.id
                ))))
                .role(Role::Heading)
                .aria_label(SharedString::from(card.heading))
                .min_w_0()
                .font_weight(FontWeight::SEMIBOLD)
                .child(card.heading),
        )
        .child(
            div()
                .ml_auto()
                .text_color(theme::color(palette.fg_dim))
                .text_size(metrics::TEXT_SM)
                .line_height(SMALL_LINE)
                .child(count(state.waiting)),
        )
}

/// The compact From line: the host, the quoted title and the shortened ID, each its own element with its full text on
/// hover, and Copy ID (`.approval-card-from`).
///
/// The parts are labelled values, not live statuses, so they carry [`Role::Label`] as the session pane's details do;
/// and they keep the card's full-contrast ink, as the old spans inherited it, rather than a status mark's quiet one.
fn from_line(
    card: &Card,
    from: &Requester,
    width: gpui::Pixels,
    palette: &Palette,
) -> impl IntoElement {
    let narrow = width <= NARROW_WIDTH;
    let id = |part: &str| SharedString::from(format!("approval-{}-from-{part}", card.id));
    let full = HostText::identity(from.id.as_str());
    let host = StatusMark::new(
        id("host"),
        "From host",
        HoverHelp::sentence(HostSentence::new(id("host-help")).host(&from.host)),
        Role::Label,
        "",
    )
    .ink(palette.fg_0)
    .content(
        div()
            .font_weight(FontWeight::BOLD)
            .child(host_text::element(&from.host)),
    )
    .truncate();
    let title = match &from.title {
        Some(title) => StatusMark::new(
            id("title"),
            "From session",
            HoverHelp::sentence(HostSentence::new(id("title-help")).host(title)),
            Role::Label,
            "",
        )
        .ink(palette.fg_0)
        .content(host_text::element(title))
        .truncate()
        .into_any_element(),
        // The old span was named "From session" too, so a screen reader hears what the words stand in for.
        None => div()
            .id(ElementId::Name(id("title")))
            .role(Role::Label)
            .aria_label("From session")
            .child("title unknown")
            .into_any_element(),
    };
    // Only the drawing is short: the accessible name and the hover carry the complete reference in the strict spelling.
    let name = HostSentence::new(id("id-name"))
        .text(format!("{FROM_ID_LABEL}: "))
        .mono(&full)
        .accessible_name();
    let short = StatusMark::new(
        id("id"),
        name,
        HoverHelp::sentence(HostSentence::new(id("id-help")).mono(&full)),
        Role::Label,
        "",
    )
    .ink(palette.fg_0)
    .content(host_text::element(&HostText::identity_abbreviated(
        from.id.as_str(),
    )))
    .monospace();
    div()
        .flex()
        .flex_none()
        .when(narrow, |line| line.flex_wrap())
        .items_center()
        .gap(px(6.))
        .min_w_0()
        .pb(px(8.))
        .border_b_1()
        .border_color(theme::color(palette.border_row))
        .text_size(metrics::TEXT_SM)
        .line_height(SMALL_LINE)
        .child("From")
        .child(
            div()
                .min_w_0()
                .max_w(gpui::relative(if narrow { 0.36 } else { 0.22 }))
                .child(host),
        )
        .child("·")
        .child(
            // In a narrow card the title starts at 42% of the line and the line wraps (the old narrow rule's
            // `flex-basis: 42%`), so the ID and Copy ID move under it rather than squeezing the title to a letter.
            div()
                .flex()
                .flex_1()
                .min_w_0()
                .when(narrow, |slot| slot.flex_basis(gpui::relative(0.42)))
                .when(from.title.is_some(), |slot| slot.child("“"))
                .child(div().min_w_0().child(title))
                .when(from.title.is_some(), |slot| slot.child("”")),
        )
        .child(
            // One line that scrolls sideways (`.approval-from-id`): the shortened ID keeps an escape spelling whole,
            // so it can still outgrow its slot, and wrapping would split the spelling the shortening protects. The
            // narrow cap is the old `calc(100% - 75px)` of the line, which is the card less its 12-pixel padding and
            // 2-pixel border on each side.
            div()
                .id(ElementId::Name(id("id-scroll")))
                .min_w_0()
                .when(narrow, |slot| slot.max_w(width - px(2. * (12. + 2.) + 75.)))
                .when(!narrow, |slot| slot.max_w(gpui::relative(0.28)))
                .overflow_x_scroll()
                .whitespace_nowrap()
                .child(short),
        )
        .child(
            copy_button(
                id("copy"),
                COPY_ID,
                HoverHelp::tooltip(COPY_HELP),
                from.id.clone(),
            )
            .accessible_name(FROM_COPY_NAME)
            .compact(),
        )
}

// ---- Facts -----------------------------------------------------------------------------------------------------------

/// One labelled fact (`.approval-card-row`): the label in its column, or above the value in a narrow card.
fn row(card: &Card, index: usize, fact: &Fact, narrow: bool, palette: &Palette) -> AnyElement {
    let label = div()
        .flex_none()
        .when(!narrow, |label| label.w(LABEL_WIDTH))
        .text_color(theme::color(palette.fg_dim))
        .child(fact.label());
    let value = div()
        .flex_1()
        .min_w_0()
        .child(value(card, index, fact, palette));
    div()
        .flex()
        .when(narrow, |row| row.flex_col().gap(px(2.)))
        .when(!narrow, |row| row.flex_row().gap(px(8.)))
        .min_w_0()
        .child(label)
        .child(value)
        .into_any_element()
}

/// A fact's value, laid out as its shape says.
fn value(card: &Card, index: usize, fact: &Fact, palette: &Palette) -> AnyElement {
    let id = SharedString::from(format!("approval-{}-fact-{index}", card.id));
    match fact {
        Fact::Value(_, text) => match fact.shape() {
            Shape::OneLine => one_line(id, host_text::element(text)),
            Shape::WrappedMono => host_text::element(text)
                .font_family(assets::TERMINAL_FONT)
                .into_any_element(),
            Shape::Chip => chip(host_text::element(text), false, palette),
            Shape::Wrapped => host_text::element(text).into_any_element(),
        },
        // The app's own words, sentence-cased as the old card showed them ("Generated", "None"). The old card set them
        // in italics; they stay upright here because the app bundles no italic face (a recorded deviation), and asking
        // for one would let the platform substitute another family.
        Fact::Words(_, words) => match fact.shape() {
            Shape::Chip => chip(div().child(sentence_case(words)), false, palette),
            Shape::OneLine | Shape::WrappedMono | Shape::Wrapped => {
                div().child(sentence_case(words)).into_any_element()
            }
        },
        Fact::Identity(_, raw) => {
            one_line(id, host_text::element(&HostText::identity(raw.as_str())))
        }
        Fact::CopyIdentity(_, raw) => div()
            .flex()
            .items_center()
            .gap(px(6.))
            .min_w_0()
            .child(div().flex_1().min_w_0().child(one_line(
                id.clone(),
                host_text::element(&HostText::identity(raw.as_str())),
            )))
            .child(
                copy_button(
                    SharedString::from(format!("{id}-copy")),
                    COPY,
                    HoverHelp::tooltip(COPY_HELP),
                    raw.clone(),
                )
                // A full-size button, as the old target-ID Copy was (`.btn`); only the From line's Copy ID is compact.
                .accessible_name(fact.copy_name().unwrap_or_default()),
            )
            .into_any_element(),
        Fact::Permission(chips) => permission(chips, palette),
        Fact::Box(..) => div().into_any_element(),
    }
}

/// A value on one line in the monospace face that scrolls sideways within itself (`.approval-fact-scroll`).
fn one_line(id: SharedString, value: gpui::Div) -> AnyElement {
    div()
        .id(ElementId::Name(id))
        .min_w_0()
        .overflow_x_scroll()
        .whitespace_nowrap()
        .font_family(assets::TERMINAL_FONT)
        .child(value)
        .into_any_element()
}

/// A chip (`.approval-permission-chip`), warning-toned for YOLO and the agent's own setting.
fn chip(content: impl IntoElement, warning: bool, palette: &Palette) -> AnyElement {
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(4.))
        .px(px(6.))
        .py(px(1.))
        .border_1()
        .rounded(metrics::RADIUS_SM)
        .text_size(metrics::TEXT_SM)
        .line_height(SMALL_LINE)
        .when(warning, |chip| {
            chip.border_color(theme::color(palette.warn))
                .text_color(theme::color(palette.warn))
                .bg(theme::color(palette.warn_chip_bg))
        })
        .when(!warning, |chip| {
            chip.border_color(theme::color(palette.border))
        })
        .child(content)
        .into_any_element()
}

/// The Permissions row: the session list's shield with the chip's words and description, then stored workspace trust
/// on a line of its own (`PermissionChips`).
fn permission(chips: &Chips, palette: &Palette) -> AnyElement {
    let shield = svg()
        .path(chips.icon().path())
        .size(px(12.))
        .flex_none()
        .text_color(theme::color(chips.ink(palette)));
    let line = div()
        .flex()
        .flex_wrap()
        .items_baseline()
        .gap(px(8.))
        .child(chip(
            div()
                .flex()
                .items_center()
                .gap(px(4.))
                .child(shield)
                .child(chips.kind.label()),
            chips.warning(),
            palette,
        ))
        .child(
            // At most the line's width, so a long description wraps under the chip instead of running past the card.
            div()
                .min_w_0()
                .max_w_full()
                .text_size(metrics::TEXT_SM)
                .line_height(SMALL_LINE)
                .child(chips.description().to_owned()),
        );
    let trust = chips.trust.map(|trusted| {
        div()
            .flex()
            .flex_wrap()
            .items_baseline()
            .gap(px(8.))
            .mt(px(5.))
            .child(chip(
                div().child(if trusted {
                    "Trusted workspace"
                } else {
                    "Not trusted"
                }),
                false,
                palette,
            ))
            .when(trusted, |line| {
                line.child(
                    div()
                        .text_size(metrics::TEXT_SM)
                        .line_height(SMALL_LINE)
                        .child(TRUSTED_WORKSPACE),
                )
            })
    });
    div()
        .flex()
        .flex_col()
        .child(line)
        .children(trust)
        .into_any_element()
}

/// A long text under its label, in a bounded box that keeps every line (`.approval-card-command` and
/// `.approval-card-block`): it shrinks toward two lines before the fact list needs to scroll, and grows to six. The box
/// is the shared keyboard-scrollable one ([`ScrollBox`]), a tab stop named for its label as the old labelled box was, so
/// the whole text is reachable without a pointer. Commands, resume commands and wrapper words are in the monospace face
/// (the old `pre`); a prompt keeps the interface face (`.approval-card-prompt`), being prose.
fn long_text(card: &Card, index: usize, fact: &Fact, palette: &Palette) -> AnyElement {
    let Fact::Box(_, text) = fact else {
        return div().into_any_element();
    };
    let label = fact.label();
    // Each line its own host-text element, so a line break the agent wrote stays a line break and every character on
    // the line is escaped as host text anywhere is.
    let lines = text.raw().replace("\r\n", "\n");
    let lines = div().flex().flex_col().children(
        lines
            .split('\n')
            .map(|line| host_text::element(&HostText::from_host(line)).min_h(BOX_LINE)),
    );
    let scroll = ScrollBox::new(
        SharedString::from(format!("approval-{}-box-{index}", card.id)),
        label.clone(),
        HoverHelp::tooltip(format!(
            "{label}: the whole text, scrolled from the keyboard once focused"
        )),
        Role::Region,
        lines,
    )
    .min_height(px(3.2 * 16. + 14.))
    .max_height(px(9.6 * 16. + 14.));
    let scroll = match fact.shape() {
        Shape::WrappedMono => scroll.monospace(),
        Shape::OneLine | Shape::Wrapped | Shape::Chip => scroll,
    };
    div()
        .flex()
        .flex_col()
        .flex_shrink_1()
        .min_h(px(4.6 * 16. + 18.))
        .max_h(px(11. * 16. + 18.))
        .gap(px(4.))
        .child(
            div()
                .flex_none()
                .line_height(BOX_LABEL_LINE)
                .text_color(theme::color(palette.fg_dim))
                .child(label),
        )
        .child(scroll)
        .into_any_element()
}

// ---- Notes -----------------------------------------------------------------------------------------------------------

/// A note in the app's words (`.approval-card-note`). Upright, not the old italics: see [`value`]'s note on the app's
/// words.
fn note(words: &'static str) -> impl IntoElement {
    div().flex_none().child(words)
}

/// The note a link card draws first: the tunnel's port and host, or the local-network explanation.
fn lead_note(card: &Card, lead: &LeadNote) -> AnyElement {
    match lead {
        LeadNote::Tunnel { port, host } => {
            let (before, after) = LeadNote::tunnel_words(*port);
            div()
                .flex_none()
                .child(
                    HostSentence::new(SharedString::from(format!(
                        "approval-{}-tunnel-note",
                        card.id
                    )))
                    .text(before)
                    .host(host)
                    .text(after),
                )
                .into_any_element()
        }
        LeadNote::LocalNetwork => note(LeadNote::LOCAL_NETWORK).into_any_element(),
    }
}

// ---- The answer bar --------------------------------------------------------------------------------------------------

/// The answer buttons, wrapping in a narrow pane, with the last (Deny or Dismiss) apart at the right
/// (`.approval-card-actions`).
fn answer_bar(
    card: &Card,
    state: &CardState,
    on_answer: OnAnswer,
    palette: &Palette,
) -> impl IntoElement {
    let last = card.answers.len().saturating_sub(1);
    div()
        .flex()
        .flex_none()
        .flex_wrap()
        .gap(px(8.))
        .mx(px(-12.))
        .py(px(10.))
        .px(px(12.))
        .border_t_1()
        .border_color(theme::color(palette.border_row))
        .bg(theme::color(palette.bg_1))
        .children(card.answers.iter().enumerate().map(|(index, answer)| {
            let button = answer_button(card, answer, state.busy, on_answer.clone());
            div()
                .min_w_0()
                .max_w_full()
                .when(index == last, |slot| slot.ml_auto())
                .child(button)
        }))
}

/// One answer button, with its words (naming the requesting host as host text where they do) and its help.
fn answer_button(card: &Card, answer: &Answer, busy: bool, on_answer: OnAnswer) -> Button {
    let id = SharedString::from(format!(
        "approval-{}-answer-{}",
        card.id,
        word(answer.answer)
    ));
    let help = match &answer.help {
        AnswerHelp::Words(words) => HoverHelp::tooltip(*words),
        AnswerHelp::OwnHost(host) => HoverHelp::sentence(
            HostSentence::new(SharedString::from(format!("{id}-help")))
                .text(OWN_HOST_HELP[0])
                .host(host)
                .text(OWN_HOST_HELP[1])
                .host(host)
                .text(OWN_HOST_HELP[2]),
        ),
    };
    let (label, sentence) = match &answer.label {
        AnswerLabel::Words(words) => (SharedString::from(*words), None),
        AnswerLabel::OnHostOnly(host) => {
            let sentence = HostSentence::new(SharedString::from(format!("{id}-label")))
                .text(ON_HOST_ONLY.0)
                .host(host)
                .text(ON_HOST_ONLY.1);
            (sentence.accessible_name(), Some(sentence))
        }
        AnswerLabel::FromHost(host) => {
            let sentence = HostSentence::new(SharedString::from(format!("{id}-label")))
                .text(FROM_HOST)
                .host(host);
            (sentence.accessible_name(), Some(sentence))
        }
    };
    let chosen = answer.answer;
    let button = Button::new(id, label, help, Role::Button)
        .tier(if answer.primary {
            Tier::Primary
        } else {
            Tier::Neutral
        })
        .busy(busy)
        .on_click(move |_, window, cx| on_answer(chosen, window, cx));
    match sentence {
        Some(sentence) => button.content(sentence),
        None => button,
    }
}

/// An answer's word in its button's element id (kebab-case, unlike the wire's snake_case), which tests and the bridge
/// find the button by.
fn word(answer: ApprovalAnswer) -> &'static str {
    match answer {
        ApprovalAnswer::Allow => "allow",
        ApprovalAnswer::TunnelAndOpen => "tunnel-and-open",
        ApprovalAnswer::AlwaysAllow => "always-allow",
        ApprovalAnswer::AlwaysAllowOwnHost => "always-allow-own-host",
        ApprovalAnswer::Deny => "deny",
    }
}
