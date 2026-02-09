use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use anyhow::Context;
use anyhow::Result;
use clap::Args;
use clap::Parser;
use clap::Subcommand;
use codex_core::config::find_codex_home;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Parser)]
pub struct CovenantCommand {
    #[command(subcommand)]
    subcommand: CovenantSubcommand,
}

#[derive(Debug, Subcommand)]
enum CovenantSubcommand {
    /// Insert a new event with optional intent metadata.
    Log(LogArgs),
    /// Add a hypothesis to an existing event.
    Predict(PredictArgs),
    /// Attach a test and result to an event.
    Test(TestArgs),
    /// Resolve an event with outcome evidence.
    Resolve(ResolveArgs),
    /// Create or update a named pattern.
    #[command(name = "patterns-add")]
    PatternsAdd(PatternsAddArgs),
}

#[derive(Debug, Args)]
struct LogArgs {
    /// Covenant scope for the event.
    #[arg(long)]
    scope: String,
    /// Summary of what happened.
    #[arg(long)]
    summary: String,
    /// Optional intent summary for the event.
    #[arg(long)]
    intent: Option<String>,
    /// Optional JSON payload associated with the intent.
    #[arg(long = "intent-data")]
    intent_data: Option<String>,
}

#[derive(Debug, Args)]
struct PredictArgs {
    /// Covenant scope for the event.
    #[arg(long)]
    scope: String,
    /// Event id to attach the hypothesis to.
    #[arg(long = "event-id")]
    event_id: String,
    /// Hypothesis text to attach.
    #[arg(long)]
    hypothesis: String,
    /// Optional confidence between 0 and 1.
    #[arg(long)]
    confidence: Option<f32>,
}

#[derive(Debug, Args)]
struct TestArgs {
    /// Covenant scope for the event.
    #[arg(long)]
    scope: String,
    /// Event id to attach the test to.
    #[arg(long = "event-id")]
    event_id: String,
    /// Test name or description.
    #[arg(long)]
    name: String,
    /// Result status for the test.
    #[arg(long)]
    result: String,
    /// Optional details about the test run.
    #[arg(long)]
    details: Option<String>,
}

#[derive(Debug, Args)]
struct ResolveArgs {
    /// Covenant scope for the event.
    #[arg(long)]
    scope: String,
    /// Event id to resolve.
    #[arg(long = "event-id")]
    event_id: String,
    /// Resolution outcome summary.
    #[arg(long)]
    outcome: String,
    /// Evidence references for the resolution (repeatable).
    #[arg(long)]
    evidence: Vec<String>,
}

#[derive(Debug, Args)]
struct PatternsAddArgs {
    /// Covenant scope for the pattern.
    #[arg(long)]
    scope: String,
    /// Pattern name or identifier.
    #[arg(long)]
    name: String,
    /// Pattern definition to store.
    #[arg(long)]
    pattern: String,
    /// Optional notes or context for the pattern.
    #[arg(long)]
    notes: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Event {
    id: String,
    scope: String,
    created_at: i64,
    summary: String,
    intent: Option<Intent>,
    hypotheses: Vec<Hypothesis>,
    tests: Vec<TestRecord>,
    resolution: Option<Resolution>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Intent {
    summary: Option<String>,
    data: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Hypothesis {
    id: String,
    created_at: i64,
    text: String,
    confidence: Option<f32>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TestRecord {
    id: String,
    created_at: i64,
    name: String,
    result: String,
    details: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Resolution {
    resolved_at: i64,
    outcome: String,
    evidence: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PatternEntry {
    name: String,
    scope: String,
    pattern: String,
    notes: Option<String>,
    updated_at: i64,
}

#[derive(Debug, Serialize)]
struct AuditEntry {
    timestamp: i64,
    action: String,
    scope: String,
    target: String,
    details: Value,
}

struct StorePaths {
    events: PathBuf,
    patterns: PathBuf,
    audit_log: PathBuf,
}

impl CovenantCommand {
    pub fn run(self) -> Result<()> {
        match self.subcommand {
            CovenantSubcommand::Log(args) => run_log(args),
            CovenantSubcommand::Predict(args) => run_predict(args),
            CovenantSubcommand::Test(args) => run_test(args),
            CovenantSubcommand::Resolve(args) => run_resolve(args),
            CovenantSubcommand::PatternsAdd(args) => run_patterns_add(args),
        }
    }
}

fn run_log(args: LogArgs) -> Result<()> {
    let scope = normalize_scope(&args.scope)?;
    let intent = build_intent(args.intent.as_ref(), args.intent_data.as_ref())?;
    let mut events = load_events()?;
    let event_id = Uuid::new_v4().to_string();
    let event = Event {
        id: event_id.clone(),
        scope: scope.clone(),
        created_at: now_epoch_seconds()?,
        summary: args.summary.clone(),
        intent,
        hypotheses: Vec::new(),
        tests: Vec::new(),
        resolution: None,
    };
    events.push(event);
    save_events(&events)?;

    let audit = AuditEntry {
        timestamp: now_epoch_seconds()?,
        action: "log".to_string(),
        scope: scope.clone(),
        target: event_id.clone(),
        details: serde_json::json!({
            "summary": args.summary,
            "intent": args.intent,
        }),
    };
    append_audit(audit)?;

    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "status": "ok",
            "eventId": event_id,
        }))?
    );
    Ok(())
}

fn run_predict(args: PredictArgs) -> Result<()> {
    let scope = normalize_scope(&args.scope)?;
    let mut events = load_events()?;
    let event = find_event_mut(&mut events, &args.event_id)?;
    ensure_scope(&scope, &event.scope, "event")?;

    let hypothesis = Hypothesis {
        id: Uuid::new_v4().to_string(),
        created_at: now_epoch_seconds()?,
        text: args.hypothesis.clone(),
        confidence: args.confidence,
    };
    event.hypotheses.push(hypothesis);
    save_events(&events)?;

    let audit = AuditEntry {
        timestamp: now_epoch_seconds()?,
        action: "predict".to_string(),
        scope: scope.clone(),
        target: args.event_id.clone(),
        details: serde_json::json!({
            "hypothesis": args.hypothesis,
            "confidence": args.confidence,
        }),
    };
    append_audit(audit)?;

    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "status": "ok",
            "eventId": args.event_id,
        }))?
    );
    Ok(())
}

fn run_test(args: TestArgs) -> Result<()> {
    let scope = normalize_scope(&args.scope)?;
    let mut events = load_events()?;
    let event = find_event_mut(&mut events, &args.event_id)?;
    ensure_scope(&scope, &event.scope, "event")?;

    let record = TestRecord {
        id: Uuid::new_v4().to_string(),
        created_at: now_epoch_seconds()?,
        name: args.name.clone(),
        result: args.result.clone(),
        details: args.details.clone(),
    };
    event.tests.push(record);
    save_events(&events)?;

    let audit = AuditEntry {
        timestamp: now_epoch_seconds()?,
        action: "test".to_string(),
        scope: scope.clone(),
        target: args.event_id.clone(),
        details: serde_json::json!({
            "name": args.name,
            "result": args.result,
        }),
    };
    append_audit(audit)?;

    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "status": "ok",
            "eventId": args.event_id,
        }))?
    );
    Ok(())
}

fn run_resolve(args: ResolveArgs) -> Result<()> {
    let scope = normalize_scope(&args.scope)?;
    let mut events = load_events()?;
    let event = find_event_mut(&mut events, &args.event_id)?;
    ensure_scope(&scope, &event.scope, "event")?;
    if event.resolution.is_some() {
        anyhow::bail!("event {} is already resolved", args.event_id);
    }

    event.resolution = Some(Resolution {
        resolved_at: now_epoch_seconds()?,
        outcome: args.outcome.clone(),
        evidence: args.evidence.clone(),
    });
    save_events(&events)?;

    let audit = AuditEntry {
        timestamp: now_epoch_seconds()?,
        action: "resolve".to_string(),
        scope: scope.clone(),
        target: args.event_id.clone(),
        details: serde_json::json!({
            "outcome": args.outcome,
            "evidence": args.evidence,
        }),
    };
    append_audit(audit)?;

    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "status": "ok",
            "eventId": args.event_id,
        }))?
    );
    Ok(())
}

fn run_patterns_add(args: PatternsAddArgs) -> Result<()> {
    let scope = normalize_scope(&args.scope)?;
    let mut patterns = load_patterns()?;
    let mut updated = false;

    for pattern in &mut patterns {
        if pattern.name == args.name {
            ensure_scope(&scope, &pattern.scope, "pattern")?;
            pattern.pattern = args.pattern.clone();
            pattern.notes = args.notes.clone();
            pattern.updated_at = now_epoch_seconds()?;
            updated = true;
            break;
        }
    }

    if !updated {
        patterns.push(PatternEntry {
            name: args.name.clone(),
            scope: scope.clone(),
            pattern: args.pattern.clone(),
            notes: args.notes.clone(),
            updated_at: now_epoch_seconds()?,
        });
    }

    save_patterns(&patterns)?;

    let audit = AuditEntry {
        timestamp: now_epoch_seconds()?,
        action: "patterns-add".to_string(),
        scope: scope.clone(),
        target: args.name.clone(),
        details: serde_json::json!({
            "pattern": args.pattern,
            "notes": args.notes,
        }),
    };
    append_audit(audit)?;

    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "status": "ok",
            "pattern": args.name,
        }))?
    );
    Ok(())
}

fn normalize_scope(scope: &str) -> Result<String> {
    let trimmed = scope.trim();
    if trimmed.is_empty() {
        anyhow::bail!("scope must not be empty");
    }
    Ok(trimmed.to_string())
}

fn ensure_scope(expected: &str, actual: &str, entity: &str) -> Result<()> {
    if expected == actual {
        return Ok(());
    }
    anyhow::bail!("{entity} scope mismatch: expected {expected}, got {actual}");
}

fn build_intent(summary: Option<&String>, data: Option<&String>) -> Result<Option<Intent>> {
    if summary.is_none() && data.is_none() {
        return Ok(None);
    }
    let parsed = match data {
        Some(raw) => Some(
            serde_json::from_str::<Value>(raw)
                .with_context(|| format!("failed to parse intent data: {raw}"))?,
        ),
        None => None,
    };
    Ok(Some(Intent {
        summary: summary.cloned(),
        data: parsed,
    }))
}

fn load_events() -> Result<Vec<Event>> {
    let paths = store_paths()?;
    read_json(&paths.events)
}

fn save_events(events: &[Event]) -> Result<()> {
    let paths = store_paths()?;
    write_json(&paths.events, events)
}

fn load_patterns() -> Result<Vec<PatternEntry>> {
    let paths = store_paths()?;
    read_json(&paths.patterns)
}

fn save_patterns(patterns: &[PatternEntry]) -> Result<()> {
    let paths = store_paths()?;
    write_json(&paths.patterns, patterns)
}

fn append_audit(entry: AuditEntry) -> Result<()> {
    let paths = store_paths()?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&paths.audit_log)
        .with_context(|| format!("failed to open audit log {}", paths.audit_log.display()))?;
    let line = serde_json::to_string(&entry)?;
    writeln!(file, "{line}")?;
    Ok(())
}

fn store_paths() -> Result<StorePaths> {
    let codex_home = find_codex_home().context("failed to resolve CODEX_HOME")?;
    let root = codex_home.join("covenant");
    fs::create_dir_all(&root).with_context(|| format!("failed to create {}", root.display()))?;
    Ok(StorePaths {
        events: root.join("events.json"),
        patterns: root.join("patterns.json"),
        audit_log: root.join("audit.jsonl"),
    })
}

fn read_json<T>(path: &Path) -> Result<Vec<T>>
where
    T: for<'de> Deserialize<'de>,
{
    if !path.exists() {
        return Ok(Vec::new());
    }
    let data =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    if data.trim().is_empty() {
        return Ok(Vec::new());
    }
    serde_json::from_str(&data).with_context(|| format!("failed to parse {}", path.display()))
}

fn write_json<T>(path: &Path, payload: &T) -> Result<()>
where
    T: Serialize + ?Sized,
{
    let dir = path
        .parent()
        .context("failed to resolve directory for covenant storage")?;
    let mut temp = tempfile::NamedTempFile::new_in(dir)
        .with_context(|| format!("failed to create temp file in {}", dir.display()))?;
    serde_json::to_writer_pretty(&mut temp, payload)?;
    temp.as_file_mut().write_all(b"\n")?;
    temp.persist(path)
        .map_err(|err| err.error)
        .with_context(|| format!("failed to persist {}", path.display()))?;
    Ok(())
}

fn find_event_mut<'a>(events: &'a mut [Event], event_id: &str) -> Result<&'a mut Event> {
    events
        .iter_mut()
        .find(|event| event.id == event_id)
        .with_context(|| format!("event not found: {event_id}"))
}

fn now_epoch_seconds() -> Result<i64> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system time before unix epoch")?;
    Ok(duration.as_secs() as i64)
}
