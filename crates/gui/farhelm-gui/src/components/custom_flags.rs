//! The Custom permission flags block: the flags field its owner draws, what the flags mean, and the words they parse
//! to (each in an escaped box of its own) or why they do not parse, set off by a warning stripe because these flags
//! make the session YOLO.
//!
//! The old app drew this block once, as `launch_controls::CustomPermissionFlags` (`.custom-permission-choice` in its
//! stylesheet at reference commit `24bc0580`), for both the session launcher and the Templates editor; this is that
//! component again, so the two cannot drift apart on the sentence, the words' accessible name or the error.
//!
//! It holds no control of its own and takes no focus: the flags field is its owner's text input, laid out the owner's
//! way (the launcher puts the label beside the field, the Templates editor above it), and handed in as an element.
//! The rest is display: a help line and the words, or the parse error, which assistive technology reads through their
//! labelled nodes. Under the component layer's rule a control's constructor takes a label, hover help and a role; this
//! block is not a control, so it takes none, and the field inside it is the owner's control with its own.
//!
//! The parsed words are a host value as far as the screen is concerned: words someone typed or an agent wrote into a
//! template, which may hold hidden characters, so each is drawn through the host-text element.

use gpui::{
    AnyElement, App, ElementId, InteractiveElement as _, IntoElement, ParentElement as _, Pixels,
    RenderOnce, Role, SharedString, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};

use crate::host_text::{self, HostSentence, HostText};
use crate::theme::{self, metrics};

/// `.custom-permission-choice`: the gap between the block's lines, and the room between the warning stripe and them.
const GAP: f32 = 6.;
const STRIPE_PAD: f32 = 10.;

/// The Custom permission flags block. See the module docs.
#[derive(IntoElement)]
pub struct CustomFlags {
    prefix: SharedString,
    field: AnyElement,
    typed: SharedString,
    agent: SharedString,
    indent: Pixels,
}

impl CustomFlags {
    /// The block for the flags `typed` (as the owner's `field` holds them), passed to `agent` (its display name, or
    /// "the agent" before one is chosen). `prefix` names the block's nodes for tests and the test bridge: the words are
    /// `<prefix>-flag-words` and the error `<prefix>-flags-error`.
    pub fn new(
        prefix: impl Into<SharedString>,
        field: impl IntoElement,
        typed: impl Into<SharedString>,
        agent: impl Into<SharedString>,
    ) -> Self {
        CustomFlags {
            prefix: prefix.into(),
            field: field.into_any_element(),
            typed: typed.into(),
            agent: agent.into(),
            indent: px(0.),
        }
    }

    /// Start the lines under the field `indent` in, so they line up with a field drawn beside its label (the
    /// launcher's label column).
    pub fn indent(mut self, indent: Pixels) -> Self {
        self.indent = indent;
        self
    }
}

impl RenderOnce for CustomFlags {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let palette = theme::palette(cx);
        let help: SharedString = format!(
            "Passed to {} instead of Farhelm's permission flags. You are responsible for ensuring they only change \
             permissions. Farhelm does not check them, so this session counts as YOLO.",
            self.agent
        )
        .into();
        // A labelled node named by its words, since plain text is not in the accessibility tree.
        let help_line = div()
            .id(ElementId::Name(help.clone()))
            .role(Role::Label)
            .aria_label(help.clone())
            .pl(self.indent)
            .text_size(metrics::TEXT_SM)
            .text_color(theme::color(palette.fg_1))
            .child(help);
        let outcome = match farhelm_proto::split_permission_flags(&self.typed) {
            Ok(words) => {
                let words: Vec<HostText> = words
                    .iter()
                    .map(|word| HostText::from_host(word.as_str()))
                    .collect();
                // Named as the old `aria-label="Permission arguments"` list read, each word escaped and the words
                // apart by commas, so `--a 'b c'` (two words) and `--a b c` (three) do not sound the same.
                let name = words
                    .iter()
                    .enumerate()
                    .fold(
                        HostSentence::new(SharedString::from(format!(
                            "{}-flag-words-name",
                            self.prefix
                        )))
                        .text("Permission arguments:"),
                        |name, (index, word)| {
                            name.text(if index == 0 { " " } else { ", " }).host(word)
                        },
                    )
                    .accessible_name();
                div()
                    .id(ElementId::Name(
                        format!("{}-flag-words", self.prefix).into(),
                    ))
                    .role(Role::Label)
                    .aria_label(name)
                    .pl(self.indent)
                    .flex()
                    .flex_wrap()
                    .gap(px(4.))
                    .children(words.iter().map(|word| {
                        host_text::element(word)
                            .px(px(5.))
                            .border_1()
                            .border_color(theme::color(palette.border))
                            .font_family(crate::assets::TERMINAL_FONT)
                    }))
                    .into_any_element()
            }
            Err(error) => {
                let words = capitalized(error);
                div()
                    .id(ElementId::Name(
                        format!("{}-flags-error", self.prefix).into(),
                    ))
                    .role(Role::Alert)
                    .aria_label(words.clone())
                    .pl(self.indent)
                    .text_size(metrics::TEXT_SM)
                    // `.launch-composer-error`, which the old block used wherever it was drawn: amber, red being kept
                    // for danger.
                    .text_color(theme::color(palette.warn_bright))
                    .child(words)
                    .into_any_element()
            }
        };
        div()
            .flex()
            .flex_col()
            .gap(px(GAP))
            .border_l_2()
            .border_color(theme::color(palette.warn))
            .pl(px(STRIPE_PAD))
            .child(self.field)
            .child(help_line)
            .child(outcome)
    }
}

/// A parse error as a sentence: its first letter capitalized, as the old app's `local_validation_notice` showed it.
fn capitalized(mut reason: String) -> String {
    if let Some(first) = reason.get_mut(..1) {
        first.make_ascii_uppercase();
    }
    reason
}
