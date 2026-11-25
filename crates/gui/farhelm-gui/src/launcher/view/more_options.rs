//! The Agent tab's disclosure. Folding only removes editors from view. Values remain in the draft and in the
//! folded summary, and a required or refused value keeps its editor and explanation reachable.

use farhelm_proto::LaunchHarness;
use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, Context, Div, FontWeight, InteractiveElement as _, IntoElement as _,
    ParentElement as _, Styled as _, Transformation, Window, div, px, radians, relative, svg,
};

use super::{
    Input, Launcher, choices, help_line, label_with_note, labelled_note, on_launcher, row,
    shown_text,
};
use crate::assets::Icon;
use crate::components::{Button, HoverHelp, Role, Tier};
use crate::host_text::{HostSentence, HostText};
use crate::launcher::draft::harness_label;
use crate::launcher::request::capitalized;
use crate::theme::{self, metrics};

const WRAPPER_LABEL: &str = "Wrapper prefix";

impl Launcher {
    /// Show the disclosure after the ordinary Agent settings. Keep expansion after a correction so typing a valid
    /// value cannot remove the focused editor. The draft, which owns both values, survives folding and tab switches.
    pub(super) fn more_options(
        &mut self,
        harness: Option<LaunchHarness>,
        palette: &theme::Palette,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let blocker = self.custom_restart_refusal(cx);
        let offered = harness.filter(|harness| {
            blocker.is_none()
                && (self.draft.offers_conversation()
                    || (harness.uses_custom_catalog() && !self.draft.conversation.is_empty()))
        });
        let label = offered.map(conversation_label);
        let (required, conversation_error) = self.conversation_attention(blocker);
        let wrapper_error = self.draft.wrapper_error();
        let must_open = required || conversation_error.is_some() || wrapper_error.is_some();
        self.more_options_open |= must_open;
        let mut fields = Vec::new();
        if let Some(reason) = blocker {
            fields.push(
                choices::status_line("launcher-custom-restart-blocked", reason, palette)
                    .into_any_element(),
            );
        }
        fields.push(self.more_options_toggle(label, must_open, palette, cx));
        if !self.more_options_open {
            return fields;
        }
        if let Some((harness, label)) = offered.zip(label) {
            fields.push(
                row(
                    "launcher-conversation-row",
                    label,
                    self.launcher_input(
                        "launcher-conversation",
                        &self.conversation,
                        label,
                        HoverHelp::tooltip(format!(
                            "{label}: start {} on this conversation, this start only",
                            harness_label(harness)
                        )),
                    ),
                    palette,
                )
                .into_any_element(),
            );
            if let Some(error) = conversation_error {
                fields.push(
                    choices::status_line("launcher-conversation-error", error, palette)
                        .into_any_element(),
                );
            }
            if required {
                fields.push(
                    help_line(
                        "Farhelm has no captured conversation to resume. Enter its ID or session file to restart here.",
                        palette,
                    )
                    .into_any_element(),
                );
            }
            if !harness.captures_named_conversation()
                || (harness.uses_custom_catalog() && !self.draft.agent.custom_reporting)
            {
                fields.push(
                    help_line(
                        "Farhelm will not capture this conversation after a named start, so Restart will not be \
                         available.",
                        palette,
                    )
                    .into_any_element(),
                );
            }
        }
        fields.push(
            row(
                "launcher-wrapper-row",
                label_with_note(
                    WRAPPER_LABEL,
                    labelled_note("runs through Bash before the launch command"),
                    palette,
                ),
                self.launcher_input(
                    "launcher-wrapper",
                    &self.wrapper,
                    WRAPPER_LABEL,
                    HoverHelp::tooltip(
                        "Wrapper prefix: Bash text before the launch command. Leave blank for none. Quotes and \
                         expansions keep their shell meaning. There are no placeholders.",
                    ),
                )
                .monospace(),
                palette,
            )
            .into_any_element(),
        );
        if Input::Wrapper.present(&self.draft, &self.opened).read_only {
            fields.push(shown_text::clear_button(
                "launcher-clear-wrapper",
                "Clear wrapper prefix",
                "Clear wrapper prefix: remove this value before editing",
                Launcher::clear_wrapper,
                cx,
            ));
        }
        if let Some(error) = wrapper_error {
            fields.push(
                choices::status_line("launcher-wrapper-error", capitalized(&error), palette)
                    .into_any_element(),
            );
        }
        fields
    }

    /// The whole row is one focusable disclosure. Its accessible name and hover help give the full escaped
    /// spelling even when the viewport has room for only the beginning of a pill.
    fn more_options_toggle(
        &self,
        label: Option<&str>,
        must_open: bool,
        palette: &theme::Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let labels = label.map_or_else(
            || WRAPPER_LABEL.to_owned(),
            |label| format!("{label} · {WRAPPER_LABEL}"),
        );
        let mut name = HostSentence::new("launcher-more-options-help")
            .text(format!("More options. {labels}."));
        let open = self.more_options_open;
        let mut summary = div()
            .ml_auto()
            .flex()
            .min_w_0()
            .max_w(relative(0.48))
            .items_center()
            .gap(px(4.));
        if !open {
            let conversation = label
                .zip(self.draft.active_conversation())
                .map(|(label, value)| ("launcher-more-options-conversation", label, value, false));
            let wrapper = self.draft.agent.wrapper.as_str();
            let wrapper = (!wrapper.is_empty()).then_some((
                "launcher-more-options-wrapper",
                WRAPPER_LABEL,
                wrapper,
                true,
            ));
            if conversation.is_none() && wrapper.is_none() {
                name = name.text(" None set.");
                summary = summary.child(
                    div()
                        .id("launcher-more-options-none")
                        .text_color(theme::color(palette.fg_2))
                        .child("none set"),
                );
            }
            for (id, label, value, mono) in conversation.into_iter().chain(wrapper) {
                let value = HostText::from_host(value);
                name = name.text(format!(" {label}: ")).host(&value).text(".");
                summary = summary.child(value_pill(id, &value, mono, palette));
            }
        }
        if must_open {
            name = name.text(" A setting here is required or needs attention.");
        }

        let caret = svg()
            .path(Icon::ChevronDown.path())
            .size(px(12.))
            .flex_none()
            .text_color(theme::color(palette.fg_1))
            .with_transformation(Transformation::rotate(radians(if open {
                0.
            } else {
                -std::f32::consts::FRAC_PI_2
            })));
        let content = div()
            .flex()
            .w_full()
            .min_w_0()
            .items_center()
            .flex_wrap()
            .gap_x(px(8.))
            .gap_y(px(4.))
            .child(
                div().flex().items_center().gap(px(6.)).child(caret).child(
                    div()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme::color(palette.fg_0))
                        .child("More options"),
                ),
            )
            .child(div().text_color(theme::color(palette.fg_2)).child(labels))
            .when(!open, |content| content.child(summary));
        div()
            .border_1()
            .border_dashed()
            .border_color(theme::color(palette.border_dim))
            .rounded(metrics::RADIUS_SM)
            .child(
                Button::new(
                    "launcher-more-options",
                    "More options",
                    HoverHelp::sentence(name.clone()),
                    Role::Button,
                )
                .tier(Tier::Ghost)
                .accessible_name(name.accessible_name())
                .discloses(open)
                .busy(must_open)
                .content(content)
                .on_click(on_launcher(cx.weak_entity(), Self::toggle_more_options)),
            )
            .into_any_element()
    }

    /// Keep a conversation error or requirement only while the current registration can use the field.
    fn conversation_attention(&self, blocker: Option<&str>) -> (bool, Option<String>) {
        if blocker.is_some() {
            return (false, None);
        }
        (
            self.draft.requires_conversation(),
            self.draft.conversation_error(),
        )
    }

    /// A press can arrive before a redraw following a draft edit. Recheck whether folding would hide a field
    /// that needs attention, and leave the draft's values and any launch refusal alone.
    fn toggle_more_options(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        let (required, error) = self.conversation_attention(self.custom_restart_refusal(cx));
        if required || error.is_some() || self.draft.wrapper_error().is_some() {
            return;
        }
        self.more_options_open = !self.more_options_open;
        cx.notify();
    }
}

/// Each value gets its own chip so one long value can shorten without taking the other value out of the summary.
/// The editor and the disclosure's accessible name retain the full value.
fn value_pill(id: &'static str, value: &HostText, mono: bool, palette: &theme::Palette) -> Div {
    let sentence = HostSentence::new(id);
    let sentence = if mono {
        sentence.mono(value)
    } else {
        sentence.host(value)
    };
    div()
        .min_w(px(24.))
        .max_w_full()
        .truncate()
        .rounded(metrics::RADIUS_SM)
        .border_1()
        .border_color(theme::color(palette.accent_edge))
        .px(px(5.))
        .text_size(metrics::TEXT_SM)
        .text_color(theme::color(palette.accent))
        .child(sentence)
}

/// The selector's label uses the harness's own vocabulary, including when it is folded.
const fn conversation_label(harness: LaunchHarness) -> &'static str {
    match harness {
        LaunchHarness::Pi | LaunchHarness::Omp => "Session file",
        LaunchHarness::Goose | LaunchHarness::OpenCode => "Session ID",
        LaunchHarness::Cursor => "Chat ID",
        LaunchHarness::Claude
        | LaunchHarness::Codex
        | LaunchHarness::Muse
        | LaunchHarness::Antigravity
        | LaunchHarness::Custom
        | LaunchHarness::Grok => "Conversation ID",
        LaunchHarness::Bash | LaunchHarness::Zsh => "Conversation ID",
    }
}
