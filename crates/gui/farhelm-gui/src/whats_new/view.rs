//! Draw the finalized release feed in the shared launcher style. The presenter reads the same feed on older and
//! newer releases, so a highlighted entry can have a curated headline or consist entirely of body text.

use gpui::{
    Div, FontWeight, HighlightStyle, InteractiveElement as _, IntoElement, ParentElement as _,
    SharedString, StatefulInteractiveElement as _, Styled as _, StyledText, div, px, relative,
};

use crate::assets::{Icon, TERMINAL_FONT};
use crate::components::{
    Button, HoverHelp, Role, Tier, chips_and_words, command_glyph, command_heading_with_count,
    command_icon_tile, key_chip,
};
use crate::desktop::updates::notes;
use crate::theme::{self, Palette, metrics};

/// The feed owns category spelling and order. New names can still identify their entries with a neutral mark.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Category {
    Added,
    Changed,
    Fixed,
    Breaking,
    Removed,
    Other,
}

impl Category {
    /// In display order. The boolean says whether to show the count even when it is zero.
    const COUNTS: [(Self, &'static str, bool); 5] = [
        (Self::Added, "new", true),
        (Self::Changed, "changed", true),
        (Self::Fixed, "fixed", true),
        (Self::Breaking, "breaking", false),
        (Self::Removed, "removed", false),
    ];

    fn from_feed(name: &str) -> Self {
        match name {
            "Added" => Self::Added,
            "Changed" => Self::Changed,
            "Fixes worth highlighting" => Self::Fixed,
            "Breaking" => Self::Breaking,
            "Removed" => Self::Removed,
            _ => Self::Other,
        }
    }

    /// Pair a shape with its tint so the category still reads when the colors look similar.
    fn mark(self, size: f32, palette: &Palette) -> Div {
        let (icon, ink, ground) = match self {
            Self::Added => (Icon::Add, palette.ok, palette.ok_badge_bg),
            Self::Changed => (Icon::Refresh, palette.accent, palette.marked_bg),
            Self::Fixed => (Icon::Wrench, palette.warn, palette.warn_badge_bg),
            Self::Breaking => (Icon::EndedError, palette.danger, palette.danger_wash),
            Self::Removed => (Icon::Trash, palette.fg_1, palette.bg_1),
            Self::Other => (Icon::Info, palette.fg_1, palette.bg_1),
        };
        div()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(px(size))
            .rounded(metrics::RADIUS_SM)
            .bg(theme::color(ground))
            .child(command_glyph(icon, ink))
    }
}

/// The header names the upgrade and counts the highlights actually shown. The release labels have the full totals.
pub(super) fn heading(
    range: &str,
    range_label: &str,
    releases: &[notes::ReleaseHighlights],
    palette: &Palette,
) -> Div {
    let counts = Category::COUNTS
        .into_iter()
        .filter_map(|(kind, name, show_zero)| {
            let count: usize = releases
                .iter()
                .flat_map(|release| &release.highlights)
                .filter(|category| Category::from_feed(&category.name) == kind)
                .map(|category| category.entries.len())
                .sum();
            (show_zero || count > 0).then(|| {
                let label = format!("{count} {name}");
                div()
                    .id(SharedString::from(format!("whats-new-count-{name}")))
                    .role(Role::Label)
                    .aria_label(label.clone())
                    .flex()
                    .items_center()
                    .gap(px(5.))
                    .child(kind.mark(17., palette))
                    .child(label)
            })
        });
    div()
        .flex()
        .items_start()
        .gap(px(10.))
        .child(command_icon_tile(Icon::Sparkle, palette).flex_none())
        .child(
            div()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap_x(px(9.))
                        .gap_y(px(4.))
                        .child(
                            div()
                                .text_size(metrics::TEXT_TITLE)
                                .font_weight(FontWeight::SEMIBOLD)
                                .child("What's new"),
                        )
                        .child(
                            div()
                                .id("whats-new-version-range")
                                .role(Role::Label)
                                .aria_label(range_label.to_owned())
                                .rounded_full()
                                .px(px(7.))
                                .py(px(2.))
                                .bg(theme::color(palette.marked_bg))
                                .text_size(px(11.))
                                .font_family(TERMINAL_FONT)
                                .text_color(theme::color(palette.accent))
                                .child(range.to_owned()),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap_x(px(12.))
                        .gap_y(px(4.))
                        .text_size(metrics::TEXT_SM)
                        .text_color(theme::color(palette.fg_1))
                        .children(counts),
                ),
        )
}

/// The frame keeps these hints and the actions in view when a long release scrolls.
pub(super) fn footer_hints(palette: &Palette) -> impl IntoElement {
    let pair = |key, words| {
        chips_and_words([
            key_chip(key, palette.fg_1, palette).into_any_element(),
            div().child(words).into_any_element(),
        ])
    };
    div()
        .id("whats-new-footer-keys")
        .role(Role::Label)
        .aria_label("Enter for Got it, Escape to close")
        .flex()
        .flex_wrap()
        .gap(px(12.))
        .text_size(metrics::TEXT_SM)
        .text_color(theme::color(palette.fg_1))
        .child(pair("⏎", "got it"))
        .child(pair("esc", "close"))
}

/// Only a complete leading bold sentence from curation earns its own line. Older highlights can begin with
/// emphasized control names, so the rest of the entry must contain visible text and the punctuation must be bold.
pub(super) fn headline(
    pieces: &[notes::HighlightPiece],
) -> Option<(&str, &[notes::HighlightPiece])> {
    let (first, rest) = pieces.split_first()?;
    let text = first.text.as_str();
    (matches!(first.style, notes::HighlightStyle::Bold)
        && text.ends_with(['.', '?', '!'])
        && rest.iter().any(|piece| !piece.text.trim().is_empty()))
    .then_some((text, rest))
}

/// Strip the join across leading pieces after a headline. Keep the remaining spacing and styles, including any
/// leading whitespace in an older entry whose full text fits in a single styled paragraph.
pub(super) fn body_pieces(
    pieces: &[notes::HighlightPiece],
    trim_join: bool,
) -> impl Iterator<Item = (notes::HighlightStyle, &str)> {
    let mut at_join = trim_join;
    pieces.iter().filter_map(move |piece| {
        let words = if at_join {
            piece.text.trim_start()
        } else {
            piece.text.as_str()
        };
        if words.is_empty() {
            return None;
        }
        at_join = false;
        Some((piece.style, words))
    })
}

/// Text runs wrap at the row's width. Code ranges split the drawing's runs to carry the font override.
fn paragraph(pieces: &[notes::HighlightPiece], trim_join: bool) -> StyledText {
    let mut text = String::new();
    let mut highlights = Vec::new();
    let mut code = Vec::new();
    for (style, words) in body_pieces(pieces, trim_join) {
        let start = text.len();
        text.push_str(words);
        let range = start..text.len();
        match style {
            notes::HighlightStyle::Plain => {}
            notes::HighlightStyle::Bold => highlights.push((
                range,
                HighlightStyle {
                    font_weight: Some(FontWeight::BOLD),
                    ..HighlightStyle::default()
                },
            )),
            notes::HighlightStyle::Code => {
                highlights.push((range.clone(), HighlightStyle::default()));
                code.push((range, SharedString::from(TERMINAL_FONT)));
            }
        }
    }
    StyledText::new(SharedString::from(text))
        .with_highlights(highlights)
        .with_font_family_overrides(code)
}

/// The screenshot link stays independently focusable after the text. The entry's accessible label carries the
/// category and the entire paragraph, including a headline when present.
fn entry_row(suffix: String, category: &str, entry: &notes::Highlight, palette: &Palette) -> Div {
    let label = entry
        .pieces
        .iter()
        .fold(format!("{category}: "), |mut label, piece| {
            label.push_str(&piece.text);
            label
        });
    let (title, rest) = headline(&entry.pieces)
        .map(|(title, rest)| (Some(title), rest))
        .unwrap_or((None, &entry.pieces));
    let link = entry.screenshot_url.clone().map(|url| {
        Button::new(
            SharedString::from(format!("whats-new-shot-{suffix}")),
            "Screenshot ↗",
            HoverHelp::tooltip("Open this entry on the Release notes page"),
            Role::Link,
        )
        .tier(Tier::Link)
        .on_click(move |_, _, cx| cx.open_url(&url))
    });
    div()
        .flex()
        .items_start()
        .gap(px(10.))
        .px(px(6.))
        .py(px(5.))
        .child(Category::from_feed(category).mark(21., palette))
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .gap(px(3.))
                .items_start()
                .child(
                    div()
                        .id(SharedString::from(format!("whats-new-entry-{suffix}")))
                        .role(Role::Label)
                        .aria_label(label)
                        .w_full()
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .children(title.map(|title| {
                            div()
                                .text_size(metrics::TEXT_ROW_TITLE)
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme::color(palette.fg_0))
                                .child(title.to_owned())
                        }))
                        .child(
                            div()
                                .text_size(px(12.5))
                                .line_height(relative(1.45))
                                .text_color(theme::color(palette.fg_1))
                                .child(paragraph(rest, title.is_some())),
                        ),
                )
                .children(link),
        )
}

/// Display the feed's ISO date compactly. Keep the supplied string if a future feed uses another shape.
fn short_date(date: &str) -> String {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let mut parts = date.split('-');
    if let (Some(year), Some(month), Some(day), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
        && year.len() == 4
        && year.bytes().all(|byte| byte.is_ascii_digit())
        && let (Ok(month), Ok(day)) = (month.parse::<usize>(), day.parse::<u8>())
        && (1..=31).contains(&day)
        && let Some(month) = month.checked_sub(1).and_then(|month| MONTHS.get(month))
    {
        format!("{month} {day}")
    } else {
        date.to_owned()
    }
}

/// Keep the feed's order and distinguish a release's total changes from the number of featured entries.
pub(super) fn release_section(release: &notes::ReleaseHighlights, palette: &Palette) -> Div {
    let count = release.total_changes;
    let changes = if count == 1 { "change" } else { "changes" };
    div()
        .flex()
        .flex_col()
        .gap(px(4.))
        .child(
            command_heading_with_count(
                format!("whats-new-release-{}", release.version),
                &format!("{} · {}", release.version, short_date(&release.date)),
                Some(format!("{count} {changes}")),
                palette,
            )
            .role(Role::Heading),
        )
        .children(
            release
                .highlights
                .iter()
                .enumerate()
                .flat_map(|(index, category)| {
                    category
                        .entries
                        .iter()
                        .enumerate()
                        .map(move |(entry_index, entry)| {
                            entry_row(
                                format!("{}-{index}-{entry_index}", release.version),
                                &category.name,
                                entry,
                                palette,
                            )
                        })
                }),
        )
}
