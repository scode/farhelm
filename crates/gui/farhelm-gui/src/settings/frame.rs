//! Settings keeps navigation and the current page in separate scroll regions.
//! The frame pins the controls common to every page while Tab still reveals controls inside either pane.

use farhelm_proto::ui_channel::Preferences;
use gpui::{
    AnyElement, Context, Div, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _,
    Role, ScrollHandle, StatefulInteractiveElement as _, Styled as _, Window, canvas, div,
    prelude::FluentBuilder as _, px, svg,
};

use super::{SECTION_GAP, Section, SettingsDialog};
use crate::assets::Icon;
use crate::components::{self, Button, HoverHelp, Tier, scroll_from_controls, scroll_handle};
use crate::{focus, layers, theme};

/// Settings' visual grouping determines the tile hue, independent of the page's storage scope.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Group {
    General,
    Placement,
    Agents,
}

impl Group {
    const ALL: [Self; 3] = [Self::General, Self::Placement, Self::Agents];

    /// Group and section order agree with the order used by the search catalog.
    fn of(section: Section) -> Self {
        match section {
            Section::Sessions
            | Section::Appearance
            | Section::Terminal
            | Section::Notifications
            | Section::App => Self::General,
            Section::Hosts
            | Section::Checkouts
            | Section::Containers
            | Section::Recipes
            | Section::Tensorlake => Self::Placement,
            Section::AgentTrust | Section::Credentials => Self::Agents,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Placement => "Where sessions run",
            Self::Agents => "Agents",
        }
    }

    fn hue(self, palette: &theme::Palette) -> theme::Token {
        match self {
            Self::General => palette.settings_general(),
            Self::Placement => palette.settings_placement(),
            Self::Agents => palette.settings_agents(),
        }
    }
}

/// The scope describes which installation the page configures. It stays the same as the page's contents load.
enum Scope {
    Hosts,
    Helm,
    Device,
}

impl Scope {
    fn of(section: Section) -> Self {
        match section {
            Section::Recipes | Section::Tensorlake | Section::Credentials => Self::Helm,
            Section::App => Self::Device,
            Section::Sessions
            | Section::Appearance
            | Section::Terminal
            | Section::Notifications
            | Section::Hosts
            | Section::Checkouts
            | Section::Containers
            | Section::AgentTrust => Self::Hosts,
        }
    }

    fn label(&self) -> &'static str {
        match self {
            Self::Hosts => "Every host on this helm",
            Self::Helm => "This helm",
            Self::Device => "This Mac",
        }
    }

    fn icon(&self) -> Icon {
        match self {
            Self::Hosts => Icon::Hosts,
            Self::Helm => Icon::HostHouse,
            Self::Device => Icon::HostLaptop,
        }
    }
}

impl Section {
    /// The page description belongs in the pinned heading so it remains available on long or scrolled forms.
    fn description(self) -> &'static str {
        match self {
            Self::Sessions => "How session commands and quick launches behave",
            Self::Appearance => "The interface colors",
            Self::Terminal => "Links and files clicked in a terminal",
            Self::Notifications => "Sounds and push alerts when a session needs you",
            Self::App => "This Farhelm app's version and updates",
            Self::Hosts => "Updating, adding and removing hosts",
            Self::Checkouts => "Where managed checkouts go and how they start",
            Self::Containers => "Podman hosts and leftover containers",
            Self::Recipes => "Software and credentials for new containers and sandboxes",
            Self::Tensorlake => "Cloud sandboxes in your Tensorlake project",
            Self::AgentTrust => "Answering agents' folder-trust questions in managed checkouts",
            Self::Credentials => "Secrets Farhelm hands to hosts for you",
        }
    }

    /// Pages with shared choices explain live updates once in the scope label's hover help.
    fn has_live_choices(self, cx: &gpui::App) -> bool {
        match self {
            Self::App => crate::desktop::updates::upgrade_notes(cx).is_some(),
            Self::Sessions
            | Self::Appearance
            | Self::Terminal
            | Self::Notifications
            | Self::Hosts
            | Self::Checkouts
            | Self::AgentTrust
            | Self::Tensorlake => true,
            Self::Containers | Self::Recipes | Self::Credentials => false,
        }
    }

    /// Use the same glyph and role in the 20px section tile and the 32px page tile.
    /// The selected tile gets inverse ink so it stays legible on the accent fill.
    fn tile(self, large: bool, selected: bool, palette: &theme::Palette) -> Div {
        let (side, corner, glyph) = if large {
            (32., 6., 17.)
        } else {
            (20., 4., 13.)
        };
        let hue = Group::of(self).hue(palette);
        let ink = theme::color(if selected { palette.on_fill } else { hue });
        div()
            .flex_none()
            .size(px(side))
            .rounded(px(corner))
            .bg(ink.opacity(if selected { 0.20 } else { 0.24 }))
            .flex()
            .items_center()
            .justify_center()
            .child(
                svg()
                    .path(self.icon().path())
                    .size(px(glyph))
                    .text_color(ink),
            )
    }
}

impl SettingsDialog {
    /// The search field stays reachable even when the section list itself has to scroll.
    pub(super) fn navigation(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let palette = theme::palette(cx);
        let help_edge =
            layers::beside_edge("settings-navigation-help-edge", window, cx).into_any_element();
        let scroll = scroll_handle("settings-navigation-scroll".into(), window, cx);
        let searching = self.search.active();
        let sections = Group::ALL.into_iter().filter_map(|group| {
            let rows: Vec<_> = Section::ALL
                .iter()
                .copied()
                .filter(|&section| Group::of(section) == group && section.listed(cx))
                .filter_map(|section| {
                    let count = searching.then(|| self.search.section_count(section));
                    (count != Some(0)).then(|| self.section_button(section, count, cx))
                })
                .collect();
            (!rows.is_empty()).then(|| {
                div()
                    .flex_none()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(
                        div()
                            .id(("settings-group", group as usize))
                            .role(Role::Label)
                            .aria_label(group.label())
                            .px(px(8.))
                            .pt(px(8.))
                            .pb(px(3.))
                            .text_size(px(10.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme::color(palette.fg_1))
                            .child(group.label().to_uppercase()),
                    )
                    .children(rows)
            })
        });
        let list = div()
            .id("settings-navigation-region")
            .debug_selector(|| "settings-navigation-region".into())
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .gap(px(2.))
            .px(px(3.))
            .pb(px(4.))
            .overflow_y_scroll()
            .children(sections);
        div()
            .id("settings-sections")
            .relative()
            .role(Role::Navigation)
            .aria_label("Settings sections")
            .flex_none()
            .w(px(178.))
            .min_h_0()
            .flex()
            .flex_col()
            .p(px(8.))
            .bg(theme::color(palette.bg_1))
            .border_r_1()
            .border_color(theme::color(palette.border_dim))
            .child(help_edge)
            .child(
                div()
                    .flex_none()
                    .px(px(3.))
                    .child(self.search.field(self.platform, cx)),
            )
            .child(focus::scroll_region(
                scroll_from_controls(list, &scroll),
                &scroll,
            ))
            .into_any_element()
    }

    /// The shared button keeps the platform's focus, tooltip and selection behavior for both ordinary and search rows.
    fn section_button(&self, section: Section, count: Option<usize>, cx: &Context<Self>) -> Button {
        let weak = cx.weak_entity();
        let selected = count.is_none() && section == self.shown;
        let content = div()
            .flex_1()
            .min_w_0()
            .flex()
            .items_center()
            .gap(px(7.))
            .child(section.tile(false, selected, theme::palette(cx)))
            .child(div().flex_1().min_w_0().truncate().child(section.label()))
            .when_some(count, |row, count| {
                row.child(
                    div()
                        .flex_none()
                        .text_size(theme::metrics::TEXT_SM)
                        .child(count.to_string()),
                )
            });
        Button::new(
            format!("settings-section-{}", section.key()),
            section.label(),
            HoverHelp::tooltip(format!("Show {} settings", section.label())).beside_list(false),
            Role::Button,
        )
        .tier(Tier::Nav)
        .pressed(selected)
        .content(content)
        .when_some(count, |button, count| {
            let matches = if count == 1 { "match" } else { "matches" };
            button.accessible_name(format!("{} ({count} {matches})", section.label()))
        })
        .on_click(move |_, window, cx| {
            let _ = weak.update(cx, |dialog, cx| {
                dialog.search.clear(window, cx);
                dialog.show(section, window, cx);
            });
        })
    }

    /// The pinned heading and the page's scroll viewport are siblings, so focus reveal cannot scroll under the heading.
    pub(super) fn page(
        &self,
        preferences: Option<&Preferences>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = theme::palette(cx);
        let scroll = div()
            .id("settings-body")
            .debug_selector(|| "settings-body".into())
            .flex_1()
            .min_h_0()
            .min_w_0()
            .overflow_y_scroll()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .min_w_0()
                    .px(px(20.))
                    .pt(px(10.))
                    .pb(px(16.))
                    .gap(px(SECTION_GAP))
                    .children(self.body(self.shown, preferences, cx)),
            );
        div()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .bg(theme::color(palette.settings_page))
            .child(self.page_heading(window, cx))
            .child(focus::scroll_region(
                scroll_from_controls(scroll, &self.page_scroll),
                &self.page_scroll,
            ))
            .into_any_element()
    }

    /// The scope tells readers where the controls apply without repeating that information under each switch.
    fn page_heading(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let section = self.shown;
        let scope = Scope::of(section);
        let palette = theme::palette(cx);
        let scope_id = gpui::SharedString::from("settings-page-scope");
        let scope_line = div()
            .id(gpui::ElementId::Name(scope_id.clone()))
            .role(Role::Label)
            .aria_label(scope.label())
            .flex_none()
            .flex()
            .items_center()
            .gap(px(5.))
            .text_size(px(11.))
            .text_color(theme::color(palette.fg_1))
            .child(
                svg()
                    .path(scope.icon().path())
                    .size(px(13.))
                    .text_color(theme::color(palette.fg_1)),
            )
            .child(scope.label());
        let scope_line = if section.has_live_choices(cx) {
            let help = HoverHelp::tooltip(
                "Changes from the command line or an agent show here right away.",
            );
            components::with_hover_help(scope_line, &scope_id, &help, true, None, window, cx)
        } else {
            scope_line
        };
        div()
            .relative()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(12.))
            .px(px(22.))
            .pt(px(14.))
            .pb(px(12.))
            .child(page_rule(&self.page_scroll, palette))
            .child(section.tile(true, false, palette))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    // Give the description the whole line, even where the window caps the dialog's width.
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap(px(8.))
                            .child(
                                div()
                                    .id("settings-heading")
                                    .debug_selector(|| "settings-heading".into())
                                    .role(Role::Heading)
                                    .aria_label(section.label())
                                    .aria_description(section.description())
                                    .text_size(px(16.))
                                    .line_height(px(20.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(section.label()),
                            )
                            .child(scope_line),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .line_height(px(16.))
                            .text_color(theme::color(palette.fg_1))
                            .truncate()
                            .child(section.description()),
                    ),
            )
            .into_any_element()
    }
}

/// Paint the heading's edge after layout clamps the page's scroll offset. A wheel event on a short page or a resize
/// that fits all the content can leave an out-of-range offset until the viewport is laid out.
fn page_rule(scroll: &ScrollHandle, palette: &theme::Palette) -> impl IntoElement {
    let scroll = scroll.clone();
    let border = theme::color(palette.border_dim);
    let shade = theme::color(palette.shadow_menu);
    let shadow = gpui::linear_gradient(
        180.,
        gpui::linear_color_stop(shade.opacity(0.5), 0.),
        gpui::linear_color_stop(shade.opacity(0.), 1.),
    );
    canvas(
        |_, _, _| (),
        move |bounds, (), window, _| {
            if scroll.offset().y < px(0.) {
                let mut below = bounds;
                below.origin.y += bounds.size.height;
                below.size.height = px(8.);
                window.paint_quad(gpui::fill(below, shadow));
                window.paint_quad(gpui::fill(bounds, border));
            }
        },
    )
    .absolute()
    .bottom_0()
    .left_0()
    .w_full()
    .h(px(1.))
}
