//! The GUI's copy of the helm's pushed collections: decoded once, kept by one generic apply.
//!
//! The channel decodes every pushed entry as it arrives ([`decode_state`], [`decode_changes`]) into the typed value
//! its collection holds. An entry that does not decode breaks the channel like any other protocol violation, so the
//! GUI only ever sees typed state and has no decode error to handle in a view.
//!
//! Every collection (sessions, hosts, preferences, approvals) then goes through the same three operations: install a
//! snapshot, upsert or remove entries by key, and replace the metadata (availability, order, counts). There is no
//! per-collection code path to drift. The per-collection facts are which [`Entry`] type keys its entries (from the
//! proto crate, where each type's round-trip test pins the key to the one the helm uses) and which [`PushedEntry`]
//! variant carries it, and both live in exhaustive matches here.
//!
//! [`Collections`] is the one copy of this state. Views read it through [`Collections::ordered`] and
//! [`Collections::get`], which hand out references; a view that kept its own copy of an entry would be a second
//! store that the next change silently leaves behind.

use std::collections::{BTreeMap, HashMap};

use farhelm_proto::approvals::PendingApproval;
use farhelm_proto::ui_channel::{
    Collection, CollectionChanges, CollectionMeta, CollectionState, Entry, HostView, Preferences,
    SessionRow,
};
use serde_json::Value;

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
}

/// An entry type that has its own pushed collection, so a reader can name the type and get the collection with it.
pub trait Pushed: Entry {
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

/// Decode and key every entry with its collection's type. The match is exhaustive on purpose: a new collection fails
/// to compile here until someone names its entry type.
fn decode_entries(
    collection: Collection,
    entries: Vec<Value>,
) -> Result<Vec<(String, PushedEntry)>, DecodeError> {
    let decode: fn(Value) -> Result<(String, PushedEntry), serde_json::Error> = match collection {
        Collection::Sessions => |value| typed(value, PushedEntry::Session),
        Collection::Hosts => |value| typed(value, PushedEntry::Host),
        Collection::Preferences => |value| typed(value, PushedEntry::Preferences),
        Collection::Approvals => |value| typed(value, PushedEntry::Approval),
    };
    entries
        .into_iter()
        .map(|entry| decode(entry).map_err(|error| DecodeError { collection, error }))
        .collect()
}

/// Decode one entry as `T` and key it with `T`'s own key, the one the helm diffs by.
fn typed<T: Entry>(
    value: Value,
    wrap: fn(T) -> PushedEntry,
) -> Result<(String, PushedEntry), serde_json::Error> {
    let entry: T = serde_json::from_value(value)?;
    Ok((entry.key(), wrap(entry)))
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
    use farhelm_proto::ui_channel::Availability;
    use serde_json::json;

    fn meta(order: &[&str]) -> CollectionMeta {
        CollectionMeta {
            availability: Availability::Available,
            order: order.iter().map(|key| key.to_string()).collect(),
            counts: None,
        }
    }

    /// An approval whose `destination` carries `label`, so two versions of one key differ. Built from the type so
    /// the test follows the wire shape instead of hand-copying it.
    fn approval(id: &str, label: &str) -> Value {
        serde_json::to_value(PendingApproval {
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
        .unwrap()
    }

    fn preferences(compact: bool) -> Value {
        serde_json::to_value(Preferences {
            compact: Some(compact),
            ..Preferences::default()
        })
        .unwrap()
    }

    fn state(collection: Collection, order: &[&str], entries: Vec<Value>) -> PushedState {
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
        upsert: Vec<Value>,
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

    /// An entry without a key cannot be filed, and a half-decoded message would leave the list inconsistent with
    /// both the old and the new state, so one bad entry fails the whole message's decode. The channel turns that
    /// failure into a broken connection (its own tests cover that half).
    #[test]
    fn one_undecodable_entry_fails_the_whole_decode() {
        let result = decode_changes(CollectionChanges {
            collection: Collection::Approvals,
            meta: meta(&[]),
            upsert: vec![approval("b", "two"), json!({"not": "an approval"})],
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
        // A valid entry of another collection's type is just as undecodable here: keys come from the right type only.
        assert!(
            decode_state(CollectionState {
                collection: Collection::Preferences,
                meta: meta(&[]),
                entries: vec![approval("a", "one")],
            })
            .is_err()
        );
    }
}
