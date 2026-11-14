//! The GUI's copy of the helm's pushed collections: decoded once, kept by one generic apply.
//!
//! The channel decodes every pushed entry as it arrives ([`decode_state`], [`decode_changes`]) into the typed value
//! its collection holds. An entry that does not decode breaks the channel like any other protocol violation, so the
//! GUI only ever sees typed state and has no decode error to handle in a view.
//!
//! Every collection (sessions, hosts, preferences, approvals, templates, notices, announcements, host progress) then
//! goes through the same three operations: install a snapshot, upsert or remove entries by key, and replace the
//! metadata (availability, order, counts). There is no per-collection code path to drift. Every entry arrives with its
//! key ([`KeyedEntry`]), the one the helm diffed it by, and that is the key it is filed under, so applying a change
//! never derives a key from an entry. Looking an entry up by a typed id ([`Collections::get`]) still needs the helm's
//! spelling of that key (a host's numeric id as a string, for example); a helper in `farhelm-proto` that spells it
//! comes with the first view that looks entries up that way. The per-collection facts are which type an entry decodes
//! as and which [`PushedEntry`] variant carries it, and both live in exhaustive matches here.
//!
//! [`Collections`] is the one copy of this state. Views read it through [`Collections::ordered`] and
//! [`Collections::get`], which hand out references; a view that kept its own copy of an entry would be a second
//! store that the next change silently leaves behind.

use std::collections::{BTreeMap, HashMap};

use farhelm_proto::announcements::AnnouncementQueue;
use farhelm_proto::approvals::PendingApproval;
use farhelm_proto::launcher::LaunchTemplate;
use farhelm_proto::notices::Notice;
use farhelm_proto::provisioning::ProvisioningView;
use farhelm_proto::ui_channel::{
    Collection, CollectionChanges, CollectionMeta, CollectionState, HostView, KeyedEntry,
    Preferences, SessionRow,
};
use serde::de::DeserializeOwned;

// ---------------------------------------------------------------------------
// Typed entries
// ---------------------------------------------------------------------------

/// One decoded entry of a pushed collection. The variant always matches the collection it was decoded for.
#[derive(Debug, Clone, PartialEq)]
pub enum PushedEntry {
    Session(SessionRow),
    Host(HostView),
    Preferences(Preferences),
    Approval(PendingApproval),
    Template(LaunchTemplate),
    Notice(Notice),
    Announcements(AnnouncementQueue),
    HostProgress(ProvisioningView),
}

/// An entry type that has its own pushed collection, so a reader can name the type and get the collection with it.
pub trait Pushed: DeserializeOwned {
    /// The collection whose entries are this type.
    const COLLECTION: Collection;

    /// This type's value inside a [`PushedEntry`], or `None` for another collection's variant.
    fn from_entry(entry: &PushedEntry) -> Option<&Self>;
}

impl Pushed for SessionRow {
    const COLLECTION: Collection = Collection::Sessions;
    fn from_entry(entry: &PushedEntry) -> Option<&Self> {
        match entry {
            PushedEntry::Session(row) => Some(row),
            _ => None,
        }
    }
}

impl Pushed for HostView {
    const COLLECTION: Collection = Collection::Hosts;
    fn from_entry(entry: &PushedEntry) -> Option<&Self> {
        match entry {
            PushedEntry::Host(host) => Some(host),
            _ => None,
        }
    }
}

impl Pushed for Preferences {
    const COLLECTION: Collection = Collection::Preferences;
    fn from_entry(entry: &PushedEntry) -> Option<&Self> {
        match entry {
            PushedEntry::Preferences(preferences) => Some(preferences),
            _ => None,
        }
    }
}

impl Pushed for PendingApproval {
    const COLLECTION: Collection = Collection::Approvals;
    fn from_entry(entry: &PushedEntry) -> Option<&Self> {
        match entry {
            PushedEntry::Approval(approval) => Some(approval),
            _ => None,
        }
    }
}

impl Pushed for LaunchTemplate {
    const COLLECTION: Collection = Collection::Templates;
    fn from_entry(entry: &PushedEntry) -> Option<&Self> {
        match entry {
            PushedEntry::Template(value) => Some(value),
            _ => None,
        }
    }
}

impl Pushed for Notice {
    const COLLECTION: Collection = Collection::Notices;
    fn from_entry(entry: &PushedEntry) -> Option<&Self> {
        match entry {
            PushedEntry::Notice(value) => Some(value),
            _ => None,
        }
    }
}

impl Pushed for AnnouncementQueue {
    const COLLECTION: Collection = Collection::Announcements;
    fn from_entry(entry: &PushedEntry) -> Option<&Self> {
        match entry {
            PushedEntry::Announcements(value) => Some(value),
            _ => None,
        }
    }
}

impl Pushed for ProvisioningView {
    const COLLECTION: Collection = Collection::HostProgress;
    fn from_entry(entry: &PushedEntry) -> Option<&Self> {
        match entry {
            PushedEntry::HostProgress(value) => Some(value),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Decoding (done by the channel, once per message)
// ---------------------------------------------------------------------------

/// One collection of a snapshot, decoded. Entries keep the helm's order on the wire, which carries no meaning; the
/// collection's order is `meta.order`.
#[derive(Debug, Clone, PartialEq)]
pub struct PushedState {
    pub collection: Collection,
    pub meta: CollectionMeta,
    pub entries: Vec<(String, PushedEntry)>,
}

/// One collection's changes, decoded.
#[derive(Debug, Clone, PartialEq)]
pub struct PushedChanges {
    pub collection: Collection,
    pub meta: CollectionMeta,
    pub upsert: Vec<(String, PushedEntry)>,
    pub remove: Vec<String>,
}

/// An entry the GUI cannot decode. The channel treats this as a broken connection, not as something to show.
#[derive(Debug, thiserror::Error)]
#[error("an entry of {collection:?} does not decode: {error}")]
pub struct DecodeError {
    pub collection: Collection,
    pub error: serde_json::Error,
}

/// Decode a snapshot's collection. All or nothing: one bad entry fails the whole collection.
pub fn decode_state(state: CollectionState) -> Result<PushedState, DecodeError> {
    Ok(PushedState {
        collection: state.collection,
        meta: state.meta,
        entries: decode_entries(state.collection, state.entries)?,
    })
}

/// Decode one collection's changes. All or nothing, like [`decode_state`].
pub fn decode_changes(changes: CollectionChanges) -> Result<PushedChanges, DecodeError> {
    Ok(PushedChanges {
        collection: changes.collection,
        meta: changes.meta,
        upsert: decode_entries(changes.collection, changes.upsert)?,
        remove: changes.remove,
    })
}

/// Decode every entry as its collection's type, keeping the key it arrived under. The match is exhaustive on purpose:
/// a new collection fails to compile here until someone names its entry type.
fn decode_entries(
    collection: Collection,
    entries: Vec<KeyedEntry>,
) -> Result<Vec<(String, PushedEntry)>, DecodeError> {
    let decode: fn(KeyedEntry) -> Result<(String, PushedEntry), serde_json::Error> =
        match collection {
            Collection::Sessions => |value| typed(value, PushedEntry::Session),
            Collection::Hosts => |value| typed(value, PushedEntry::Host),
            Collection::Preferences => |value| typed(value, PushedEntry::Preferences),
            Collection::Approvals => |value| typed(value, PushedEntry::Approval),
            Collection::Templates => |value| typed(value, PushedEntry::Template),
            Collection::Notices => |value| typed(value, PushedEntry::Notice),
            Collection::Announcements => |value| typed(value, PushedEntry::Announcements),
            Collection::HostProgress => |value| typed(value, PushedEntry::HostProgress),
        };
    entries
        .into_iter()
        .map(|entry| decode(entry).map_err(|error| DecodeError { collection, error }))
        .collect()
}

/// Decode one entry's value as `T`, filed under the key it arrived with.
fn typed<T: DeserializeOwned>(
    entry: KeyedEntry,
    wrap: fn(T) -> PushedEntry,
) -> Result<(String, PushedEntry), serde_json::Error> {
    let value: T = serde_json::from_value(entry.value)?;
    Ok((entry.key, wrap(value)))
}

// ---------------------------------------------------------------------------
// The store
// ---------------------------------------------------------------------------

/// One collection as the GUI holds it.
#[derive(Debug, Clone, PartialEq)]
struct Stored {
    meta: CollectionMeta,
    entries: HashMap<String, PushedEntry>,
}

/// The pushed collections, as the last snapshot and the changes since left them.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Collections {
    stored: BTreeMap<Collection, Stored>,
}

impl Collections {
    /// Replace everything with a snapshot. A collection the snapshot does not list is dropped: a snapshot is the
    /// whole pushed state, sent fresh on every (re)connect.
    pub fn install(&mut self, snapshot: &[PushedState]) {
        self.stored = snapshot
            .iter()
            .map(|state| {
                let stored = Stored {
                    meta: state.meta.clone(),
                    entries: state.entries.iter().cloned().collect(),
                };
                (state.collection, stored)
            })
            .collect();
    }

    /// Apply one collection's changes: upserts replace an entry whole, removes drop keys, and the metadata is
    /// replaced.
    ///
    /// NOTE: Changes for a collection no snapshot installed are ignored. The channel never delivers them: it treats
    /// them as a protocol violation and breaks, so the reset that follows rebuilds everything from a new snapshot.
    pub fn apply(&mut self, changes: &PushedChanges) {
        let Some(stored) = self.stored.get_mut(&changes.collection) else {
            tracing::warn!(collection = ?changes.collection, "ignoring changes to a collection with no snapshot");
            return;
        };
        for key in &changes.remove {
            stored.entries.remove(key);
        }
        stored.entries.extend(changes.upsert.iter().cloned());
        stored.meta = changes.meta.clone();
    }

    /// A collection's metadata, or `None` before a snapshot installed it.
    pub fn meta(&self, collection: Collection) -> Option<&CollectionMeta> {
        self.stored.get(&collection).map(|stored| &stored.meta)
    }

    /// The entries of `T`'s collection in the helm's order. A key the order names but no entry carries is skipped:
    /// the helm keeps the last available order while a collection is unavailable, and its entries may be gone.
    pub fn ordered<T: Pushed>(&self) -> Vec<&T> {
        let Some(stored) = self.stored.get(&T::COLLECTION) else {
            return Vec::new();
        };
        stored
            .meta
            .order
            .iter()
            .filter_map(|key| stored.entries.get(key))
            .filter_map(T::from_entry)
            .collect()
    }

    /// One entry of `T`'s collection by key.
    pub fn get<T: Pushed>(&self, key: &str) -> Option<&T> {
        T::from_entry(self.stored.get(&T::COLLECTION)?.entries.get(key)?)
    }

    /// Compare against a fresh snapshot and describe every difference; empty means consistent.
    ///
    /// This is the check that the incremental apply never drifts from what the helm would send from scratch. Tests
    /// use it after a run of changes, and a debug build can run it after a reconnect's snapshot to catch a helm or
    /// client bug that would otherwise show as a quietly wrong list.
    pub fn check_against(&self, snapshot: &[PushedState]) -> Vec<String> {
        let mut fresh = Collections::default();
        fresh.install(snapshot);
        let mut differences = Vec::new();
        let collections: std::collections::BTreeSet<_> = self
            .stored
            .keys()
            .chain(fresh.stored.keys())
            .copied()
            .collect();
        for collection in collections {
            match (self.stored.get(&collection), fresh.stored.get(&collection)) {
                (Some(ours), Some(theirs)) => {
                    if ours.meta != theirs.meta {
                        differences.push(format!("{collection:?}: metadata differs"));
                    }
                    let mut keys: Vec<_> =
                        ours.entries.keys().chain(theirs.entries.keys()).collect();
                    keys.sort();
                    keys.dedup();
                    for key in keys {
                        if ours.entries.get(key) != theirs.entries.get(key) {
                            differences.push(format!("{collection:?}: entry {key} differs"));
                        }
                    }
                }
                (Some(_), None) => {
                    differences.push(format!("{collection:?}: held but not in the snapshot"))
                }
                (None, Some(_)) => {
                    differences.push(format!("{collection:?}: in the snapshot but not held"))
                }
                (None, None) => {}
            }
        }
        differences
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use farhelm_proto::ui_channel::{Availability, PREFERENCES_KEY};
    use serde_json::json;

    fn meta(order: &[&str]) -> CollectionMeta {
        CollectionMeta {
            availability: Availability::Available,
            order: order.iter().map(|key| key.to_string()).collect(),
            counts: None,
        }
    }

    /// An approval whose `destination` carries `label`, so two versions of one key differ, under its id as the wire
    /// key. Built from the type so the test follows the wire shape instead of hand-copying it.
    fn approval(id: &str, label: &str) -> KeyedEntry {
        let value = serde_json::to_value(PendingApproval {
            id: id.into(),
            host_id: 1,
            host_name: "local".into(),
            session: farhelm_proto::approvals::ApprovalSession {
                id: "s1".into(),
                title: None,
                host_name: None,
            },
            action: farhelm_proto::approvals::ApprovalAction::HostAdd {
                destination: label.into(),
                alias: None,
            },
            own_host_session_action: false,
            expires_at_ms: 0,
        })
        .unwrap();
        KeyedEntry {
            key: id.into(),
            value,
        }
    }

    fn preferences(compact: bool) -> KeyedEntry {
        let value = serde_json::to_value(Preferences {
            compact: Some(compact),
            ..Preferences::default()
        })
        .unwrap();
        KeyedEntry {
            key: PREFERENCES_KEY.into(),
            value,
        }
    }

    fn state(collection: Collection, order: &[&str], entries: Vec<KeyedEntry>) -> PushedState {
        decode_state(CollectionState {
            collection,
            meta: meta(order),
            entries,
        })
        .unwrap()
    }

    fn changes(
        collection: Collection,
        order: &[&str],
        upsert: Vec<KeyedEntry>,
        remove: &[&str],
    ) -> PushedChanges {
        decode_changes(CollectionChanges {
            collection,
            meta: meta(order),
            upsert,
            remove: remove.iter().map(|key| key.to_string()).collect(),
        })
        .unwrap()
    }

    /// The generic apply is the GUI's only model of pushed state, so install, upsert, remove and metadata replacement
    /// must compose to exactly what a fresh snapshot of the end state holds; `check_against` is the oracle.
    #[test]
    fn changes_applied_to_a_snapshot_match_a_fresh_snapshot_of_the_result() {
        let mut collections = Collections::default();
        collections.install(&[
            state(
                Collection::Approvals,
                &["a", "b"],
                vec![approval("a", "one"), approval("b", "two")],
            ),
            state(
                Collection::Preferences,
                &["preferences"],
                vec![preferences(false)],
            ),
        ]);
        collections.apply(&changes(
            Collection::Approvals,
            &["c", "a"],
            vec![approval("c", "three"), approval("a", "one again")],
            &["b"],
        ));
        collections.apply(&changes(
            Collection::Preferences,
            &["preferences"],
            vec![preferences(true)],
            &[],
        ));

        let fresh = [
            state(
                Collection::Approvals,
                &["c", "a"],
                vec![approval("a", "one again"), approval("c", "three")],
            ),
            state(
                Collection::Preferences,
                &["preferences"],
                vec![preferences(true)],
            ),
        ];
        assert_eq!(collections.check_against(&fresh), Vec::<String>::new());
        let ordered = collections.ordered::<PendingApproval>();
        assert_eq!(
            ordered.iter().map(|a| a.id.as_str()).collect::<Vec<_>>(),
            ["c", "a"]
        );
        let prefs = collections.get::<Preferences>("preferences").unwrap();
        assert_eq!(prefs.compact, Some(true));

        // And the oracle is not vacuous: a stale entry and a stale order are both reported.
        let stale = [state(
            Collection::Approvals,
            &["a", "c"],
            vec![approval("a", "one"), approval("c", "three")],
        )];
        let differences = collections.check_against(&stale);
        assert!(
            differences.contains(&"Approvals: metadata differs".to_string()),
            "{differences:?}"
        );
        assert!(
            differences.contains(&"Approvals: entry a differs".to_string()),
            "{differences:?}"
        );
        assert!(
            differences.contains(&"Preferences: held but not in the snapshot".to_string()),
            "{differences:?}"
        );
    }

    /// An entry that is not its collection's type cannot be filed, and a half-decoded message would leave the list
    /// inconsistent with both the old and the new state, so one bad entry fails the whole message's decode. The
    /// channel turns that failure into a broken connection (its own tests cover that half).
    #[test]
    fn one_undecodable_entry_fails_the_whole_decode() {
        let result = decode_changes(CollectionChanges {
            collection: Collection::Approvals,
            meta: meta(&[]),
            upsert: vec![
                approval("b", "two"),
                KeyedEntry {
                    key: "c".into(),
                    value: json!({"not": "an approval"}),
                },
            ],
            remove: vec!["a".into()],
        });
        assert!(
            matches!(
                result,
                Err(DecodeError {
                    collection: Collection::Approvals,
                    ..
                })
            ),
            "{result:?}"
        );
        // A valid entry of another collection's type is just as undecodable here: it must decode as this one's type.
        assert!(
            decode_state(CollectionState {
                collection: Collection::Preferences,
                meta: meta(&[]),
                entries: vec![approval("a", "one")],
            })
            .is_err()
        );
    }

    /// The four collections added after the first channel PR (templates, notices, the announcement queue, host
    /// progress) each decode as their own proto type and are read back through the typed accessors, so a GUI view
    /// gets typed state for them exactly as for the first four. Built from the proto types, so the test follows the
    /// wire shape the helm serializes.
    #[test]
    fn templates_notices_announcements_and_host_progress_decode_as_their_types() {
        use farhelm_proto::announcements::Announcement;
        use farhelm_proto::notices::NoticeContent;
        use farhelm_proto::provisioning::RunStatus;
        use farhelm_proto::ui_channel::ANNOUNCEMENTS_KEY;
        let keyed = |key: &str, value: serde_json::Value| KeyedEntry {
            key: key.into(),
            value,
        };
        let template = LaunchTemplate {
            name: "review".into(),
            fields: serde_json::from_value(json!({ "prompt": "review this" })).unwrap(),
        };
        let notice = Notice {
            id: 7,
            host: 1,
            raised_at: 10,
            read: false,
            resolved: false,
            content: NoticeContent::StuckTrash {
                checkout_id: "c1".into(),
                name: "c1-archive".into(),
                repository: "example/project".into(),
                bytes: Some(4096),
                reason: "permission denied".into(),
                at: 20,
            },
        };
        let queue = AnnouncementQueue {
            pending: vec![Announcement {
                id: "news-1".into(),
                title: "News".into(),
                body: "Something new.".into(),
                detail: None,
                link: None,
            }],
            history: vec![],
        };
        let run = ProvisioningView {
            host_id: 1,
            run_id: Some("run-1".into()),
            operation: None,
            status: RunStatus::Running,
            steps: vec![],
            message: None,
        };
        let snapshot = [
            (
                Collection::Templates,
                "review",
                serde_json::to_value(&template).unwrap(),
            ),
            (
                Collection::Notices,
                "7",
                serde_json::to_value(&notice).unwrap(),
            ),
            (
                Collection::Announcements,
                ANNOUNCEMENTS_KEY,
                serde_json::to_value(&queue).unwrap(),
            ),
            (
                Collection::HostProgress,
                "1",
                serde_json::to_value(&run).unwrap(),
            ),
        ]
        .into_iter()
        .map(|(collection, key, value)| {
            decode_state(CollectionState {
                collection,
                meta: meta(&[key]),
                entries: vec![keyed(key, value)],
            })
            .expect("the entry decodes as its collection's type")
        })
        .collect::<Vec<_>>();
        let mut held = Collections::default();
        held.install(&snapshot);
        assert_eq!(held.get::<LaunchTemplate>("review"), Some(&template));
        assert_eq!(held.get::<Notice>("7"), Some(&notice));
        assert_eq!(
            held.get::<AnnouncementQueue>(ANNOUNCEMENTS_KEY),
            Some(&queue)
        );
        assert_eq!(held.ordered::<ProvisioningView>(), [&run]);
    }
}
