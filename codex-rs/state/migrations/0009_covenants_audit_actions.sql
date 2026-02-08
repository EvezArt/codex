CREATE TABLE covenants (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    version TEXT NOT NULL UNIQUE,
    scopes_json TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE audit_actions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at INTEGER NOT NULL,
    actor TEXT NOT NULL,
    action_type TEXT NOT NULL,
    scope TEXT NOT NULL,
    covenant_version TEXT NOT NULL,
    event_id TEXT,
    intent_id TEXT
);

CREATE INDEX idx_audit_actions_created_at ON audit_actions(created_at DESC, id DESC);
CREATE INDEX idx_audit_actions_covenant_version ON audit_actions(covenant_version);
