-- Frozen fresh schema from baseline 7eb561e8f1968d7dfbbaf9592deee95a74ad4c09.
-- Do not derive this fixture from the schema under test.
BEGIN;
CREATE TABLE sessions (
    id            TEXT PRIMARY KEY,
    title         TEXT NOT NULL,
    cwd           TEXT NOT NULL,
    invocation    TEXT NOT NULL,
    tmux_name     TEXT NOT NULL UNIQUE,
    pane          TEXT NOT NULL,
    created_at    INTEGER NOT NULL,
    outcome_state TEXT NOT NULL DEFAULT 'launching',
    exit_code     INTEGER,
    annotation    TEXT,
    error_detail  TEXT,
    agent_kind    TEXT NOT NULL DEFAULT 'generic',
    resume_template       TEXT,
    canonical_cwd         TEXT,
    captured_conversation TEXT,
    captured_record       TEXT,
    capture_ambiguous     INTEGER NOT NULL DEFAULT 0,
    first_input_at        INTEGER,
    generation            INTEGER NOT NULL DEFAULT 0,
    launch_scoped         INTEGER NOT NULL DEFAULT 0,
    source_profile_id     TEXT,
    source_profile_name   TEXT,
    parent                TEXT,
    session_token         TEXT,
    creation_seq          INTEGER,
    archived              INTEGER NOT NULL DEFAULT 0,
    last_activity_at      INTEGER NOT NULL DEFAULT 0,
    conversation_source   TEXT,
    launch                TEXT,
    last_work_started_at  INTEGER NOT NULL DEFAULT 0
) STRICT;
CREATE TABLE supervisor_meta (
    id            INTEGER PRIMARY KEY CHECK (id = 0),
    boot_id       TEXT,
    host_identity TEXT,
    last_creation_seq INTEGER NOT NULL DEFAULT 0
) STRICT;
CREATE TABLE create_reservations (
    intent_key   TEXT PRIMARY KEY,
    fingerprint  TEXT NOT NULL,
    state        TEXT NOT NULL,
    session_id   TEXT NOT NULL,
    tmux_name    TEXT NOT NULL,
    error_kind   TEXT,
    error_detail TEXT,
    created_at   INTEGER NOT NULL,
    dedup_scope  TEXT NOT NULL DEFAULT 'permanent'
) STRICT;
CREATE INDEX create_reservations_pending
    ON create_reservations (session_id) WHERE state = 'pending';
PRAGMA user_version = 17;
COMMIT;
