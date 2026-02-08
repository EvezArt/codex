use anyhow::Context;
use anyhow::Result;
use clap::Parser;
use codex_core::config::find_codex_home;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

const DEFAULT_EVENTS_FILE: &str = "resolved_events.jsonl";
const DEFAULT_PATTERNS_FILE: &str = "patterns.jsonl";
const DEFAULT_AUDIT_FILE: &str = "audit.jsonl";
const MIN_EVIDENCE_COUNT: usize = 2;

#[derive(Debug, Parser)]
pub struct CompileCommand {
    /// Path to resolved events JSONL.
    #[arg(long, value_name = "FILE")]
    events: Option<PathBuf>,

    /// Path to patterns JSONL file.
    #[arg(long, value_name = "FILE")]
    patterns: Option<PathBuf>,

    /// Path to audit JSONL file.
    #[arg(long, value_name = "FILE")]
    audit: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
struct ResolvedEvent {
    trigger: String,
    invariant: String,
    response: String,
    #[serde(default)]
    evidence: Option<String>,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct PatternKey {
    trigger_key: String,
    invariant_key: String,
    response_key: String,
}

#[derive(Debug)]
struct PatternGroup {
    trigger: String,
    invariant: String,
    response: String,
    trigger_signature: String,
    evidence: Vec<String>,
    total_events: usize,
}

#[derive(Debug, Serialize)]
struct SuggestedPattern {
    trigger: String,
    invariant: String,
    response: String,
    trigger_signature: String,
    evidence: Vec<String>,
    evidence_count: usize,
    total_events: usize,
    compiled_at: i64,
}

#[derive(Debug, Deserialize)]
struct ExistingPattern {
    trigger: String,
    invariant: String,
    response: String,
    #[serde(default)]
    trigger_signature: Option<String>,
}

#[derive(Debug, Serialize)]
struct AuditAction {
    action: String,
    events_scanned: usize,
    patterns_written: usize,
    patterns_path: String,
    compiled_at: i64,
}

impl CompileCommand {
    pub fn run(self) -> Result<()> {
        let codex_home = find_codex_home().context("failed to resolve CODEX_HOME")?;
        let events_path = self
            .events
            .unwrap_or_else(|| codex_home.join(DEFAULT_EVENTS_FILE));
        let patterns_path = self
            .patterns
            .unwrap_or_else(|| codex_home.join(DEFAULT_PATTERNS_FILE));
        let audit_path = self
            .audit
            .unwrap_or_else(|| codex_home.join(DEFAULT_AUDIT_FILE));

        let events = read_resolved_events(&events_path)?;
        let (_suggested, patterns_written) = compile_patterns(&events, &patterns_path)?;
        write_audit_entry(&audit_path, events.len(), patterns_written, &patterns_path)?;
        Ok(())
    }
}

fn read_resolved_events(path: &Path) -> Result<Vec<ResolvedEvent>> {
    let file = File::open(path).with_context(|| {
        format!(
            "failed to open resolved events file {path}",
            path = path.display()
        )
    })?;
    let reader = BufReader::new(file);
    let mut events = Vec::new();

    for (line_index, line) in reader.lines().enumerate() {
        let line = line.with_context(|| {
            format!(
                "failed to read resolved events file {path} at line {line}",
                path = path.display(),
                line = line_index + 1
            )
        })?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let event: ResolvedEvent = serde_json::from_str(trimmed).with_context(|| {
            format!(
                "failed to parse resolved event from {path} at line {line}",
                path = path.display(),
                line = line_index + 1
            )
        })?;
        events.push(event);
    }

    Ok(events)
}

fn compile_patterns(
    events: &[ResolvedEvent],
    patterns_path: &Path,
) -> Result<(Vec<SuggestedPattern>, usize)> {
    let mut groups: HashMap<PatternKey, PatternGroup> = HashMap::new();
    for event in events {
        let normalized_trigger = normalize_text(&event.trigger);
        let trigger_signature = keyword_signature(&normalized_trigger);
        let trigger_key = select_trigger_key(&normalized_trigger, &trigger_signature);
        let invariant_key = normalize_text(&event.invariant);
        let response_key = normalize_text(&event.response);
        let key = PatternKey {
            trigger_key: trigger_key.clone(),
            invariant_key: invariant_key.clone(),
            response_key: response_key.clone(),
        };

        let group = groups.entry(key).or_insert_with(|| PatternGroup {
            trigger: event.trigger.clone(),
            invariant: event.invariant.clone(),
            response: event.response.clone(),
            trigger_signature: trigger_signature.clone(),
            evidence: Vec::new(),
            total_events: 0,
        });
        group.total_events += 1;
        if let Some(evidence) = clean_evidence(event.evidence.as_deref()) {
            group.evidence.push(evidence);
        }
    }

    let existing_keys = load_existing_pattern_keys(patterns_path)?;
    let compiled_at = unix_timestamp();
    let mut suggested = Vec::new();
    for (key, group) in groups {
        if group.evidence.len() < MIN_EVIDENCE_COUNT {
            continue;
        }
        if existing_keys.contains(&key) {
            continue;
        }
        let evidence_count = group.evidence.len();
        suggested.push(SuggestedPattern {
            trigger: group.trigger,
            invariant: group.invariant,
            response: group.response,
            trigger_signature: group.trigger_signature,
            evidence: group.evidence,
            evidence_count,
            total_events: group.total_events,
            compiled_at,
        });
    }

    if suggested.is_empty() {
        return Ok((suggested, 0));
    }

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(patterns_path)
        .with_context(|| {
            format!(
                "failed to open patterns file {path}",
                path = patterns_path.display()
            )
        })?;

    for pattern in &suggested {
        let line = serde_json::to_string(pattern).context("failed to serialize pattern")?;
        writeln!(file, "{line}").context("failed to write pattern line")?;
    }

    let suggested_count = suggested.len();
    Ok((suggested, suggested_count))
}

fn load_existing_pattern_keys(path: &Path) -> Result<HashSet<PatternKey>> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok(HashSet::new());
        }
        Err(err) => {
            return Err(err).with_context(|| {
                format!("failed to open patterns file {path}", path = path.display())
            })
        }
    };

    let reader = BufReader::new(file);
    let mut keys = HashSet::new();
    for (line_index, line) in reader.lines().enumerate() {
        let line = line.with_context(|| {
            format!(
                "failed to read patterns file {path} at line {line}",
                path = path.display(),
                line = line_index + 1
            )
        })?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let pattern: ExistingPattern = serde_json::from_str(trimmed).with_context(|| {
            format!(
                "failed to parse pattern from {path} at line {line}",
                path = path.display(),
                line = line_index + 1
            )
        })?;
        let normalized_trigger = normalize_text(&pattern.trigger);
        let trigger_signature = pattern.trigger_signature.unwrap_or_else(|| {
            keyword_signature(&normalized_trigger)
        });
        keys.insert(PatternKey {
            trigger_key: select_trigger_key(&normalized_trigger, &trigger_signature),
            invariant_key: normalize_text(&pattern.invariant),
            response_key: normalize_text(&pattern.response),
        });
    }

    Ok(keys)
}

fn write_audit_entry(
    path: &Path,
    events_scanned: usize,
    patterns_written: usize,
    patterns_path: &Path,
) -> Result<()> {
    let compiled_at = unix_timestamp();
    let entry = AuditAction {
        action: "compile_patterns".to_string(),
        events_scanned,
        patterns_written,
        patterns_path: patterns_path.display().to_string(),
        compiled_at,
    };
    let line = serde_json::to_string(&entry).context("failed to serialize audit entry")?;

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("failed to open audit file {path}", path = path.display()))?;
    writeln!(file, "{line}").context("failed to write audit entry")?;
    Ok(())
}

fn normalize_text(input: &str) -> String {
    let mut normalized = String::with_capacity(input.len());
    let mut last_was_space = false;
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            normalized.push(ch.to_ascii_lowercase());
            last_was_space = false;
        } else if !last_was_space {
            normalized.push(' ');
            last_was_space = true;
        }
    }
    normalized.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn keyword_signature(normalized: &str) -> String {
    let mut keywords: Vec<&str> = normalized
        .split_whitespace()
        .filter(|word| word.len() > 2)
        .collect();
    keywords.sort_unstable();
    keywords.dedup();
    keywords.truncate(6);
    keywords.join("|")
}

fn select_trigger_key(normalized: &str, signature: &str) -> String {
    let word_count = normalized.split_whitespace().count();
    if word_count <= 6 {
        normalized.to_string()
    } else if signature.is_empty() {
        normalized.to_string()
    } else {
        signature.to_string()
    }
}

fn clean_evidence(value: Option<&str>) -> Option<String> {
    let trimmed = value.map(str::trim)?;
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn unix_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .try_into()
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::keyword_signature;
    use super::normalize_text;
    use super::select_trigger_key;
    use pretty_assertions::assert_eq;

    #[test]
    fn normalize_text_strips_punctuation() {
        let normalized = normalize_text("Fix: Foo/Bar? 100% ready.");
        assert_eq!(normalized, "fix foo bar 100 ready");
    }

    #[test]
    fn keyword_signature_dedupes_and_sorts() {
        let signature = keyword_signature("compile compile event response");
        assert_eq!(signature, "compile|event|response");
    }

    #[test]
    fn select_trigger_key_prefers_phrase_for_short_inputs() {
        let normalized = "short trigger phrase";
        let signature = keyword_signature(normalized);
        let key = select_trigger_key(normalized, &signature);
        assert_eq!(key, normalized);
    }
}
