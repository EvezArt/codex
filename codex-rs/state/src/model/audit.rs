#[derive(Debug, Clone)]
pub struct CovenantRecord {
    pub version: String,
    pub scopes_json: String,
}

#[derive(Debug, Clone)]
pub struct AuditAction {
    pub created_at: i64,
    pub actor: String,
    pub action_type: String,
    pub scope: String,
    pub covenant_version: String,
    pub event_id: Option<String>,
    pub intent_id: Option<String>,
}
