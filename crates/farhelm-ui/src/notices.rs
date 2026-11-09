//! Shared non-session notices use the existing reader and mutation boundaries.
//!
//! ListView owns this reader permanently and supplies its ordinary feed and
//! fallback triggers. The bell owns only its opening and exact shown IDs;
//! closing cannot acknowledge a later item. Marks follow the session bells'
//! best-effort policy: the shared acknowledgement quiets the bell, and a failed
//! mark can be sent again on another explicit close. No write retry or poll.

use std::collections::HashSet;

use dioxus::prelude::*;

use crate::ApiBase;
use crate::api::{self, HelmNotice, NoticeMark};
use crate::ops::ReadGate;
use crate::reader::{SurfaceReader, Trigger, request_read};

/// A failed refresh retains the last successful history. Only a successful
/// snapshot establishes absence; an acknowledged mark changes only its IDs.
#[derive(Default)]
pub(crate) struct NoticeSnapshot {
    pub(crate) notices: Vec<HelmNotice>,
    pub(crate) loaded: bool,
    pub(crate) error: Option<String>,
}

impl NoticeSnapshot {
    /// Apply only the IDs the helm acknowledged, never a newly arrived item.
    /// The caller fences older GETs before this local projection is published.
    fn acknowledge_read(&mut self, ids: &[i64]) {
        let shown: HashSet<_> = ids.iter().copied().collect();
        for notice in &mut self.notices {
            if shown.contains(&notice.id) {
                notice.read = true;
            }
        }
    }

    /// The helm owns shared history and marks; no local tombstone outlives a
    /// successful list or suppresses a genuinely later notice with a new ID.
    fn observe(&mut self, notices: Vec<HelmNotice>) {
        self.notices = notices;
        self.loaded = true;
        self.error = None;
    }

    /// Resolved entries are quiet independently of any client's read history.
    pub(crate) fn unread(&self) -> usize {
        self.notices.iter().filter(|notice| notice.unread()).count()
    }
}

/// Persistent ListView-owned operations, shared with the footer's bell.
/// Identity is the snapshot signal; every handle is created once by its hook.
#[derive(Clone, Copy)]
pub(crate) struct NoticeReader {
    pub(crate) state: Signal<NoticeSnapshot>,
    pub(crate) request: Callback<Trigger>,
    pub(crate) mark_read: Callback<Vec<i64>>,
    pub(crate) clear: Callback<Vec<i64>>,
    pub(crate) clearing: Signal<bool>,
    pub(crate) action_error: Signal<Option<String>>,
}

impl PartialEq for NoticeReader {
    fn eq(&self, other: &Self) -> bool {
        self.state == other.state
    }
}

/// Mount one retrying snapshot reader, with no independent feed or timer.
/// ReadGate also fences GETs that predate an acknowledged mark/clear, since
/// SurfaceReader alone bounds requests but does not order them with writes.
pub(crate) fn use_notice_reader() -> NoticeReader {
    let base = use_context::<ApiBase>().0;
    let mut state = use_signal(NoticeSnapshot::default);
    let reader = use_signal(SurfaceReader::default);
    let mut read_gate = use_signal(ReadGate::default);
    let mut clearing = use_signal(|| false);
    let mut action_error = use_signal(|| None::<String>);
    let read_base = base.clone();
    let request = use_callback(move |trigger: Trigger| {
        let base = read_base.clone();
        request_read(reader, trigger, move || {
            let base = base.clone();
            let generation = read_gate.write().start();
            async move {
                let result = api::fetch_notices(&base).await;
                let answered = result.is_ok();
                match result {
                    Ok(notices) => {
                        if read_gate.write().accept_success(generation) {
                            state.write().observe(notices);
                        }
                    }
                    Err(error) => {
                        if read_gate.peek().accept_failure(generation) {
                            state.write().error = Some(error);
                        }
                    }
                }
                answered
            }
        });
    });
    use_effect(move || request.call(Trigger::Explicit));

    let mark_base = base.clone();
    let mark_read = use_callback(move |ids: Vec<i64>| {
        if ids.is_empty() {
            return;
        }
        let base = mark_base.clone();
        spawn(async move {
            match api::mark_notices(&base, NoticeMark::Read, &ids).await {
                Ok(()) => {
                    read_gate.write().fence();
                    state.write().acknowledge_read(&ids);
                    request.call(Trigger::Explicit);
                }
                Err(error) => {
                    // As with session bells, a lost mark can leave the bell
                    // loud until its next close. No local read overlay or
                    // automatic write retry masks that shared outcome.
                    dioxus::logger::tracing::warn!(target: "notices", %error, "could not mark shown notices read");
                }
            }
        });
    });

    let clear = use_callback(move |ids: Vec<i64>| {
        if ids.is_empty() || *clearing.peek() {
            return;
        }
        clearing.set(true);
        action_error.set(None);
        let base = base.clone();
        spawn(async move {
            match api::mark_notices(&base, NoticeMark::ClearResolved, &ids).await {
                Ok(()) => {
                    // Re-read the server's actual disposition: a condition
                    // could have reopened while this clear was in flight.
                    read_gate.write().fence();
                    request.call(Trigger::Explicit);
                }
                Err(error) => action_error.set(Some(error)),
            }
            clearing.set(false);
        });
    });
    NoticeReader {
        state,
        request,
        mark_read,
        clear,
        clearing,
        action_error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::NoticeContent;

    /// Real notice identities matter: a cleared condition's later new ID must
    /// not inherit a local acknowledgement of its older history.
    fn notice(id: i64) -> HelmNotice {
        HelmNotice {
            id,
            host: 1,
            raised_at: 1_700_000_000,
            read: false,
            resolved: false,
            content: NoticeContent::StuckTrash {
                checkout_id: format!("checkout-{id}"),
                name: "archive".into(),
                repository: "fixture/repo".into(),
                bytes: Some(4096),
                reason: "permission denied".into(),
                at: 1_700_000_000,
            },
        }
    }

    /// A successful close-time mark acknowledges only shown IDs, even when a
    /// new item arrives before its reply. Clearing old history cannot give a
    /// genuinely later notice the old acknowledgement.
    #[farhelm_testtrace::test]
    fn acknowledged_shown_ids_do_not_acknowledge_arrivals() {
        let mut state = NoticeSnapshot::default();
        state.observe(vec![notice(1)]);
        assert_eq!(state.unread(), 1);
        state.observe(vec![notice(2), notice(1)]);
        state.acknowledge_read(&[1]);
        assert_eq!(state.unread(), 1);
        assert!(state.notices[1].read);
        let mut confirmed = notice(1);
        confirmed.read = true;
        state.observe(vec![notice(2), confirmed]);
        assert_eq!(state.unread(), 1);
        state.acknowledge_read(&[2]);
        assert_eq!(state.unread(), 0);
        state.observe(vec![]);
        state.observe(vec![notice(3)]);
        assert_eq!(state.unread(), 1);
    }

    /// Resolution suppresses alerting even when nobody opened the panel;
    /// its shared read field need not be true to suppress that alert.
    #[farhelm_testtrace::test]
    fn resolved_items_are_quiet_without_a_read_mark() {
        let mut resolved = notice(1);
        resolved.resolved = true;
        let mut state = NoticeSnapshot::default();
        state.observe(vec![resolved]);
        assert_eq!(state.unread(), 0);
    }
}
