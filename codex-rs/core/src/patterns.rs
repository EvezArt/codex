use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Outcome {
    Success,
    Failure,
    Mixed,
    Unknown,
    Other(String),
}

impl fmt::Display for Outcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Outcome::Success => write!(f, "success"),
            Outcome::Failure => write!(f, "failure"),
            Outcome::Mixed => write!(f, "mixed"),
            Outcome::Unknown => write!(f, "unknown"),
            Outcome::Other(value) => write!(f, "{value}"),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedEvent {
    pub id: String,
    pub trigger: String,
    pub invariant: Option<String>,
    pub outcome: Outcome,
    pub response: Option<String>,
    pub domain_signature: Vec<f32>,
    pub evidence: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Pattern {
    pub trigger: String,
    pub invariant: Option<String>,
    pub counterexample: Option<String>,
    pub best_response: Option<String>,
    pub domain_signature: Vec<f32>,
    pub supporting_evidence: Vec<String>,
    pub outcome: Outcome,
    pub support_count: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EventForMatch {
    pub trigger: String,
    pub invariant: Option<String>,
    pub domain_signature: Vec<f32>,
    pub desired_outcome: Option<Outcome>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PatternMatch {
    pub pattern: Pattern,
    pub score: f32,
    pub text_similarity: f32,
    pub domain_similarity: f32,
    pub outcome_affinity: f32,
    pub rationale: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct PatternKey {
    trigger: String,
    invariant: Option<String>,
}

const TEXT_WEIGHT: f32 = 0.5;
const DOMAIN_WEIGHT: f32 = 0.3;
const OUTCOME_WEIGHT: f32 = 0.2;

pub fn compile(events: &[ResolvedEvent]) -> Vec<Pattern> {
    let mut groups: HashMap<PatternKey, Vec<&ResolvedEvent>> = HashMap::new();
    for event in events {
        let key = PatternKey {
            trigger: normalize_text(event.trigger.as_str()),
            invariant: event
                .invariant
                .as_ref()
                .map(|value| normalize_text(value.as_str())),
        };
        groups.entry(key).or_default().push(event);
    }

    let mut patterns = Vec::new();
    for group in groups.values() {
        if group.len() < 2 {
            continue;
        }
        let trigger = most_common_string(group.iter().map(|event| event.trigger.as_str()))
            .unwrap_or_default();
        let invariant = most_common_optional_string(group.iter().map(|event| event.invariant.as_ref()));
        let outcome = dominant_outcome(group);
        let best_response =
            most_common_optional_string(group.iter().map(|event| event.response.as_ref()));
        let counterexample = select_counterexample(group, &outcome);
        let domain_signature = average_signature(group.iter().map(|event| &event.domain_signature));
        let supporting_evidence = collect_supporting_evidence(group);

        patterns.push(Pattern {
            trigger,
            invariant,
            counterexample,
            best_response,
            domain_signature,
            supporting_evidence,
            outcome,
            support_count: group.len(),
        });
    }

    patterns.sort_by(|a, b| b.support_count.cmp(&a.support_count));
    patterns
}

pub fn patterns_match(event: &EventForMatch, patterns: &[Pattern]) -> Vec<PatternMatch> {
    let mut matches = patterns
        .iter()
        .cloned()
        .map(|pattern| {
            let (text_similarity, trigger_similarity, invariant_similarity) =
                compute_text_similarity(event, &pattern);
            let domain_similarity =
                cosine_similarity(event.domain_signature.as_slice(), pattern.domain_signature.as_slice());
            let outcome_affinity = compute_outcome_affinity(event.desired_outcome.as_ref(), &pattern.outcome);
            let score = text_similarity * TEXT_WEIGHT
                + domain_similarity * DOMAIN_WEIGHT
                + outcome_affinity * OUTCOME_WEIGHT;
            let rationale = build_rationale(
                trigger_similarity,
                invariant_similarity,
                domain_similarity,
                outcome_affinity,
                event.desired_outcome.as_ref(),
                &pattern.outcome,
            );

            PatternMatch {
                pattern,
                score,
                text_similarity,
                domain_similarity,
                outcome_affinity,
                rationale,
            }
        })
        .collect::<Vec<_>>();

    matches.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| b.pattern.support_count.cmp(&a.pattern.support_count))
    });
    matches
}

fn compute_text_similarity(
    event: &EventForMatch,
    pattern: &Pattern,
) -> (f32, f32, Option<f32>) {
    let trigger_similarity = token_similarity(event.trigger.as_str(), pattern.trigger.as_str());
    let invariant_similarity = match (event.invariant.as_ref(), pattern.invariant.as_ref()) {
        (Some(event_text), Some(pattern_text)) => {
            Some(token_similarity(event_text.as_str(), pattern_text.as_str()))
        }
        _ => None,
    };
    let text_similarity = match invariant_similarity {
        Some(invariant_similarity) => (trigger_similarity + invariant_similarity) / 2.0,
        None => trigger_similarity,
    };
    (text_similarity, trigger_similarity, invariant_similarity)
}

fn build_rationale(
    trigger_similarity: f32,
    invariant_similarity: Option<f32>,
    domain_similarity: f32,
    outcome_affinity: f32,
    desired_outcome: Option<&Outcome>,
    pattern_outcome: &Outcome,
) -> String {
    let invariant_part = match invariant_similarity {
        Some(value) => format!("invariant={value:.2}"),
        None => "invariant=n/a".to_string(),
    };
    let desired = desired_outcome.map_or_else(|| "none".to_string(), Outcome::to_string);
    format!(
        "text_similarity={:.2} (trigger={trigger_similarity:.2}, {invariant_part}), domain_similarity={domain_similarity:.2}, outcome_affinity={outcome_affinity:.2} (desired={desired}, pattern={pattern_outcome})",
        average_value(trigger_similarity, invariant_similarity)
    )
}

fn average_value(trigger_similarity: f32, invariant_similarity: Option<f32>) -> f32 {
    match invariant_similarity {
        Some(value) => (trigger_similarity + value) / 2.0,
        None => trigger_similarity,
    }
}

fn compute_outcome_affinity(desired: Option<&Outcome>, pattern: &Outcome) -> f32 {
    match (desired, pattern) {
        (None, _) => 0.5,
        (_, Outcome::Mixed) => 0.5,
        (Some(Outcome::Unknown), _) => 0.5,
        (Some(desired), pattern) if desired == pattern => 1.0,
        _ => 0.0,
    }
}

fn dominant_outcome(group: &[&ResolvedEvent]) -> Outcome {
    let mut counts: HashMap<&Outcome, usize> = HashMap::new();
    for event in group {
        *counts.entry(&event.outcome).or_default() += 1;
    }
    let mut items: Vec<(&Outcome, usize)> = counts.into_iter().collect();
    items.sort_by(|a, b| b.1.cmp(&a.1));
    let Some((top_outcome, top_count)) = items.first() else {
        return Outcome::Unknown;
    };
    let tied = items.iter().skip(1).any(|(_, count)| *count == *top_count);
    if tied {
        Outcome::Mixed
    } else {
        (*top_outcome).clone()
    }
}

fn select_counterexample(group: &[&ResolvedEvent], dominant: &Outcome) -> Option<String> {
    let baseline = match dominant {
        Outcome::Unknown => None,
        Outcome::Mixed => group.first().map(|event| &event.outcome),
        _ => Some(dominant),
    }?;
    group
        .iter()
        .find(|event| &event.outcome != baseline)
        .map(|event| format!("{} -> {}", event.trigger, event.outcome))
}

fn most_common_string<'a, I>(items: I) -> Option<String>
where
    I: Iterator<Item = &'a str>,
{
    let mut counts: HashMap<&'a str, usize> = HashMap::new();
    for item in items {
        *counts.entry(item).or_default() += 1;
    }
    counts
        .into_iter()
        .max_by_key(|(_, count)| *count)
        .map(|(value, _)| value.to_string())
}

fn most_common_optional_string<'a, I>(items: I) -> Option<String>
where
    I: Iterator<Item = Option<&'a String>>,
{
    let mut counts: HashMap<&'a String, usize> = HashMap::new();
    for item in items.flatten() {
        if item.trim().is_empty() {
            continue;
        }
        *counts.entry(item).or_default() += 1;
    }
    counts
        .into_iter()
        .max_by_key(|(_, count)| *count)
        .map(|(value, _)| value.clone())
}

fn average_signature<'a, I>(signatures: I) -> Vec<f32>
where
    I: Iterator<Item = &'a Vec<f32>>,
{
    let sigs: Vec<&Vec<f32>> = signatures.collect();
    let max_len = sigs.iter().map(|sig| sig.len()).max().unwrap_or(0);
    if max_len == 0 {
        return Vec::new();
    }
    let mut sums = vec![0.0; max_len];
    for sig in sigs.iter() {
        for (idx, value) in sig.iter().enumerate() {
            sums[idx] += value;
        }
    }
    let count = sigs.len() as f32;
    sums.into_iter().map(|value| value / count).collect()
}

fn collect_supporting_evidence(group: &[&ResolvedEvent]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for event in group {
        if event.evidence.is_empty() {
            if seen.insert(event.id.as_str()) {
                out.push(event.id.clone());
            }
            continue;
        }
        for item in &event.evidence {
            if seen.insert(item.as_str()) {
                out.push(item.clone());
            }
        }
    }
    out
}

fn normalize_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last_was_space = false;
    for ch in text.chars() {
        let normalized = if ch.is_alphanumeric() {
            ch.to_ascii_lowercase()
        } else {
            ' '
        };
        if normalized == ' ' {
            if last_was_space {
                continue;
            }
            last_was_space = true;
            out.push(' ');
            continue;
        }
        last_was_space = false;
        out.push(normalized);
    }
    out.trim().to_string()
}

fn token_similarity(left: &str, right: &str) -> f32 {
    let left_tokens = tokenize(left);
    let right_tokens = tokenize(right);
    if left_tokens.is_empty() && right_tokens.is_empty() {
        return 0.0;
    }
    let intersection = left_tokens
        .intersection(&right_tokens)
        .count() as f32;
    let union = left_tokens.union(&right_tokens).count() as f32;
    if union == 0.0 {
        0.0
    } else {
        intersection / union
    }
}

fn tokenize(text: &str) -> HashSet<String> {
    let mut normalized = String::with_capacity(text.len());
    for ch in text.chars() {
        if ch.is_alphanumeric() {
            normalized.push(ch.to_ascii_lowercase());
        } else {
            normalized.push(' ');
        }
    }
    normalized
        .split_whitespace()
        .map(str::to_string)
        .collect()
}

fn cosine_similarity(left: &[f32], right: &[f32]) -> f32 {
    if left.is_empty() || right.is_empty() {
        return 0.0;
    }
    let max_len = left.len().max(right.len());
    let min_len = left.len().min(right.len());
    let mut dot = 0.0;
    let mut norm_left = 0.0;
    let mut norm_right = 0.0;
    for idx in 0..max_len {
        let left_value = left.get(idx).copied().unwrap_or(0.0);
        let right_value = right.get(idx).copied().unwrap_or(0.0);
        dot += left_value * right_value;
        norm_left += left_value * left_value;
        norm_right += right_value * right_value;
    }
    if norm_left == 0.0 || norm_right == 0.0 {
        return 0.0;
    }
    let raw = dot / (norm_left.sqrt() * norm_right.sqrt());
    let length_penalty = min_len as f32 / max_len as f32;
    (raw * length_penalty).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn event(
        id: &str,
        trigger: &str,
        invariant: Option<&str>,
        outcome: Outcome,
        response: Option<&str>,
        domain_signature: Vec<f32>,
        evidence: Vec<&str>,
    ) -> ResolvedEvent {
        ResolvedEvent {
            id: id.to_string(),
            trigger: trigger.to_string(),
            invariant: invariant.map(str::to_string),
            outcome,
            response: response.map(str::to_string),
            domain_signature,
            evidence: evidence.into_iter().map(str::to_string).collect(),
        }
    }

    #[test]
    fn compile_groups_repeated_events() {
        let events = vec![
            event(
                "1",
                "Disk full error",
                Some("Writes fail"),
                Outcome::Failure,
                Some("Free space"),
                vec![1.0, 0.0],
                vec!["log-1"],
            ),
            event(
                "2",
                "Disk full error",
                Some("Writes fail"),
                Outcome::Failure,
                Some("Free space"),
                vec![1.0, 0.0],
                vec!["log-2"],
            ),
        ];

        let patterns = compile(&events);

        assert_eq!(
            patterns,
            vec![Pattern {
                trigger: "Disk full error".to_string(),
                invariant: Some("Writes fail".to_string()),
                counterexample: None,
                best_response: Some("Free space".to_string()),
                domain_signature: vec![1.0, 0.0],
                supporting_evidence: vec!["log-1".to_string(), "log-2".to_string()],
                outcome: Outcome::Failure,
                support_count: 2,
            }]
        );
    }

    #[test]
    fn compile_records_counterexample() {
        let events = vec![
            event(
                "1",
                "Cache miss",
                Some("Cold start"),
                Outcome::Failure,
                Some("Warm cache"),
                vec![0.5, 0.0],
                vec![],
            ),
            event(
                "2",
                "Cache miss",
                Some("Cold start"),
                Outcome::Success,
                Some("Warm cache"),
                vec![0.5, 0.0],
                vec![],
            ),
        ];

        let patterns = compile(&events);

        assert_eq!(
            patterns,
            vec![Pattern {
                trigger: "Cache miss".to_string(),
                invariant: Some("Cold start".to_string()),
                counterexample: Some("Cache miss -> success".to_string()),
                best_response: Some("Warm cache".to_string()),
                domain_signature: vec![0.5, 0.0],
                supporting_evidence: vec!["1".to_string(), "2".to_string()],
                outcome: Outcome::Mixed,
                support_count: 2,
            }]
        );
    }

    #[test]
    fn patterns_match_ranks_by_score() {
        let patterns = vec![
            Pattern {
                trigger: "Disk full error".to_string(),
                invariant: Some("Writes fail".to_string()),
                counterexample: None,
                best_response: Some("Free space".to_string()),
                domain_signature: vec![1.0, 0.0],
                supporting_evidence: vec!["log-1".to_string()],
                outcome: Outcome::Failure,
                support_count: 3,
            },
            Pattern {
                trigger: "Network timeout".to_string(),
                invariant: Some("Retries fail".to_string()),
                counterexample: None,
                best_response: Some("Backoff".to_string()),
                domain_signature: vec![0.0, 1.0],
                supporting_evidence: vec!["log-2".to_string()],
                outcome: Outcome::Success,
                support_count: 2,
            },
        ];

        let event = EventForMatch {
            trigger: "disk full error on write".to_string(),
            invariant: Some("writes fail".to_string()),
            domain_signature: vec![1.0, 0.0],
            desired_outcome: Some(Outcome::Failure),
        };

        let matches = patterns_match(&event, &patterns);

        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].pattern.trigger, "Disk full error");
        assert_eq!(matches[1].pattern.trigger, "Network timeout");
        assert_eq!(matches[0].outcome_affinity, 1.0);
        assert_eq!(matches[1].outcome_affinity, 0.0);
    }
}
