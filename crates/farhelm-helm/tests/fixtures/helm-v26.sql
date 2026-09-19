-- Frozen fresh schema from baseline 7eb561e8f1968d7dfbbaf9592deee95a74ad4c09.
-- Historical constants expanded; historical SQL comments omitted.
CREATE TABLE hosts (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    kind             TEXT NOT NULL CHECK (kind IN ('local', 'ssh')),
    destination      TEXT,
    remote_farhelm   TEXT,
    remote_state_dir TEXT,
    host_identity    TEXT,
    cache_truncated  INTEGER NOT NULL DEFAULT 0,
    alias            TEXT,
    CHECK (
        (kind = 'local' AND destination IS NULL AND remote_farhelm IS NULL
             AND remote_state_dir IS NULL)
        OR (kind = 'ssh' AND destination IS NOT NULL)
    )
) STRICT;
CREATE UNIQUE INDEX hosts_one_local_row
    ON hosts (kind) WHERE kind = 'local';
CREATE UNIQUE INDEX hosts_ssh_destination
    ON hosts (destination) WHERE kind = 'ssh';
CREATE UNIQUE INDEX hosts_identity_claim
    ON hosts (host_identity) WHERE host_identity IS NOT NULL;
CREATE TABLE session_cache (
    host_id    INTEGER NOT NULL REFERENCES hosts (id) ON DELETE CASCADE,
    session_id TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    info_json  TEXT NOT NULL,
    archived   INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (host_id, session_id)
) STRICT;
CREATE UNIQUE INDEX session_cache_one_owner
    ON session_cache (session_id);
CREATE TABLE profiles (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    invocation      TEXT NOT NULL,
    agent_kind      TEXT NOT NULL,
    resume_template TEXT
) STRICT;
CREATE TABLE remembered_profile (
    singleton     INTEGER PRIMARY KEY CHECK (singleton = 1),
    profile_id    TEXT NOT NULL,
    source_host_id INTEGER,
    source_created_at INTEGER,
    source_session_id TEXT,
    source_creation_seq INTEGER,
    CHECK ((source_created_at IS NULL) = (source_session_id IS NULL))
) STRICT;
CREATE TABLE web_token (
    singleton  INTEGER PRIMARY KEY CHECK (singleton = 1),
    token      TEXT NOT NULL,
    created_at INTEGER NOT NULL
) STRICT;
CREATE TABLE device_sessions (
    cookie_hash BLOB PRIMARY KEY CHECK (length(cookie_hash) = 32),
    created_at  INTEGER NOT NULL
) STRICT;
CREATE TABLE preferences (
    singleton              INTEGER PRIMARY KEY CHECK (singleton = 1),
    list_sort              TEXT,
    last_selected          TEXT,
    compact                INTEGER CHECK (compact IN (0, 1)),
    remembered_permissions TEXT
) STRICT;
CREATE TABLE launch_history (
    host_id       INTEGER NOT NULL REFERENCES hosts (id) ON DELETE CASCADE,
    host_identity TEXT NOT NULL,
    session_id    TEXT NOT NULL,
    created_at    INTEGER NOT NULL,
    creation_seq  INTEGER,
    ordering_kind INTEGER NOT NULL,
    ordering_value INTEGER NOT NULL,
    cwd           TEXT NOT NULL,
    canonical_cwd TEXT,
    launch_json   TEXT NOT NULL,
    PRIMARY KEY (host_id, host_identity, session_id)
) STRICT;
CREATE INDEX launch_history_recent
    ON launch_history (host_id, host_identity, ordering_kind DESC, ordering_value DESC, session_id ASC);
CREATE TABLE create_history_sessions (
    host_id       INTEGER NOT NULL REFERENCES hosts (id) ON DELETE CASCADE,
    host_identity TEXT NOT NULL,
    session_id    TEXT NOT NULL,
    creation_seq  INTEGER,
    created_at    INTEGER NOT NULL,
    ordering_kind INTEGER NOT NULL,
    ordering_value INTEGER NOT NULL,
    PRIMARY KEY (host_id, host_identity, session_id)
) STRICT;
CREATE INDEX create_history_sessions_recent
    ON create_history_sessions (host_id, host_identity, ordering_kind DESC, ordering_value DESC, session_id ASC);
CREATE TABLE create_history_cutoffs (
    host_id          INTEGER NOT NULL REFERENCES hosts (id) ON DELETE CASCADE,
    host_identity    TEXT NOT NULL,
    cutoff_kind      INTEGER NOT NULL,
    cutoff_value     INTEGER NOT NULL,
    cutoff_session_id TEXT NOT NULL,
    cutoff_created_at INTEGER NOT NULL,
    fallback_cutoff_created_at INTEGER NOT NULL,
    fallback_cutoff_session_id TEXT NOT NULL,
    PRIMARY KEY (host_id, host_identity)
) STRICT;
CREATE TABLE folder_history (
    host_id       INTEGER NOT NULL REFERENCES hosts (id) ON DELETE CASCADE,
    host_identity TEXT NOT NULL,
    canonical_cwd TEXT NOT NULL,
    canonical_proven INTEGER NOT NULL CHECK (canonical_proven IN (0, 1)),
    display_cwd   TEXT NOT NULL,
    created_at    INTEGER NOT NULL,
    creation_seq  INTEGER,
    ordering_kind INTEGER NOT NULL,
    ordering_value INTEGER NOT NULL,
    ordering_session_id TEXT NOT NULL,
    PRIMARY KEY (host_id, host_identity, canonical_cwd)
) STRICT;
CREATE INDEX folder_history_recent
    ON folder_history (host_id, host_identity, ordering_kind DESC, ordering_value DESC, ordering_session_id ASC);
CREATE TABLE create_history_partitions (
    host_id INTEGER NOT NULL REFERENCES hosts (id) ON DELETE CASCADE,
    host_identity TEXT NOT NULL,
    fallback_order INTEGER NOT NULL CHECK (fallback_order IN (0, 1)),
    PRIMARY KEY (host_id, host_identity)
) STRICT;
CREATE TABLE session_seen (
    session_id       TEXT NOT NULL PRIMARY KEY,
    seen_activity_at INTEGER NOT NULL
) STRICT;
PRAGMA user_version = 26;
