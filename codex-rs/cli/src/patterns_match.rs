use anyhow::Context;
use clap::Parser;
use codex_core::pattern_match::PatternDefinition;
use codex_core::pattern_match::PatternMatchEvent;
use codex_core::pattern_match::PatternStatsRecord;
use codex_core::pattern_match::compute_pattern_stats;
use codex_core::pattern_match::rank_patterns;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug, Parser)]
pub struct PatternsMatchCommand {
    /// JSON file containing an array of stored patterns.
    #[arg(long, value_name = "FILE")]
    pub patterns: PathBuf,

    /// JSON file describing the event to match.
    #[arg(long, value_name = "FILE")]
    pub event: PathBuf,

    /// Maximum number of matches to print.
    #[arg(long, default_value_t = 5)]
    pub limit: usize,
}

#[derive(Debug, Parser)]
pub struct PatternsStatsCommand {
    /// JSON file containing an array of stats records.
    #[arg(long, value_name = "FILE")]
    pub records: PathBuf,

    /// Print output as pretty JSON.
    #[arg(long, default_value_t = false)]
    pub json: bool,
}

pub fn run_patterns_match(cmd: PatternsMatchCommand) -> anyhow::Result<()> {
    let patterns: Vec<PatternDefinition> = read_json(&cmd.patterns)?;
    let event: PatternMatchEvent = read_json(&cmd.event)?;

    let results = rank_patterns(&event, &patterns, cmd.limit);
    for result in results {
        println!("{} {}", result.pattern_id, result.rationale);
    }

    Ok(())
}

pub fn run_patterns_stats(cmd: PatternsStatsCommand) -> anyhow::Result<()> {
    let records: Vec<PatternStatsRecord> = read_json(&cmd.records)?;
    let summary = compute_pattern_stats(&records);

    if cmd.json {
        println!("{}", serde_json::to_string_pretty(&summary)?);
        return Ok(());
    }

    println!("Sample size: {}", summary.sample_size);
    println!(
        "Intent→outcome fidelity: {:.3}",
        summary.intent_outcome_fidelity
    );
    println!("Override-rate proxy: {:.3}", summary.override_rate_proxy);
    if let Some(steps) = summary.recovery_time_proxy_steps {
        println!("Recovery-time proxy (steps): {:.3}", steps);
    } else {
        println!("Recovery-time proxy (steps): n/a");
    }
    if let Some(seconds) = summary.recovery_time_proxy_seconds {
        println!("Recovery-time proxy (seconds): {:.3}", seconds);
    } else {
        println!("Recovery-time proxy (seconds): n/a");
    }
    if let Some(hit_rate) = summary.model_hit_rate {
        println!("Model hit-rate: {:.3}", hit_rate);
    } else {
        println!("Model hit-rate: n/a");
    }
    println!("Heuristics:");
    for note in summary.heuristic_notes {
        println!("- {note}");
    }

    Ok(())
}

fn read_json<T>(path: &Path) -> anyhow::Result<T>
where
    T: serde::de::DeserializeOwned,
{
    let contents = fs::read_to_string(path)
        .with_context(|| format!("failed to read {path}", path = path.display()))?;
    serde_json::from_str(&contents)
        .with_context(|| format!("failed to parse JSON from {path}", path = path.display()))
}
