//! The launcher's side of Save as template (SPEC.md, "Launch templates"): the footer's button, opening the inline panel
//! on what the draft holds, closing it, and what a save does to the launcher. The panel itself, and what it saves, are
//! the templates area's (`crate::templates::save_panel`, `crate::templates::save`).
//!
//! While the panel is open it holds the launcher's Launch (`TEMPLATE_PANEL_OPEN` in the launcher's refusal), and while it
//! saves, Cancel, Close and Escape too.

use std::rc::Rc;

use farhelm_proto::split_wrapper_prefix;
use gpui::{
    App, AppContext as _, Context, Div, InteractiveElement as _, ParentElement as _, SharedString,
    Stateful, StatefulInteractiveElement as _, Styled as _, Window, div,
};

use super::{Launcher, host_name_with_phase, on_launcher};
use crate::components::{Button, HoverHelp, Role};
use crate::focus::Mark;
use crate::launcher::draft::{Mode, Tab};
use crate::launcher::request::capitalized;
use crate::layers;
use crate::templates::save::Candidate;
use crate::templates::save_panel::SavePanel;
use crate::theme::{self, metrics};

impl Launcher {
    /// Whether Save as template's save is in flight, when Cancel, Close and Escape hold still.
    pub(super) fn template_saving(&self, cx: &App) -> bool {
        self.save_panel
            .as_ref()
            .is_some_and(|panel| panel.read(cx).saving())
    }

    /// Save as template: open the inline panel on what the draft holds now, with focus in its name field (SPEC.md,
    /// "Launch templates"). Not while the panel is open or the YOLO question's host write is out. A draft whose Agent tab
    /// holds a retired container choice or a wrapper prefix that does not parse cannot be saved as it stands, and the
    /// launcher says why on a plain line instead, as the old one did.
    fn open_save_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.save_panel.is_some() || self.yolo_in_flight(cx) {
            return;
        }
        let mark = Mark::take(cx);
        if self.draft.tab == Tab::Agent {
            let refusal = if self.draft.agent.container.is_some() {
                Some(farhelm_proto::RETIRED_CONTAINER_REASON.to_owned())
            } else if self.draft.agent.wrapper.is_empty() {
                None
            } else {
                split_wrapper_prefix(&self.draft.agent.wrapper)
                    .err()
                    .map(|reason| capitalized(&reason))
            };
            if refusal.is_some() {
                self.save_refusal = refusal;
                self.pressed = false;
                cx.notify();
                return;
            }
        }
        let hosts = self.hosts(cx);
        let candidate = Candidate::from_draft(&self.draft, &hosts);
        let host_label = self.chosen_host(cx).map(host_name_with_phase);
        let (cancel, saved) = (cx.weak_entity(), cx.weak_entity());
        let panel = cx.new(|cx| {
            SavePanel::new(
                candidate,
                host_label,
                self.catalog.clone(),
                Rc::new(move |window, cx| {
                    let _ = cancel.update(cx, |launcher, cx| launcher.close_save_panel(window, cx));
                }),
                Rc::new(move |name, moved_on, window, cx| {
                    let _ = saved.update(cx, |launcher, cx| {
                        launcher.template_saved(&name, moved_on, window, cx)
                    });
                }),
                window,
                cx,
            )
        });
        let name = panel.read(cx).name_focus(cx);
        self.save_panel = Some(panel);
        // Template presses still waiting for the catalog read would edit the draft under the panel once it answers;
        // while the panel is open a press changes nothing (SPEC.md, "Quick launch"), so they are dropped, as a press
        // of Launch drops them.
        self.waiting_batches.clear();
        self.save_refusal = None;
        self.pressed = false;
        crate::focus::focus_in_layer(&name, &mark, window, cx);
        cx.notify();
    }

    /// Close the panel without saving (its Cancel, or Escape), and put focus back where the launcher starts it, as the
    /// old panel returned to the composer's search. Not while it saves.
    pub(super) fn close_save_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.template_saving(cx) {
            return;
        }
        if self.save_panel.take().is_some() {
            let start = self.start_focus(cx);
            crate::focus::focus_in_layer(&start, &Mark::take(cx), window, cx);
            cx.notify();
        }
    }

    /// The panel saved the template `name`: the launcher closes, its draft discarded with any edit made while the save
    /// was out, and Templates opens with the new template in its editor (SPEC.md: "Successful saving closes the launcher
    /// and opens the new template in the Templates editor"), unless the user moved on while it was saved (`moved_on`).
    /// Then the completion opens no dialog (SPEC.md, "Who may move focus": "Once the user has moved on, the result moves
    /// no focus, opens no dialog"); closing the launcher, its own subject, is the one thing it still does, and the
    /// template shows in the Templates list. The old launcher opened Templates either way (recorded in the parity file).
    fn template_saved(
        &mut self,
        name: &str,
        moved_on: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.save_panel = None;
        if let Some(layer) = self.layer.take() {
            layers::close(layer, window, cx);
        }
        // The store's flag is the moved-on gate; the mark only lets the dialog open now.
        if !moved_on {
            crate::templates::open_at(name, &Mark::take(cx), window, cx);
        }
    }

    /// Whether Save as template's panel is open, when the template shortcuts and the sequence prefix change nothing in
    /// this launcher (`crate::templates::dispatch`, `crate::templates::armed`).
    pub(crate) fn save_panel_open(&self) -> bool {
        self.save_panel.is_some()
    }

    /// The open Save as template panel, for tests that look at its own fields.
    #[cfg(test)]
    pub(crate) fn save_panel_for_tests(&self) -> Option<gpui::Entity<SavePanel>> {
        self.save_panel.clone()
    }

    /// Why Save as template did not open, for the notices region: a plain line in the warning ink, not a refused
    /// launch.
    pub(super) fn save_refusal_line(&self, palette: &theme::Palette) -> Option<Stateful<Div>> {
        self.save_refusal.clone().map(|reason| {
            div()
                .id("launcher-save-template-refusal")
                .debug_selector(|| "launcher-save-template-refusal".into())
                .role(Role::Alert)
                .aria_label(SharedString::from(reason.clone()))
                .text_size(metrics::TEXT_SM)
                .text_color(theme::color(palette.warn_bright))
                .child(reason)
        })
    }

    /// The footer's Save as template button, after Reset choices and before the main action (SPEC.md: "Cancel comes
    /// first in the action row, then Reset choices where offered, Save as template where offered, and the main action
    /// last"). Replace with does not offer it, and neither does a launch on a host a disposable environment owns, as the
    /// old footer left it out there: that host's identity names the environment, not a place a template can send a later
    /// launch. Busy while its panel is open, as the old button was disabled then, and while the YOLO question's host
    /// write is out (`holding`).
    pub(super) fn save_template_button(
        &self,
        holding: bool,
        cx: &mut Context<Self>,
    ) -> Option<Button> {
        let environment_host = self
            .chosen_host(cx)
            .is_some_and(|host| host.is_session_owned);
        if matches!(self.draft.mode, Mode::ReplaceWith { .. }) || environment_host {
            return None;
        }
        Some(
            Button::new(
                "launcher-save-template",
                "Save as template",
                HoverHelp::tooltip(
                    "Save as template: keep choices from this setup for another launch",
                ),
                Role::Button,
            )
            .busy(holding || self.save_panel.is_some())
            .on_click(on_launcher(cx.weak_entity(), Launcher::open_save_panel)),
        )
    }
}
