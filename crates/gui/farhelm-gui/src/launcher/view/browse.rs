//! Browse: list the folders one level under the folder as typed, on the chosen host, and take one as the folder.
//!
//! SPEC.md, "Recent setups" ("explicit browsing asks the selected supervisor for one bounded directory level") and
//! "Working directory" ("A directory picker/completer against the target host's filesystem is provided"). Ported from
//! the old launcher's Browse button and `.launch-composer-browser` listing (`list/create_form.rs` and
//! `list/create_form/draft.rs` at reference commit `24bc0580`). Here's the TLDR:
//!
//! - **Asking.** The Browse button under the Host row, or the search's "Browse this path" row, sends
//!   `POST /api/browse-directory` for the folder as typed, on the chosen host's connection. Typing never reaches the
//!   host. The supervisor answers one level, bounded (at most 200 folders, said as `truncated`); a host that cannot be
//!   asked answers with its words, shown in place of the listing.
//! - **The listing** sits in the launcher's body, after the choices, as the old one did: "Use" the folder the host
//!   resolved, its parent, a line when there are more folders than listed, then one button per folder. Every button
//!   takes its folder as the user's own choice and closes the listing; nothing descends, so going deeper is Browse
//!   again on the taken folder, as before.
//! - **Latest wins.** A listing is about the host, the connection and the folder it was asked for; any change to them
//!   (an edit, a host change, a reconnect) forgets it and its request, so a late answer is dropped and an undone edit
//!   does not bring the listing back. GPUI runs the
//!   handlers in order against the current draft, so the old click-time recheck and its counters do not come back.
//! - **Not offered** where the folder cannot change or is not a folder: a managed checkout, and Restart with, which
//!   holds its destination.

use farhelm_helm_client::{ApiPath, Method};
use farhelm_proto::session_requests::{BrowseDirectoryBody, BrowseDirectoryReply};
use farhelm_proto::ui_channel::HostView;
use gpui::{
    AnyElement, App, Context, InteractiveElement as _, IntoElement, ParentElement as _,
    SharedString, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};

use super::{Launcher, on_launcher};
use crate::components::{Button, HoverHelp, Role, Tier};
use crate::host_text::{HostSentence, HostText};
use crate::launcher::draft::Field;
use crate::operations;
use crate::theme::{self, metrics};

/// Browse's words from inside its edge: the old `padding: 0 14px` (CSS padding sits inside the border).
const BROWSE_PAD_X: f32 = 14.;

/// What one listing is about: the host, the connection the launcher saw it on, and the folder as typed.
#[derive(Clone, Debug, PartialEq, Eq)]
struct BrowseKey {
    host: i64,
    incarnation: u64,
    cwd: String,
}

/// The launcher's browse: what it last asked about, and the answer for it.
#[derive(Default)]
pub(super) struct Browse {
    /// The key the last request is about; an answer for any other is dropped.
    asked: Option<BrowseKey>,
    /// The last answer, with its key: the listing, or the host's words refusing it.
    answered: Option<(BrowseKey, Result<BrowseDirectoryReply, HostText>)>,
}

impl Launcher {
    /// Whether the launcher offers Browse: a folder destination this draft may change.
    pub(super) fn offers_browse(&self) -> bool {
        self.draft.checkout.is_none() && !self.draft.holds(Field::Destination)
    }

    /// What a browse would be about now: the chosen host on the connection the registry lists, and the folder as it
    /// stands. A host that is not connected is asked anyway, as the old launcher did: the helm answers with why not.
    fn browse_key(&self, cx: &App) -> Option<BrowseKey> {
        if !self.offers_browse() {
            return None;
        }
        let host = self.draft.host?;
        let view = self
            .store
            .read(cx)
            .collections()
            .ordered::<HostView>()
            .into_iter()
            .find(|view| view.id == host)?;
        Some(BrowseKey {
            host,
            incarnation: view.incarnation,
            cwd: self.draft.folder.value().clone(),
        })
    }

    /// Browse the folder as it stands: ask the host for one level under it (see the module docs).
    pub(super) fn browse(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        let Some(key) = self.browse_key(cx) else {
            return;
        };
        let Ok(path) = ApiPath::api(&["browse-directory"]) else {
            return;
        };
        let body = BrowseDirectoryBody {
            host: key.host,
            cwd: key.cwd.clone(),
            expected_incarnation: Some(key.incarnation),
            membership_only: false,
        };
        let Ok(body) = serde_json::to_value(&body) else {
            return;
        };
        self.browse.asked = Some(key.clone());
        self.browse.answered = None;
        let launcher = cx.weak_entity();
        operations::send(
            Method::Post,
            path,
            Some(body),
            "POST /api/browse-directory",
            cx,
            move |answer, cx| {
                let listing = match answer {
                    Ok(body) => body
                        .and_then(|body| serde_json::from_value::<BrowseDirectoryReply>(body).ok())
                        .ok_or_else(|| HostText::from_host("the host's listing could not be read")),
                    Err(words) => Err(words),
                };
                let _ = launcher.update(cx, |launcher, cx| {
                    if launcher.browse.asked.as_ref() == Some(&key) {
                        launcher.browse.answered = Some((key, listing));
                        cx.notify();
                    }
                });
            },
        );
        cx.notify();
    }

    /// Forget a request or listing that is no longer about the host, connection and folder as they stand, so a late
    /// answer for it is dropped and an earlier listing never comes back when the user returns to its folder. Called
    /// while drawing, since a connection changes by a push with no edit of the launcher's.
    pub(super) fn follow_browse(&mut self, cx: &App) {
        if self.browse.asked.is_some() && self.browse.asked != self.browse_key(cx) {
            self.browse = Browse::default();
        }
    }

    /// Take `folder` from the listing as the folder, the user's own choice, and close the listing.
    fn take_browsed(&mut self, folder: String, window: &mut Window, cx: &mut Context<Self>) {
        self.browse = Browse::default();
        self.folder_picked();
        self.draft.edit_folder(folder);
        self.sync_fields(window, cx);
        self.changed(cx);
    }

    /// The Browse button, under the Host row ([`Launcher::with_browse_below`]), while the launcher offers it; greyed
    /// while no host is chosen. A press only reads, so it never waits on the YOLO question's host write. It is as tall
    /// as the host button (the old `min-height: 36px`) and 14 pixels in at the sides (`padding: 0 14px`).
    pub(super) fn browse_button(&self, cx: &mut Context<Self>) -> Option<Button> {
        if !self.offers_browse() {
            return None;
        }
        Some(
            Button::new(
                "launcher-browse",
                "Browse this path",
                HoverHelp::tooltip("Browse: list the folders under this path on the host"),
                Role::Button,
            )
            .content(
                div()
                    .flex()
                    .items_center()
                    .h(super::host_picker::control_content_height())
                    .px(px(BROWSE_PAD_X) - metrics::BUTTON_PAD.1)
                    // `.launch-composer-destination-row > button`: `font-size: 13px`.
                    .text_size(metrics::TEXT_MD)
                    .child("Browse"),
            )
            .tier(Tier::Neutral)
            .disabled(self.draft.host.is_none())
            .on_click(on_launcher(cx.weak_entity(), Launcher::browse)),
        )
    }

    /// The listing, or the host's words refusing it, while the last answer is about the host, connection and folder as
    /// they stand.
    pub(super) fn browse_listing(
        &self,
        palette: &theme::Palette,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let key = self.browse_key(cx)?;
        let (_, answer) = self
            .browse
            .answered
            .as_ref()
            .filter(|(answered, _)| *answered == key)?;
        let listing = match answer {
            Err(words) => {
                // The host's words, escaped, in the old error line's warning ink.
                let sentence = HostSentence::new("launcher-browse-error-words").host(words);
                return Some(
                    div()
                        .id("launcher-browse-error")
                        .role(Role::Alert)
                        .aria_label(sentence.accessible_name())
                        .text_size(metrics::TEXT_SM)
                        .text_color(theme::color(palette.warn_bright))
                        .child(sentence)
                        .into_any_element(),
                );
            }
            Ok(listing) => listing,
        };
        let mut rows = div()
            .id("launcher-browse-list")
            // Tests find the listing by where it is drawn; this records nothing outside test builds.
            .debug_selector(|| "launcher-browse-list".into())
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(self.browse_choice(
                "launcher-browse-use".into(),
                "Use ",
                &listing.cwd,
                "Use this folder: the session will start here",
                cx,
            ));
        if let Some(parent) = &listing.parent {
            rows = rows.child(self.browse_choice(
                "launcher-browse-parent".into(),
                "Parent: ",
                parent,
                "Use the parent folder instead",
                cx,
            ));
        }
        if listing.truncated {
            rows = rows.child(
                div()
                    .id("launcher-browse-truncated")
                    .role(Role::Label)
                    .aria_label("More directories exist; refine the path and browse again")
                    .text_size(metrics::TEXT_SM)
                    .text_color(theme::color(palette.fg_1))
                    .child("More directories exist; refine the path and browse again"),
            );
        }
        for (index, child) in listing.children.iter().enumerate() {
            rows = rows.child(self.browse_choice(
                SharedString::from(format!("launcher-browse-{index}")),
                "",
                child,
                "Use this folder: the session will start here",
                cx,
            ));
        }
        Some(rows.into_any_element())
    }

    /// One of the listing's buttons: `lead` then `folder` (host text, in the terminal font), taking `folder` when
    /// pressed. The folder is the host's exact spelling, which the press takes, while the button draws it escaped.
    fn browse_choice(
        &self,
        id: SharedString,
        lead: &'static str,
        folder: &str,
        help: &'static str,
        cx: &mut Context<Self>,
    ) -> Button {
        let words = HostSentence::new(SharedString::from(format!("{id}-words")))
            .text(lead)
            .mono(&HostText::from_host(folder));
        let taken = folder.to_owned();
        let launcher = cx.weak_entity();
        Button::new(
            id,
            words.accessible_name(),
            HoverHelp::tooltip(help),
            Role::Button,
        )
        .content(words)
        .tier(Tier::BrowseRow)
        .on_click(move |_, window, cx| {
            let folder = taken.clone();
            let _ = launcher.update(cx, |launcher, cx| launcher.take_browsed(folder, window, cx));
        })
    }
}
