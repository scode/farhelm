//! The sidebar's hover summary, rendered once per row as an inert DOM source.
//!
//! Rust already owns the host mark, status vocabulary and peer-text isolation.
//! Keeping this markup here lets the shared tooltip copy it without asking
//! JavaScript to reconstruct those rules. The source is a sibling of the row:
//! putting it inside the row would add hidden matches to row-scoped controls
//! and browser locators, and putting it inside the open button would be invalid
//! button content. Only the source's children are cloned, so its id stays unique.

use dioxus::prelude::*;
use farhelm_proto::host_appearance::{HostColor, HostIcon};

use crate::Session;
use crate::activity::{ActivityStamp, utc_stamp};
use crate::peer::{DetailPart, PeerLine};
use crate::status::StatusBadge;

use super::row::{PeerTitle, menu_header_summary_parts, observed_yolo_mode};
use super::shared::{HostLocality, SessionHostIcon, SessionHostName};

/// Keep the saved launch visible while explaining the shield's screen evidence.
/// The menu deliberately uses only the saved facts. Even an already-YOLO launch
/// gets the suffix here: observing a mode and choosing it at launch are distinct.
fn summary_parts(session: &Session) -> Vec<DetailPart> {
    let mut parts = menu_header_summary_parts(session, None);
    if observed_yolo_mode(session.launch.as_ref(), session.screen_yolo).is_some() {
        parts.push(DetailPart::text(", YOLO observed"));
    }
    parts
}

/// Provide the one summary used by every unmarked point in a session's open button.
///
/// The hidden source is outside the accessibility tree. The button keeps its
/// existing accessible name, while the body-level tooltip remains visual and
/// `aria-hidden`; copying this card into it does not make a second reading of
/// the same title, status and directory for a screen reader.
#[component]
pub(super) fn SessionSummarySource(
    session: Session,
    locality: HostLocality,
    host_icon: HostIcon,
    host_color: HostColor,
    activity: Option<ActivityStamp>,
    badge: Option<StatusBadge>,
) -> Element {
    // The row's tooltip targets this exact DOM identity. Escaping it as visible
    // text would disconnect the source from the row that owns it.
    let source_id = format!("session-summary-{}", session.id.raw());
    let card_class = if session.stale {
        "session-summary stale"
    } else {
        "session-summary"
    };
    let host_known = session.host_name.is_some() || locality != HostLocality::Unknown;
    let checkout = session.working_copy.as_ref().map(|copy| {
        // Choose the basename from the actual path before PeerLine escapes it;
        // display notation is not part of the filesystem spelling.
        let folder = copy
            .canonical_path
            .raw()
            .rsplit('/')
            .find(|segment| !segment.is_empty())
            .unwrap_or(copy.canonical_path.raw());
        vec![
            DetailPart::peer(copy.repo.identifier()),
            DetailPart::text(" · "),
            DetailPart::mono_peer(folder.to_string()),
        ]
    });
    let started = (session.created_at > 0).then(|| utc_stamp(session.created_at));
    let last_active = activity.as_ref().map(|stamp| {
        format!(
            "{} · {}",
            stamp.age,
            utc_stamp(session.effective_activity())
        )
    });
    let status = badge.map(|badge| {
        let word = if badge.visible {
            badge.label().to_string()
        } else {
            badge.text
        };
        (
            format!("session-summary-status status-badge {}", badge.class),
            word,
            !badge.visible,
        )
    });

    rsx! {
        div { id: "{source_id}", class: "session-summary-source", hidden: true,
            div { class: card_class,
                div { class: "session-summary-header",
                    div { class: "session-summary-heading",
                        PeerTitle {
                            class: "session-summary-title",
                            title: session.title.clone(),
                            quoted: false,
                        }
                        if let Some((class, word, has_dot)) = &status {
                            span { class: class.as_str(),
                                if *has_dot {
                                    span { class: "status-dot", aria_hidden: "true" }
                                }
                                span { class: "session-summary-state-word", "{word}" }
                            }
                        }
                    }
                    PeerLine {
                        class: "session-summary-agent".to_string(),
                        parts: summary_parts(&session),
                    }
                }
                div { class: "session-summary-body",
                    if host_known {
                        div { class: "session-summary-row",
                            span { class: "session-summary-label", "Host" }
                            div { class: "session-summary-value session-summary-host",
                                SessionHostIcon { locality, icon: host_icon, color: host_color }
                                if let Some(name) = &session.host_name {
                                    SessionHostName { name: name.clone(), locality, tooltip: false }
                                }
                            }
                        }
                    }
                    if let Some(checkout) = checkout {
                        div { class: "session-summary-row",
                            span { class: "session-summary-label", "Checkout" }
                            PeerLine { class: "session-summary-value".to_string(), parts: checkout }
                        }
                    }
                    if !session.cwd.raw().is_empty() {
                        div { class: "session-summary-row",
                            span { class: "session-summary-label", "Directory" }
                            PeerLine {
                                class: "session-summary-value session-summary-path".to_string(),
                                parts: vec![DetailPart::peer(session.cwd.clone())],
                            }
                        }
                    }
                    if let Some(started) = started {
                        div { class: "session-summary-row",
                            span { class: "session-summary-label", "Started" }
                            span { class: "session-summary-value font-mono", "{started}" }
                        }
                    }
                    if let Some(last_active) = last_active {
                        div { class: "session-summary-row",
                            span { class: "session-summary-label", "Last active" }
                            span { class: "session-summary-value font-mono", "{last_active}" }
                        }
                    }
                }
                div { class: "session-summary-footer",
                    span { "Click to open" }
                    span { "⋯ For actions" }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use dioxus::core::Mutation;

    use super::*;
    use crate::SessionStatus;
    use crate::github_checkout::{GithubRepo, WorkingCopyInfo};
    use crate::list::row::row_specimen;
    use crate::status::status_badge;

    std::thread_local! {
        static SPECIMEN: RefCell<Option<(Session, HostLocality, Option<ActivityStamp>)>> = const { RefCell::new(None) };
    }

    /// Render the real source component and retain the browser-facing text
    /// and dynamic attributes. This catches omissions in the Dioxus markup
    /// that a pure formatting helper could not see.
    fn render(
        session: Session,
        locality: HostLocality,
        activity: Option<ActivityStamp>,
    ) -> Vec<Mutation> {
        fn app() -> Element {
            let (session, locality, activity) =
                SPECIMEN.with(|cell| cell.borrow().clone().unwrap());
            let badge = status_badge(
                &session.status,
                session.annotation.as_ref().map(crate::peer::PeerText::raw),
                session.has_unseen_output(),
            );
            rsx! {
                SessionSummarySource {
                    session,
                    locality,
                    host_icon: HostIcon::default(),
                    host_color: HostColor::default(),
                    activity,
                    badge,
                }
            }
        }
        SPECIMEN.with(|cell| *cell.borrow_mut() = Some((session, locality, activity)));
        VirtualDom::new(app).rebuild_to_vec().edits
    }

    /// Dynamic values must be present without inventing a host or timestamp
    /// for a legacy session that never sent them. Static grid labels are
    /// covered by the browser test, since Dioxus templates do not emit them
    /// as text mutations. Managed checkouts retain the full directory.
    #[farhelm_testtrace::test]
    fn card_rows_follow_available_session_facts() {
        let legacy = render(row_specimen("legacy"), HostLocality::Unknown, None);
        let legacy_text = text_values(&legacy);
        assert!(legacy_text.contains(&"/tmp".to_string()));

        let mut managed = row_specimen("managed");
        managed.cwd = "/srv/checkouts/project-2/src".into();
        managed.created_at = 1_700_000_000;
        managed.last_activity_at = 1_700_000_000;
        managed.host_name = Some("build-host".into());
        managed.working_copy = Some(WorkingCopyInfo {
            id: "checkout".into(),
            repo: GithubRepo {
                owner: "owner".into(),
                name: "project".into(),
            },
            canonical_path: "/srv/checkouts/project-2".into(),
            origin_session_id: "managed".into(),
        });
        let activity = ActivityStamp::new(Some(1_700_000_125), 1_700_000_000);
        let managed_text = text_values(&render(managed, HostLocality::Remote, activity));
        for expected in [
            "build-host",
            "owner/project",
            "project-2",
            "/srv/checkouts/project-2/src",
            "2023-11-14 22:13:20 UTC",
            "2m · 2023-11-14 22:13:20 UTC",
        ] {
            assert!(
                managed_text.contains(&expected.to_string()),
                "missing {expected}: {managed_text:?}"
            );
        }
    }

    /// Checkout summaries select the raw basename, then escape that peer run.
    /// A trailing slash must not discard the real folder or expose its controls.
    #[farhelm_testtrace::test]
    fn checkout_summary_escapes_the_raw_basename() {
        let mut session = row_specimen("managed");
        session.working_copy = Some(WorkingCopyInfo {
            id: "checkout".into(),
            repo: GithubRepo::parse("owner/project").unwrap(),
            canonical_path: "/srv/checkouts/project\u{202e}/".into(),
            origin_session_id: "managed".into(),
        });
        let shown = text_values(&render(session, HostLocality::Unknown, None));
        assert!(shown.contains(&"project<U+202E>".to_string()), "{shown:?}");
        assert!(
            shown.iter().all(|text| !text.contains('\u{202e}')),
            "{shown:?}"
        );
    }

    /// A stored guarded launch must not conceal screen evidence in the real card.
    /// The menu remains a launch record, and unsupported readings add no claim.
    #[farhelm_testtrace::test]
    fn card_mentions_observed_yolo_without_changing_the_menu() {
        let mut session = row_specimen("observed");
        session.launch = Some(crate::SessionLaunch::Agent {
            selection: crate::LaunchSelection {
                harness: crate::LaunchHarness::Claude,
                permissions: Some(crate::LaunchPermission::Guarded),
                model: None,
                effort: None,
                permission_flags: None,
                wrapper_prefix: None,
                workspace_trust: None,
            },
            start: vec!["claude".into(), "{farhelm_args}".into()],
            resume: None,
        });
        assert!(!session.screen_yolo);
        assert_eq!(
            crate::peer::detail_text(&summary_parts(&session)),
            "Claude · Guarded"
        );
        session.screen_yolo = true;
        assert_eq!(
            crate::peer::detail_text(&summary_parts(&session)),
            "Claude · Guarded, YOLO observed"
        );
        let rendered = text_values(&render(session.clone(), HostLocality::Unknown, None)).join("");
        assert!(rendered.contains(", YOLO observed"), "{rendered}");
        assert_eq!(
            crate::peer::detail_text(&menu_header_summary_parts(&session, None)),
            "Claude · Guarded"
        );
        if let Some(crate::SessionLaunch::Agent { selection, .. }) = &mut session.launch {
            selection.permissions = Some(crate::LaunchPermission::Yolo);
        }
        assert_eq!(
            crate::peer::detail_text(&summary_parts(&session)),
            "Claude · YOLO, YOLO observed"
        );
        session.launch = None;
        assert!(!crate::peer::detail_text(&summary_parts(&session)).contains("YOLO observed"));
    }

    /// The card's status is a last-known verdict on a stale host, never a
    /// live heartbeat. Ended detail stays on the row; the card uses its state
    /// word so a long failure message cannot crowd out the title.
    #[farhelm_testtrace::test]
    fn card_status_and_peer_title_keep_their_distinct_contracts() {
        let mut live = row_specimen("stale");
        live.title = "build\u{200B} \u{202E}lanif".into();
        live.status = SessionStatus::Idle;
        live.seen_activity_at = Some(Some(0));
        live.last_activity_at = 10;
        live.stale = true;
        let live_edits = render(live.clone(), HostLocality::Unknown, None);
        let live_text = text_values(&live_edits);
        assert!(live_text.contains(&"Idle — new output".to_string()));
        assert!(live_text.contains(&live.title.escaped()));
        assert!(
            !live_text
                .iter()
                .any(|text| text.contains('\u{202E}') || text.contains('\u{200B}'))
        );
        assert!(
            attribute_values(&live_edits, "class")
                .iter()
                .any(|class| class == "session-summary stale")
        );

        let mut ended = row_specimen("ended");
        ended.status = SessionStatus::Exited {
            exit_code: Some(42),
        };
        let ended_text = text_values(&render(ended, HostLocality::Unknown, None));
        assert!(ended_text.contains(&"Exited".to_string()));
        assert!(!ended_text.contains(&"Exited (code 42)".to_string()));
    }

    /// Text nodes, rather than serialized HTML, are what the tooltip copies.
    fn text_values(edits: &[Mutation]) -> Vec<String> {
        edits
            .iter()
            .filter_map(|edit| match edit {
                Mutation::CreateTextNode { value, .. } | Mutation::SetText { value, .. } => {
                    Some(value.clone())
                }
                _ => None,
            })
            .collect()
    }

    /// Dynamic classes include the stale marker and status vocabulary.
    fn attribute_values(edits: &[Mutation], wanted: &str) -> Vec<String> {
        edits
            .iter()
            .filter_map(|edit| match edit {
                Mutation::SetAttribute {
                    name,
                    value: dioxus::core::AttributeValue::Text(value),
                    ..
                } if *name == wanted => Some(value.clone()),
                _ => None,
            })
            .collect()
    }
}
