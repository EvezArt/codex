use std::collections::HashSet;
use std::fs;
use std::fs::File;
use std::io::BufRead;
use std::io::BufReader;
use std::path::Path;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use chrono::DateTime;
use chrono::Utc;
use clap::Parser;
use codex_core::ARCHIVED_SESSIONS_SUBDIR;
use codex_core::SESSIONS_SUBDIR;
use codex_core::config::find_codex_home;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::RolloutLine;
use codex_protocol::protocol::TurnContextItem;
use codex_protocol::protocol::USER_MESSAGE_BEGIN;
use serde::Serialize;

const DEFAULT_HIT_THRESHOLD: f64 = 0.2;

#[derive(Debug, Parser)]
pub struct StatsCommand {
    /// Rollout JSONL files to analyze.
    #[arg(value_name = "ROLL_OUT_PATH")]
    pub paths: Vec<PathBuf>,

    /// Path to CODEX_HOME (defaults to $CODEX_HOME or ~/.codex).
    #[arg(long, env = "CODEX_HOME")]
    pub codex_home: Option<PathBuf>,

    /// Analyze all sessions under CODEX_HOME instead of only the latest.
    #[arg(long, default_value_t = false)]
    pub all: bool,

    /// Similarity threshold for model hit-rate.
    #[arg(long, default_value_t = DEFAULT_HIT_THRESHOLD)]
    pub hit_threshold: f64,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct TurnContextSnapshot {
    model: String,
    approval_policy: String,
    sandbox_policy: String,
    personality: Option<String>,
}

#[derive(Debug, Default)]
struct TurnRecord {
    user_message: String,
    outcome_message: Option<String>,
    outcome_ts: Option<DateTime<Utc>>,
    context: Option<TurnContextSnapshot>,
    last_agent_message: Option<String>,
}

#[derive(Debug, Default)]
struct StatsAggregate {
    total_turns: usize,
    turns_with_outcome: usize,
    similarity_sum: f64,
    hit_count: usize,
    override_turns: usize,
    override_denominator: usize,
    recovery_samples_ms: Vec<i64>,
}

pub fn run_stats(cmd: StatsCommand) -> Result<()> {
    let mut paths = resolve_paths(&cmd)?;
    if paths.is_empty() {
        return Err(anyhow::anyhow!(
            "no rollout files found (pass paths or check CODEX_HOME)"
        ));
    }

    paths.sort();
    let mut aggregate = StatsAggregate::default();
    for path in &paths {
        let per_file = analyze_rollout(path, cmd.hit_threshold)
            .with_context(|| format!("failed to analyze {}", path.display()))?;
        merge_stats(&mut aggregate, per_file);
    }

    print_summary(&aggregate, &paths, cmd.hit_threshold);
    Ok(())
}

fn resolve_paths(cmd: &StatsCommand) -> Result<Vec<PathBuf>> {
    if !cmd.paths.is_empty() {
        return Ok(cmd.paths.clone());
    }

    let codex_home = cmd
        .codex_home
        .clone()
        .or_else(|| find_codex_home().ok())
        .unwrap_or_else(|| PathBuf::from(".codex"));
    let mut roots = Vec::new();
    roots.push(codex_home.join(SESSIONS_SUBDIR));
    roots.push(codex_home.join(ARCHIVED_SESSIONS_SUBDIR));

    let mut paths = Vec::new();
    for root in roots {
        if root.exists() {
            collect_jsonl_files(&root, &mut paths)?;
        }
    }

    if cmd.all {
        return Ok(paths);
    }

    let latest = select_latest(&paths)?;
    Ok(latest.into_iter().collect())
}

fn collect_jsonl_files(root: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    let entries = fs::read_dir(root)
        .with_context(|| format!("failed to read directory {}", root.display()))?;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_jsonl_files(&path, out)?;
        } else if path.extension().is_some_and(|ext| ext == "jsonl") {
            out.push(path);
        }
    }
    Ok(())
}

fn select_latest(paths: &[PathBuf]) -> Result<Option<PathBuf>> {
    let mut latest_path = None;
    let mut latest_mtime = None;
    for path in paths {
        let metadata = fs::metadata(path)?;
        let modified = metadata.modified().ok();
        if latest_mtime.is_none() || modified > latest_mtime {
            latest_mtime = modified;
            latest_path = Some(path.clone());
        }
    }
    Ok(latest_path)
}

fn analyze_rollout(path: &Path, hit_threshold: f64) -> Result<StatsAggregate> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut aggregate = StatsAggregate::default();
    let mut turns: Vec<TurnRecord> = Vec::new();
    let mut current_context: Option<TurnContextSnapshot> = None;
    let mut baseline_context: Option<TurnContextSnapshot> = None;
    let mut pending_recovery_start: Option<DateTime<Utc>> = None;

    for (line_idx, line) in reader.lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let record: RolloutLine = serde_json::from_str(&line)
            .with_context(|| format!("line {} not valid JSON", line_idx + 1))?;
        let timestamp = parse_ts(record.timestamp.as_str());

        match record.item {
            RolloutItem::TurnContext(ctx) => {
                current_context = Some(snapshot_context(&ctx));
            }
            RolloutItem::EventMsg(EventMsg::UserMessage(user)) => {
                let Some(message) = clean_user_message(user.message.as_str()) else {
                    continue;
                };
                turns.push(TurnRecord {
                    user_message: message,
                    outcome_message: None,
                    outcome_ts: None,
                    context: current_context.clone(),
                    last_agent_message: None,
                });
            }
            RolloutItem::EventMsg(EventMsg::AgentMessage(agent)) => {
                if let Some(turn) = turns.last_mut() {
                    turn.last_agent_message = Some(agent.message);
                }
            }
            RolloutItem::EventMsg(EventMsg::TurnComplete(complete)) => {
                if let Some(turn) = turns.last_mut() {
                    let candidate = complete
                        .last_agent_message
                        .or_else(|| turn.last_agent_message.clone());
                    turn.outcome_message = candidate;
                    turn.outcome_ts = timestamp;
                }
                if let (Some(start), Some(end)) = (pending_recovery_start, timestamp) {
                    aggregate
                        .recovery_samples_ms
                        .push((end - start).num_milliseconds());
                    pending_recovery_start = None;
                }
            }
            RolloutItem::EventMsg(EventMsg::TurnStarted(_)) => {
                if let (Some(start), Some(end)) = (pending_recovery_start, timestamp) {
                    aggregate
                        .recovery_samples_ms
                        .push((end - start).num_milliseconds());
                    pending_recovery_start = None;
                }
            }
            RolloutItem::EventMsg(EventMsg::Error(_))
            | RolloutItem::EventMsg(EventMsg::StreamError(_))
            | RolloutItem::EventMsg(EventMsg::TurnAborted(_)) => {
                if pending_recovery_start.is_none() {
                    pending_recovery_start = timestamp;
                }
            }
            RolloutItem::EventMsg(_) | RolloutItem::ResponseItem(_) | RolloutItem::Compacted(_)
            | RolloutItem::SessionMeta(_) => {}
        }
    }

    for turn in turns {
        aggregate.total_turns += 1;
        if let Some(context) = turn.context.as_ref() {
            if baseline_context.is_none() {
                baseline_context = Some(context.clone());
            }
            aggregate.override_denominator += 1;
            if baseline_context.as_ref() != Some(context) {
                aggregate.override_turns += 1;
            }
        }

        let Some(outcome) = turn.outcome_message.as_deref() else {
            continue;
        };
        aggregate.turns_with_outcome += 1;
        let similarity = semantic_similarity(turn.user_message.as_str(), outcome);
        aggregate.similarity_sum += similarity;
        if similarity >= hit_threshold {
            aggregate.hit_count += 1;
        }
    }

    Ok(aggregate)
}

fn merge_stats(target: &mut StatsAggregate, incoming: StatsAggregate) {
    target.total_turns += incoming.total_turns;
    target.turns_with_outcome += incoming.turns_with_outcome;
    target.similarity_sum += incoming.similarity_sum;
    target.hit_count += incoming.hit_count;
    target.override_turns += incoming.override_turns;
    target.override_denominator += incoming.override_denominator;
    target
        .recovery_samples_ms
        .extend(incoming.recovery_samples_ms);
}

fn print_summary(aggregate: &StatsAggregate, paths: &[PathBuf], hit_threshold: f64) {
    let fidelity = if aggregate.turns_with_outcome == 0 {
        0.0
    } else {
        aggregate.similarity_sum / aggregate.turns_with_outcome as f64
    };
    let hit_rate = if aggregate.turns_with_outcome == 0 {
        0.0
    } else {
        aggregate.hit_count as f64 / aggregate.turns_with_outcome as f64 * 100.0
    };
    let override_rate = if aggregate.override_denominator == 0 {
        0.0
    } else {
        aggregate.override_turns as f64 / aggregate.override_denominator as f64 * 100.0
    };
    let avg_recovery_ms = if aggregate.recovery_samples_ms.is_empty() {
        None
    } else {
        let total: i64 = aggregate.recovery_samples_ms.iter().sum();
        Some(total / aggregate.recovery_samples_ms.len() as i64)
    };

    println!("Codex stats");
    println!("files: {}", paths.len());
    println!("turns: {}", aggregate.total_turns);
    println!("turns with outcomes: {}", aggregate.turns_with_outcome);
    println!("intent->outcome fidelity: {fidelity:.3}");
    println!("model hit-rate (>= {hit_threshold:.2}): {hit_rate:.1}%");
    println!(
        "override-rate proxy: {override_rate:.1}% ({}/{})",
        aggregate.override_turns, aggregate.override_denominator
    );
    if let Some(avg) = avg_recovery_ms {
        println!(
            "recovery-time proxy: {avg}ms (n={})",
            aggregate.recovery_samples_ms.len()
        );
    } else {
        println!("recovery-time proxy: n/a");
    }

    println!("\nHeuristics");
    println!(
        "- intent->outcome fidelity = mean Jaccard overlap of normalized tokens (user message vs. final agent message)."
    );
    println!(
        "- model hit-rate = share of turns where similarity >= {hit_threshold:.2}."
    );
    println!(
        "- override-rate proxy = share of turns whose context differs from the first observed turn (model, approval, sandbox, personality)."
    );
    println!(
        "- recovery-time proxy = time from Error/StreamError/TurnAborted to next TurnStarted or TurnComplete."
    );
    println!(
        "- data sources = local rollout JSONL files only (no network or external inference)."
    );
    println!("\nData sources");
    for path in paths {
        println!("- {}", path.display());
    }
}

fn parse_ts(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

fn clean_user_message(message: &str) -> Option<String> {
    let trimmed = strip_user_message_prefix(message).trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn strip_user_message_prefix(text: &str) -> &str {
    match text.find(USER_MESSAGE_BEGIN) {
        Some(idx) => text[idx + USER_MESSAGE_BEGIN.len()..].trim(),
        None => text.trim(),
    }
}

fn snapshot_context(ctx: &TurnContextItem) -> TurnContextSnapshot {
    TurnContextSnapshot {
        model: ctx.model.clone(),
        approval_policy: enum_to_string(&ctx.approval_policy),
        sandbox_policy: enum_to_string(&ctx.sandbox_policy),
        personality: ctx.personality.as_ref().map(enum_to_string),
    }
}

fn enum_to_string<T: Serialize>(value: &T) -> String {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(s)) => s,
        Ok(other) => other.to_string(),
        Err(_) => String::new(),
    }
}

fn semantic_similarity(intent: &str, outcome: &str) -> f64 {
    let left = semantic_fingerprint(intent);
    let right = semantic_fingerprint(outcome);
    if left.is_empty() && right.is_empty() {
        return 0.0;
    }
    let intersection = left.intersection(&right).count();
    let union = left.union(&right).count();
    if union == 0 {
        0.0
    } else {
        intersection as f64 / union as f64
    }
}

fn semantic_fingerprint(text: &str) -> HashSet<String> {
    let tokens = tokenize(text);
    let mut set = HashSet::new();
    for token in &tokens {
        set.insert(token.clone());
    }
    for pair in tokens.windows(2) {
        if let [first, second] = pair {
            set.insert(format!("{first}_{second}"));
        }
    }
    set
}

fn tokenize(text: &str) -> Vec<String> {
    let mut cleaned = String::with_capacity(text.len());
    for ch in text.chars() {
        if ch.is_alphanumeric() {
            for lower in ch.to_lowercase() {
                cleaned.push(lower);
            }
        } else {
            cleaned.push(' ');
        }
    }

    cleaned
        .split_whitespace()
        .map(stem_token)
        .filter(|token| !token.is_empty() && !is_stopword(token))
        .collect()
}

fn stem_token(token: &str) -> String {
    let trimmed = token.trim_matches('\'');
    if trimmed.len() <= 3 {
        return trimmed.to_string();
    }
    let stripped = trimmed
        .strip_suffix("ing")
        .or_else(|| trimmed.strip_suffix("ed"))
        .or_else(|| trimmed.strip_suffix("es"))
        .or_else(|| trimmed.strip_suffix('s'))
        .unwrap_or(trimmed);
    let mut normalized = stripped.to_string();
    if normalized.len() > 2 {
        let last_two: Vec<char> = normalized.chars().rev().take(2).collect();
        if last_two.len() == 2 && last_two[0] == last_two[1] {
            normalized.pop();
        }
    }
    normalized
}

fn is_stopword(token: &str) -> bool {
    matches!(
        token,
        "a"
            | "an"
            | "and"
            | "are"
            | "as"
            | "at"
            | "be"
            | "by"
            | "for"
            | "from"
            | "has"
            | "have"
            | "in"
            | "is"
            | "it"
            | "its"
            | "of"
            | "on"
            | "or"
            | "that"
            | "the"
            | "their"
            | "this"
            | "to"
            | "was"
            | "were"
            | "will"
            | "with"
            | "you"
            | "your"
            | "please"
            | "help"
            | "make"
            | "use"
            | "do"
            | "does"
            | "did"
            | "can"
            | "could"
            | "should"
            | "would"
    )
}

#[cfg(test)]
mod tests {
    use super::semantic_similarity;
    use super::stem_token;
    use super::tokenize;
    use pretty_assertions::assert_eq;

    #[test]
    fn stem_token_strips_suffixes() {
        assert_eq!(stem_token("running"), "run");
        assert_eq!(stem_token("tested"), "test");
        assert_eq!(stem_token("boxes"), "box");
        assert_eq!(stem_token("cats"), "cat");
    }

    #[test]
    fn tokenize_removes_stopwords() {
        let tokens = tokenize("Please add the tests for the user.");
        assert_eq!(tokens, vec!["add".to_string(), "test".to_string(), "user".to_string()]);
    }

    #[test]
    fn semantic_similarity_scores_overlap() {
        let score = semantic_similarity("add the tests", "added tests for module");
        assert!(score > 0.1);
    }
}
