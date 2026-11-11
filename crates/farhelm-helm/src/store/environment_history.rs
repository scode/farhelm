//! Reusable environment intent outlives the disposable installation it created.
//!
//! The ordinary create-history transaction decides admission and remembers GUI
//! defaults. This projection retains only its captured recipe/place, explicit
//! launch selection and repository-or-home intent. No foreign key ties that
//! intent to a container or sandbox that Delete is meant to remove.

use super::*;
use farhelm_proto::LaunchTarget;

/// Add an independent bounded projection while retaining installation-bound
/// history's existing foreign keys and directory-identity rules.
pub(super) fn migrate(tx: &rusqlite::Transaction<'_>) -> anyhow::Result<()> {
    let preference_exists: bool = tx.query_row(
        "SELECT count(*) != 0 FROM pragma_table_info('preferences') WHERE name = 'last_container_host'",
        [],
        |row| row.get(0),
    )?;
    if !preference_exists {
        tx.execute_batch("ALTER TABLE preferences ADD COLUMN last_container_host INTEGER;")?;
    }
    tx.execute_batch(
        "CREATE TABLE IF NOT EXISTS environment_launch_history (
             admission_order INTEGER PRIMARY KEY,
             origin_entry_uuid TEXT NOT NULL,
             origin_host_id INTEGER NOT NULL,
             origin_host_identity TEXT NOT NULL,
             session_id TEXT NOT NULL,
             target_json TEXT NOT NULL,
             selection_json TEXT,
             github_repo TEXT,
             admitted_at INTEGER NOT NULL,
             UNIQUE (origin_entry_uuid, origin_host_identity, session_id)
         ) STRICT;",
    )?;
    Ok(())
}

/// Called only after the existing create window admits a fresh observation.
/// Its selection and target are trusted request provenance. The peer's launch
/// and filesystem paths cannot supply reusable environment intent.
pub(super) fn record(
    tx: &rusqlite::Transaction<'_>,
    create: &AdmittedCreate<'_>,
    target: Option<&LaunchTarget>,
    selection: Option<&LaunchSelection>,
    github_repo: Option<&str>,
) -> anyhow::Result<bool> {
    let Some(target) = target else {
        return Ok(false);
    };
    let container_host = target.place().container_host();
    let target = serde_json::to_string(target)?;
    let selection = selection.map(serde_json::to_string).transpose()?;
    let admitted_at = i64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs(),
    )?;
    let changed = tx.execute(
        "INSERT INTO environment_launch_history (
             origin_entry_uuid, origin_host_id, origin_host_identity, session_id,
             target_json, selection_json, github_repo, admitted_at
         ) SELECT entry_uuid, id, ?2, ?3, ?4, ?5, ?6, ?7 FROM hosts WHERE id = ?1
         ON CONFLICT (origin_entry_uuid, origin_host_identity, session_id) DO NOTHING",
        rusqlite::params![
            create.host,
            create.identity,
            create.entry.id,
            target,
            selection,
            github_repo,
            admitted_at
        ],
    )? != 0;
    if !changed {
        // Identity adoption can clear the ordinary installation's admissions
        // while this independent source key survives. A retained replay must
        // establish novelty here before it can change any GUI default.
        return Ok(false);
    }
    if let Some(host) = container_host {
        tx.execute(
            "INSERT INTO preferences (singleton, last_container_host) VALUES (1, ?1)
             ON CONFLICT (singleton) DO UPDATE SET last_container_host = excluded.last_container_host
             WHERE last_container_host IS NOT excluded.last_container_host",
            [host],
        )?;
    }
    // One shared admission order compares launches from independent supervisors.
    // Raw creates carry NULL selection and age this same window. They supply
    // admission and place-default facts without becoming reusable setups.
    tx.execute(
        "DELETE FROM environment_launch_history WHERE admission_order IN (
             SELECT admission_order FROM environment_launch_history
             ORDER BY admission_order DESC LIMIT -1 OFFSET ?1
         )",
        [MAX_LAUNCH_HISTORY],
    )?;
    Ok(true)
}

impl HelmStore {
    /// Return reusable environment choices even after their original host was
    /// deleted. The origin host is diagnostic, while target owns launch authority.
    /// Missing recipes or places remain representable for client applicability.
    pub async fn environment_launch_history(&self) -> anyhow::Result<Vec<LaunchHistoryEntry>> {
        self.conn
            .call("environment launch history task panicked", move |conn| {
                let mut statement = conn.prepare(
                    "SELECT origin_host_id, target_json, selection_json, github_repo,
                        admitted_at, admission_order
                 FROM environment_launch_history WHERE selection_json IS NOT NULL
                 ORDER BY admission_order DESC LIMIT ?1",
                )?;
                let rows = statement.query_map([MAX_LAUNCH_HISTORY], |row| {
                    Ok((
                        row.get::<_, HostId>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, i64>(5)?,
                    ))
                })?;
                let mut entries = Vec::new();
                for row in rows {
                    let (host, target, selection, repo, created_at, sequence) = row?;
                    let decoded = decode_entry(
                        host,
                        &target,
                        &selection,
                        repo.as_deref(),
                        created_at,
                        sequence,
                    );
                    match decoded {
                        Ok(entry) => entries.push(entry),
                        Err(_) => {
                            tracing::warn!("a stored environment launch intent no longer decodes")
                        }
                    }
                }
                Ok(entries)
            })
            .await
    }
}

/// Decode only captured reusable intent. A corrupt repo must stay a failed
/// observation, because treating it as home intent would launch somewhere else.
fn decode_entry(
    host: HostId,
    target: &str,
    selection: &str,
    repo: Option<&str>,
    created_at: i64,
    sequence: i64,
) -> anyhow::Result<LaunchHistoryEntry> {
    Ok(LaunchHistoryEntry {
        host,
        target: Some(serde_json::from_str(target)?),
        github_repo: repo.map(farhelm_proto::parse_github_repo).transpose()?,
        canonical_cwd: None,
        cwd: "~".into(),
        selection: serde_json::from_str(selection)?,
        created_at,
        creation_seq: Some(u64::try_from(sequence)?),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Corrupt persisted intent must never silently become another launch.
    /// In particular, an invalid repository cannot be reinterpreted as home.
    #[farhelm_testtrace::test]
    fn corrupt_environment_intent_refuses_decoding() {
        let target = serde_json::to_string(&LaunchTarget::Environment {
            recipe: "dev".into(),
            place: farhelm_proto::Place::TensorlakeAccount,
        })
        .unwrap();
        let selection = serde_json::json!({"harness": "codex"}).to_string();
        assert!(
            decode_entry(7, &target, &selection, None, 1, 1).is_ok(),
            "premise: the closed target and selection decode"
        );
        assert!(decode_entry(7, "{", &selection, None, 1, 1).is_err());
        assert!(decode_entry(7, &target, "{", None, 1, 1).is_err());
        assert!(decode_entry(7, &target, &selection, Some("invalid"), 1, 1).is_err());
    }
}
