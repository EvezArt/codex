use serde::Deserialize;
use serde::Serialize;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::collections::HashSet;

const TEXT_WEIGHT: f64 = 0.4;
const DOMAIN_WEIGHT: f64 = 0.5;
const OUTCOME_WEIGHT: f64 = 0.1;
const MODEL_HIT_THRESHOLD: f64 = 0.55;
const OVERRIDE_KEYWORDS: [&str; 7] = [
    "override",
    "overrode",
    "manual",
    "force",
    "bypass",
    "ignore",
    "workaround",
];
const FAILURE_KEYWORDS: [&str; 6] = ["fail", "failed", "error", "panic", "timeout", "regress"];
const SUCCESS_KEYWORDS: [&str; 6] = [
    "pass",
    "passed",
    "success",
    "fixed",
    "recovered",
    "resolved",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatternDefinition {
    pub id: String,
    pub trigger: String,
    pub invariant: String,
    #[serde(default)]
    pub domain_signature: Vec<f64>,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatternMatchEvent {
    pub trigger: String,
    pub invariant: String,
    #[serde(default)]
    pub domain_signature: Vec<f64>,
    #[serde(default)]
    pub tests: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PatternMatchResult {
    pub pattern_id: String,
    pub text_score: f64,
    pub domain_score: f64,
    pub outcome_affinity: f64,
    pub total: f64,
    pub rationale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatternStatsRecord {
    pub intent: String,
    pub outcome: String,
    #[serde(default)]
    pub tests: Vec<String>,
    #[serde(default)]
    pub evidence: Vec<String>,
    #[serde(default)]
    pub override_applied: bool,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub expected_model: Option<String>,
    #[serde(default)]
    pub started_at: Option<i64>,
    #[serde(default)]
    pub recovered_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PatternStatsSummary {
    pub sample_size: usize,
    pub intent_outcome_fidelity: f64,
    pub override_rate_proxy: f64,
    pub recovery_time_proxy_steps: Option<f64>,
    pub recovery_time_proxy_seconds: Option<f64>,
    pub model_hit_rate: Option<f64>,
    pub heuristic_notes: Vec<String>,
}

pub fn compute_pattern_stats(records: &[PatternStatsRecord]) -> PatternStatsSummary {
    if records.is_empty() {
        return PatternStatsSummary {
            sample_size: 0,
            intent_outcome_fidelity: 0.0,
            override_rate_proxy: 0.0,
            recovery_time_proxy_steps: None,
            recovery_time_proxy_seconds: None,
            model_hit_rate: None,
            heuristic_notes: heuristic_notes(),
        };
    }

    let fidelity_sum = records
        .iter()
        .map(|record| semantic_similarity(record.intent.as_str(), record.outcome.as_str()))
        .sum::<f64>();

    let override_count = records
        .iter()
        .filter(|record| override_detected(record))
        .count();

    let step_recovery = average(
        records
            .iter()
            .filter_map(recovery_steps_proxy)
            .collect::<Vec<_>>()
            .as_slice(),
    );
    let seconds_recovery = average(
        records
            .iter()
            .filter_map(recovery_seconds_proxy)
            .collect::<Vec<_>>()
            .as_slice(),
    );

    let model_hits = records.iter().filter_map(model_hit).collect::<Vec<_>>();

    PatternStatsSummary {
        sample_size: records.len(),
        intent_outcome_fidelity: fidelity_sum / records.len() as f64,
        override_rate_proxy: override_count as f64 / records.len() as f64,
        recovery_time_proxy_steps: step_recovery,
        recovery_time_proxy_seconds: seconds_recovery,
        model_hit_rate: if model_hits.is_empty() {
            None
        } else {
            Some(
                model_hits.iter().copied().filter(|hit| *hit).count() as f64
                    / model_hits.len() as f64,
            )
        },
        heuristic_notes: heuristic_notes(),
    }
}

fn heuristic_notes() -> Vec<String> {
    vec![
        "intentOutcomeFidelity uses lexical cosine + token Jaccard overlap between intent and outcome text".to_string(),
        "overrideRateProxy marks a row as override when overrideApplied=true or tests/evidence/outcome include override-like keywords".to_string(),
        "recoveryTimeProxySeconds uses recoveredAt-startedAt when both timestamps are present and ordered".to_string(),
        "recoveryTimeProxySteps counts test/evidence steps from first failure-like signal to first later success-like signal".to_string(),
        "modelHitRate compares model with expectedModel when provided, otherwise with success-bearing tests/evidence/outcome text".to_string(),
        "all metrics are computed only from the provided rows and their tests/evidence text".to_string(),
    ]
}

fn average(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        None
    } else {
        Some(values.iter().sum::<f64>() / values.len() as f64)
    }
}

fn semantic_similarity(left: &str, right: &str) -> f64 {
    let left_tokens = tokenize(left);
    let right_tokens = tokenize(right);
    let cosine = cosine_similarity_tf(
        &term_frequencies(left_tokens.as_slice()),
        &term_frequencies(right_tokens.as_slice()),
    );
    let jaccard = jaccard_similarity(&token_set(left), &token_set(right));
    (cosine * 0.7 + jaccard * 0.3).clamp(0.0, 1.0)
}

fn override_detected(record: &PatternStatsRecord) -> bool {
    if record.override_applied {
        return true;
    }

    let text = std::iter::once(record.outcome.as_str())
        .chain(record.tests.iter().map(String::as_str))
        .chain(record.evidence.iter().map(String::as_str))
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();

    OVERRIDE_KEYWORDS
        .iter()
        .any(|keyword| text.contains(keyword))
}

fn recovery_steps_proxy(record: &PatternStatsRecord) -> Option<f64> {
    let sequence = record
        .tests
        .iter()
        .chain(record.evidence.iter())
        .map(|step| step.to_ascii_lowercase())
        .collect::<Vec<_>>();

    let first_failure = sequence
        .iter()
        .position(|step| contains_any(step, &FAILURE_KEYWORDS))?;
    let success_after = sequence
        .iter()
        .enumerate()
        .skip(first_failure + 1)
        .find(|(_, step)| contains_any(step, &SUCCESS_KEYWORDS))
        .map(|(idx, _)| idx)?;
    Some((success_after - first_failure) as f64)
}

fn recovery_seconds_proxy(record: &PatternStatsRecord) -> Option<f64> {
    let started_at = record.started_at?;
    let recovered_at = record.recovered_at?;
    if recovered_at >= started_at {
        Some((recovered_at - started_at) as f64)
    } else {
        None
    }
}

fn model_hit(record: &PatternStatsRecord) -> Option<bool> {
    let model = record.model.as_deref()?;
    let reference_text = match record.expected_model.as_deref() {
        Some(expected) => expected.to_string(),
        None => {
            let success_text = record
                .tests
                .iter()
                .chain(record.evidence.iter())
                .filter(|entry| contains_any(&entry.to_ascii_lowercase(), &SUCCESS_KEYWORDS))
                .map(String::as_str)
                .collect::<Vec<_>>();
            if success_text.is_empty() {
                record.outcome.clone()
            } else {
                format!("{} {}", success_text.join(" "), record.outcome)
            }
        }
    };
    Some(semantic_similarity(model, reference_text.as_str()) >= MODEL_HIT_THRESHOLD)
}

fn contains_any(text: &str, keywords: &[&str]) -> bool {
    keywords.iter().any(|keyword| text.contains(keyword))
}

pub fn rank_patterns(
    event: &PatternMatchEvent,
    patterns: &[PatternDefinition],
    limit: usize,
) -> Vec<PatternMatchResult> {
    let event_text = format!(
        "{trigger} {invariant}",
        trigger = event.trigger,
        invariant = event.invariant
    );
    let event_tf = term_frequencies(&tokenize(&event_text));

    let mut results: Vec<PatternMatchResult> = patterns
        .iter()
        .map(|pattern| {
            let pattern_text =
                format!("{trigger} {invariant}", trigger = pattern.trigger, invariant = pattern.invariant);
            let text_score = cosine_similarity_tf(&event_tf, &term_frequencies(&tokenize(&pattern_text)));
            let domain_score = cosine_similarity_vec(&event.domain_signature, &pattern.domain_signature);
            let outcome_affinity = outcome_affinity(&event.tests, &pattern.evidence_refs);
            let total = (text_score * TEXT_WEIGHT
                + domain_score * DOMAIN_WEIGHT
                + outcome_affinity * OUTCOME_WEIGHT)
                .clamp(0.0, 1.0);
            let rationale = format!(
                "text={text_score:.2} domain={domain_score:.2} outcome_affinity={outcome_affinity:.2} total={total:.2}",
                text_score = text_score,
                domain_score = domain_score,
                outcome_affinity = outcome_affinity,
                total = total
            );
            PatternMatchResult {
                pattern_id: pattern.id.clone(),
                text_score,
                domain_score,
                outcome_affinity,
                total,
                rationale,
            }
        })
        .collect();

    results.sort_by(|left, right| {
        right
            .total
            .partial_cmp(&left.total)
            .unwrap_or(Ordering::Equal)
            .then_with(|| left.pattern_id.cmp(&right.pattern_id))
    });

    if results.len() > limit {
        results.truncate(limit);
    }

    results
}

fn tokenize(text: &str) -> Vec<String> {
    text.split(|ch: char| !ch.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(|token| token.to_ascii_lowercase())
        .collect()
}

fn term_frequencies(tokens: &[String]) -> HashMap<String, f64> {
    let mut counts = HashMap::new();
    for token in tokens {
        let entry = counts.entry(token.clone()).or_insert(0.0);
        *entry += 1.0;
    }
    counts
}

fn cosine_similarity_tf(left: &HashMap<String, f64>, right: &HashMap<String, f64>) -> f64 {
    if left.is_empty() || right.is_empty() {
        return 0.0;
    }

    let mut dot = 0.0;
    for (token, value) in left {
        if let Some(other) = right.get(token) {
            dot += value * other;
        }
    }

    let left_norm = left.values().map(|value| value * value).sum::<f64>().sqrt();
    let right_norm = right
        .values()
        .map(|value| value * value)
        .sum::<f64>()
        .sqrt();
    if left_norm == 0.0 || right_norm == 0.0 {
        0.0
    } else {
        dot / (left_norm * right_norm)
    }
}

fn cosine_similarity_vec(left: &[f64], right: &[f64]) -> f64 {
    let len = left.len().min(right.len());
    if len == 0 {
        return 0.0;
    }

    let mut dot = 0.0;
    let mut left_norm = 0.0;
    let mut right_norm = 0.0;

    for idx in 0..len {
        let left_value = left[idx];
        let right_value = right[idx];
        dot += left_value * right_value;
        left_norm += left_value * left_value;
        right_norm += right_value * right_value;
    }

    if left_norm == 0.0 || right_norm == 0.0 {
        0.0
    } else {
        dot / (left_norm.sqrt() * right_norm.sqrt())
    }
}

fn outcome_affinity(tests: &[String], evidence_refs: &[String]) -> f64 {
    if tests.is_empty() || evidence_refs.is_empty() {
        return 0.0;
    }

    let mut best = 0.0;
    for test in tests {
        let test_tokens = token_set(test);
        for evidence in evidence_refs {
            let score = jaccard_similarity(&test_tokens, &token_set(evidence));
            if score > best {
                best = score;
            }
        }
    }
    best
}

fn token_set(text: &str) -> HashSet<String> {
    tokenize(text).into_iter().collect()
}

fn jaccard_similarity(left: &HashSet<String>, right: &HashSet<String>) -> f64 {
    if left.is_empty() || right.is_empty() {
        return 0.0;
    }

    let intersection = left.intersection(right).count() as f64;
    let union = (left.len() + right.len()) as f64 - intersection;
    if union == 0.0 {
        0.0
    } else {
        intersection / union
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn ranks_patterns_by_scores() {
        let event = PatternMatchEvent {
            trigger: "compile error".to_string(),
            invariant: "missing import".to_string(),
            domain_signature: vec![1.0, 0.0, 0.0],
            tests: vec!["test_parser failed".to_string()],
        };

        let patterns = vec![
            PatternDefinition {
                id: "pattern-a".to_string(),
                trigger: "compile error".to_string(),
                invariant: "missing import".to_string(),
                domain_signature: vec![0.9, 0.1, 0.0],
                evidence_refs: vec!["test_parser failed".to_string()],
            },
            PatternDefinition {
                id: "pattern-b".to_string(),
                trigger: "runtime error".to_string(),
                invariant: "panic".to_string(),
                domain_signature: vec![0.0, 1.0, 0.0],
                evidence_refs: vec!["test_runtime failed".to_string()],
            },
        ];

        let results = rank_patterns(&event, &patterns, 2);
        let ids: Vec<&str> = results
            .iter()
            .map(|result| result.pattern_id.as_str())
            .collect();
        assert_eq!(ids, vec!["pattern-a", "pattern-b"]);
    }

    #[test]
    fn empty_domain_signature_scores_zero() {
        let score = cosine_similarity_vec(&[], &[1.0, 0.5]);
        assert_eq!(score, 0.0);
    }

    #[test]
    fn computes_pattern_stats_from_records() {
        let records = vec![
            PatternStatsRecord {
                intent: "Route audio to bluetooth".to_string(),
                outcome: "Bluetooth route recovered and audio passed".to_string(),
                tests: vec![
                    "audio route failed initially".to_string(),
                    "audio route passed".to_string(),
                ],
                evidence: vec!["manual override applied once".to_string()],
                override_applied: false,
                model: Some("gpt-4.1".to_string()),
                expected_model: Some("gpt-4.1".to_string()),
                started_at: Some(10),
                recovered_at: Some(20),
            },
            PatternStatsRecord {
                intent: "Fix parser panic".to_string(),
                outcome: "Parser fixed and tests passed".to_string(),
                tests: vec![
                    "parser error observed".to_string(),
                    "parser test passed".to_string(),
                ],
                evidence: vec!["trace://evidence/1".to_string()],
                override_applied: false,
                model: Some("gpt-4.1".to_string()),
                expected_model: Some("gpt-4.1".to_string()),
                started_at: Some(100),
                recovered_at: Some(130),
            },
        ];

        let summary = compute_pattern_stats(&records);

        assert_eq!(summary.sample_size, 2);
        assert_eq!(summary.model_hit_rate, Some(1.0));
        assert_eq!(summary.override_rate_proxy, 0.5);
        assert_eq!(summary.recovery_time_proxy_steps, Some(1.0));
        assert_eq!(summary.recovery_time_proxy_seconds, Some(20.0));
    }
}
