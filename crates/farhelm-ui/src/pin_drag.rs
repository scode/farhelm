//! The mounted list owns drag intents; the helm still owns pin order.
//!
//! JavaScript handles capture and temporary paint because pointer geometry
//! belongs to the DOM. Its channel lives only as long as the actual list root,
//! which can disappear and recover while the surrounding ListView stays alive.
//! Receiving an intent checks the existing displayed eligibility projection,
//! then uses the same move callback as the row menu without changing its target.

use dioxus::prelude::*;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::{Session, rows};

static NEXT_MOUNT: AtomicU64 = AtomicU64::new(1);

/// A drop names a displayed pin, never an index into the helm's full order.
#[derive(Debug, serde::Deserialize)]
pub(crate) struct Intent {
    pub(crate) id: String,
    pub(crate) neighbor: String,
    pub(crate) before: bool,
}

impl Intent {
    /// Revalidate membership without rewriting the user's captured placement.
    ///
    /// A concurrent display update may change adjacency. Both pins must still
    /// be eligible, but their new adjacency does not invalidate a relative move.
    /// Source operation locks are checked by the list, which owns those locks.
    pub(crate) fn is_eligible(&self, held: Option<&rows::HeldOrder>, shown: &[Session]) -> bool {
        let eligible = rows::pin_move_neighbors(held, shown);
        self.id != self.neighbor
            && eligible.contains_key(&self.id)
            && eligible.contains_key(&self.neighbor)
    }
}

/// Retire the named mount even if its asset has not loaded yet.
///
/// Native eval cleanup can run before the DOM removal. Clearing this existing
/// marker makes the waiting snippet stand down in that gap; disconnection
/// alone is not yet observable, and a missing asset cannot cancel setup that
/// is still waiting for it.
fn release_js(token: &str) -> String {
    let token = serde_json::to_string(token).expect("a string is serializable");
    format!(
        r#"const token = {token};
        const root = [...document.querySelectorAll('[data-pin-drag-mount]')]
            .find(node => node.dataset.pinDragMount === token);
        if (root) delete root.dataset.pinDragMount;
        window.farhelmPinDrag?.release(token);"#
    )
}

/// Render the list root in the same lifetime that owns its pointer bridge.
///
/// ListView replaces this whole child when a listing fails or the fleet is
/// empty. Keeping the hook here gives each recovered DOM root a fresh token;
/// a token owned by ListView itself would survive those root replacements and
/// let the predecessor's late channel cleanup release its successor.
/// Pointer activity still goes to ListView's existing order-hold owner.
#[component]
pub(crate) fn PinDragList(
    hidden: bool,
    on_move: EventHandler<Intent>,
    on_activity: EventHandler<()>,
    on_leave: EventHandler<()>,
    children: Element,
) -> Element {
    let (token, mount) = use_pin_drag(on_move, hidden);
    rsx! {
        div {
            class: "session-list",
            hidden,
            "data-pin-drag-mount": token,
            onmounted: move |_| mount.call(()),
            onpointerenter: move |_| on_activity.call(()),
            onpointermove: move |_| on_activity.call(()),
            onpointerdown: move |_| on_activity.call(()),
            onwheel: move |_| on_activity.call(()),
            onpointerleave: move |_| on_leave.call(()),
            {children}
        }
    }
}

/// Tie bridge cleanup to one actual root, including asset-loading races.
///
/// Call the returned callback from the list's mounted event. The snippet finds
/// that mount before waiting for the asynchronously injected asset and refuses
/// to subscribe if the node has gone meanwhile. Its awaited subscription keeps
/// Dioxus from closing the send channel after listener installation (see
/// `feed::FleetFeed`). Cleanup names this mount so a delayed old eval cannot
/// release its replacement. Hiding the list cancels a gesture but keeps its
/// subscription available when the same list becomes visible again.
fn use_pin_drag(on_move: Callback<Intent>, hidden: bool) -> (String, Callback<()>) {
    let token = use_hook(|| NEXT_MOUNT.fetch_add(1, Ordering::Relaxed).to_string());
    let release_token = token.clone();
    use_drop(move || {
        document::eval(&release_js(&release_token));
    });
    let cancel_token = token.clone();
    use_effect(use_reactive!(|(hidden)| {
        if hidden {
            document::eval(&format!(
                "window.farhelmPinDrag?.cancel({});",
                serde_json::to_string(&cancel_token).expect("a string is serializable")
            ));
        }
    }));
    let mount_token = token.clone();
    let mount = use_callback(move |()| {
        let token = serde_json::to_string(&mount_token).expect("a string is serializable");
        let cleanup = release_js(&mount_token);
        spawn(async move {
            let mut channel = document::eval(&format!(
                r#"const token = {token};
                const root = [...document.querySelectorAll('[data-pin-drag-mount]')]
                    .find(node => node.dataset.pinDragMount === token);
                if (!root) return;
                while (!window.farhelmPinDrag) {{
                    if (!root.isConnected || root.dataset.pinDragMount !== token) return;
                    await new Promise(resolve => setTimeout(resolve, 50));
                }}
                if (!root.isConnected || root.dataset.pinDragMount !== token) return;
                await window.farhelmPinDrag.subscribe(root, intent => dioxus.send(intent), token);"#
            ));
            while let Ok(intent) = channel.recv::<Intent>().await {
                on_move.call(intent);
            }
            // A dead bridge cannot retain a live-looking grip subscription.
            document::eval(&cleanup);
        });
    });
    (token, mount)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    /// Extract the real bridge marker emitted when its list root is created.
    /// A missing marker fails the fixture premise rather than guessing an ID.
    fn mounted_token(edits: &[dioxus::core::Mutation]) -> String {
        edits
            .iter()
            .find_map(|edit| match edit {
                dioxus::core::Mutation::SetAttribute {
                    name: "data-pin-drag-mount",
                    value: dioxus::core::AttributeValue::Text(token),
                    ..
                } => Some(token.clone()),
                _ => None,
            })
            .expect("the actual list root must carry its bridge token")
    }

    /// Error/recovery replaces the list root while its parent stays mounted.
    /// Its bridge identity must follow that root lifetime, otherwise the old
    /// eval's cleanup releases the recovered list. The JS ownership regression
    /// separately proves that this fresh token still delivers a successor move.
    #[farhelm_testtrace::test]
    fn pin_drag_list_recovery_replaces_owner_inside_the_same_parent() {
        thread_local! {
            static LIST_AVAILABLE: Cell<bool> = const { Cell::new(true) };
            static PARENT_MOUNTS: Cell<usize> = const { Cell::new(0) };
        }
        fn app() -> Element {
            use_hook(|| PARENT_MOUNTS.with(|mounts| mounts.set(mounts.get() + 1)));
            if LIST_AVAILABLE.with(Cell::get) {
                rsx! {
                    PinDragList {
                        hidden: false,
                        on_move: |_: Intent| {},
                        on_activity: |()| {},
                        on_leave: |()| {},
                        div { "A listed row" }
                    }
                }
            } else {
                rsx! { div { "Listing unavailable" } }
            }
        }

        LIST_AVAILABLE.with(|available| available.set(true));
        PARENT_MOUNTS.with(|mounts| mounts.set(0));
        let mut dom = VirtualDom::new(app);
        let first = mounted_token(&dom.rebuild_to_vec().edits);
        LIST_AVAILABLE.with(|available| available.set(false));
        dom.mark_dirty(dioxus::core::ScopeId::APP);
        dom.render_immediate_to_vec();
        LIST_AVAILABLE.with(|available| available.set(true));
        dom.mark_dirty(dioxus::core::ScopeId::APP);
        let recovered = mounted_token(&dom.render_immediate_to_vec().edits);
        assert_ne!(
            first, recovered,
            "root replacement must replace its bridge owner"
        );
        PARENT_MOUNTS
            .with(|mounts| assert_eq!(mounts.get(), 1, "the parent must survive recovery"));
    }
}
