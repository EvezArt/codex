CREATE TABLE covenants (
    version INTEGER PRIMARY KEY,
    allowed_scopes TEXT NOT NULL,
    created_at INTEGER NOT NULL
);

CREATE TABLE events (
    id TEXT PRIMARY KEY,
    created_at INTEGER NOT NULL,
    description TEXT NOT NULL,
    domain_signature TEXT NOT NULL,
    status TEXT NOT NULL
);

CREATE TABLE audit_actions (
    id TEXT PRIMARY KEY,
    event_id TEXT,
    action_type TEXT NOT NULL,
    scope TEXT NOT NULL,
    covenant_version INTEGER NOT NULL,
    actor TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    FOREIGN KEY (event_id) REFERENCES events(id) ON DELETE SET NULL,
    FOREIGN KEY (covenant_version) REFERENCES covenants(version)
);

CREATE TABLE intent_tokens (
    id TEXT PRIMARY KEY,
    event_id TEXT NOT NULL,
    goal TEXT NOT NULL,
    constraints TEXT NOT NULL,
    success_signal TEXT NOT NULL,
    confidence REAL NOT NULL,
    created_at INTEGER NOT NULL,
    FOREIGN KEY (event_id) REFERENCES events(id) ON DELETE CASCADE
);

CREATE TABLE hypotheses (
    id TEXT PRIMARY KEY,
    event_id TEXT NOT NULL,
    model_type TEXT NOT NULL,
    probability REAL NOT NULL,
    falsifiers TEXT NOT NULL,
    domain_signature TEXT NOT NULL,
    FOREIGN KEY (event_id) REFERENCES events(id) ON DELETE CASCADE
);

CREATE TABLE tests (
    id TEXT PRIMARY KEY,
    event_id TEXT NOT NULL,
    hypothesis_id TEXT NOT NULL,
    description TEXT NOT NULL,
    result TEXT NOT NULL,
    evidence_ref TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    FOREIGN KEY (event_id) REFERENCES events(id) ON DELETE CASCADE,
    FOREIGN KEY (hypothesis_id) REFERENCES hypotheses(id) ON DELETE CASCADE
);

CREATE TABLE outcomes (
    id TEXT PRIMARY KEY,
    event_id TEXT NOT NULL,
    summary TEXT NOT NULL,
    evidence_refs TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    FOREIGN KEY (event_id) REFERENCES events(id) ON DELETE CASCADE,
    CHECK (length(trim(evidence_refs)) > 0)
);

CREATE TABLE patterns (
    id TEXT PRIMARY KEY,
    trigger TEXT NOT NULL,
    invariant TEXT NOT NULL,
    counterexample TEXT NOT NULL,
    best_response TEXT NOT NULL,
    domain_signature TEXT NOT NULL,
    evidence_refs TEXT NOT NULL,
    created_at INTEGER NOT NULL
);

CREATE INDEX idx_audit_actions_event_id ON audit_actions(event_id);
CREATE INDEX idx_audit_actions_created_at ON audit_actions(created_at);

CREATE INDEX idx_covenants_created_at ON covenants(created_at);

CREATE INDEX idx_events_created_at ON events(created_at);

CREATE INDEX idx_intent_tokens_event_id ON intent_tokens(event_id);
CREATE INDEX idx_intent_tokens_created_at ON intent_tokens(created_at);

CREATE INDEX idx_hypotheses_event_id ON hypotheses(event_id);
CREATE INDEX idx_hypotheses_model_type ON hypotheses(model_type);

CREATE INDEX idx_tests_event_id ON tests(event_id);
CREATE INDEX idx_tests_hypothesis_id ON tests(hypothesis_id);
CREATE INDEX idx_tests_created_at ON tests(created_at);

CREATE INDEX idx_outcomes_event_id ON outcomes(event_id);
CREATE INDEX idx_outcomes_created_at ON outcomes(created_at);

CREATE INDEX idx_patterns_created_at ON patterns(created_at);

CREATE TRIGGER outcomes_require_evidence_before_insert
BEFORE INSERT ON outcomes
BEGIN
    SELECT CASE
        WHEN json_array_length(NEW.evidence_refs) IS NULL
            OR json_array_length(NEW.evidence_refs) = 0
        THEN RAISE(ABORT, 'outcomes.evidence_refs must be non-empty')
    END;
    SELECT CASE
        WHEN EXISTS (
            SELECT 1
            FROM json_each(NEW.evidence_refs) AS ev
            LEFT JOIN tests
                ON tests.id = ev.value
                AND tests.event_id = NEW.event_id
            WHERE tests.id IS NULL
        )
        THEN RAISE(ABORT, 'outcomes.evidence_refs must reference tests tied to the same event')
    END;
END;

CREATE TRIGGER outcomes_require_evidence_before_update
BEFORE UPDATE OF evidence_refs, event_id ON outcomes
BEGIN
    SELECT CASE
        WHEN json_array_length(NEW.evidence_refs) IS NULL
            OR json_array_length(NEW.evidence_refs) = 0
        THEN RAISE(ABORT, 'outcomes.evidence_refs must be non-empty')
    END;
    SELECT CASE
        WHEN EXISTS (
            SELECT 1
            FROM json_each(NEW.evidence_refs) AS ev
            LEFT JOIN tests
                ON tests.id = ev.value
                AND tests.event_id = NEW.event_id
            WHERE tests.id IS NULL
        )
        THEN RAISE(ABORT, 'outcomes.evidence_refs must reference tests tied to the same event')
    END;
END;
