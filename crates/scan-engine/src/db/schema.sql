PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;
PRAGMA cache_size = -64000;
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS schema_version (
    version INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS sessions (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    root_path TEXT NOT NULL,
    status TEXT NOT NULL,
    config_json TEXT NOT NULL,
    scan_generation INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS files (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    path TEXT NOT NULL,
    size INTEGER NOT NULL,
    mtime_ns INTEGER NOT NULL,
    width INTEGER,
    height INTEGER,
    inode INTEGER,
    scan_generation INTEGER NOT NULL,
    created_at TEXT,
    modified_at TEXT,
    deleted_at TEXT,
    companion_raw_path TEXT,
    companion_raw_size INTEGER,
    UNIQUE(session_id, path)
);

CREATE TABLE IF NOT EXISTS fingerprints (
    file_id INTEGER PRIMARY KEY REFERENCES files(id) ON DELETE CASCADE,
    blake3 TEXT,
    dhash INTEGER,
    phash INTEGER,
    exif_json TEXT
);

CREATE TABLE IF NOT EXISTS duplicate_groups (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    kind TEXT NOT NULL,
    confidence REAL NOT NULL,
    review_status TEXT NOT NULL DEFAULT 'pending'
);

CREATE TABLE IF NOT EXISTS duplicate_members (
    group_id INTEGER NOT NULL REFERENCES duplicate_groups(id) ON DELETE CASCADE,
    file_id INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    is_keeper INTEGER,
    PRIMARY KEY (group_id, file_id)
);

CREATE TABLE IF NOT EXISTS scan_checkpoints (
    session_id TEXT PRIMARY KEY REFERENCES sessions(id) ON DELETE CASCADE,
    phase TEXT NOT NULL,
    files_processed INTEGER NOT NULL DEFAULT 0,
    files_total_estimate INTEGER NOT NULL DEFAULT 0,
    last_path TEXT,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_files_session ON files(session_id);
CREATE INDEX IF NOT EXISTS idx_files_session_path ON files(session_id, path);
CREATE INDEX IF NOT EXISTS idx_fingerprints_blake3 ON fingerprints(blake3);
CREATE INDEX IF NOT EXISTS idx_fingerprints_phash_prefix ON fingerprints((phash >> 48));
CREATE INDEX IF NOT EXISTS idx_groups_session_status ON duplicate_groups(session_id, review_status);
