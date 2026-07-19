use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Story {
    pub sources: Vec<String>,
    pub story: String,
    pub title: Option<String>,
    pub when: DateTime<Utc>,
}

impl Default for Story {
    fn default() -> Story {
        Story {
            sources: vec!["https://doc.rust-lang.org/book/".to_string()],
            story: include_str!("../assets/texts/01.txt").to_string(),
            title: Some("The Rust Programming Language".to_string()),
            // Stable fallback so SSR/client and save_typing_result story_when checks agree.
            when: DateTime::parse_from_rfc3339("2024-01-01T00:00:00Z")
                .expect("fallback story timestamp")
                .with_timezone(&Utc),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct UserProfile {
    pub github_id: String,
    pub login: String,
    pub display_name: Option<String>,
    pub avatar_url: String,
    pub created_at: DateTime<Utc>,
    pub last_login_at: DateTime<Utc>,
    pub total_runs: i64,
    pub best_wpm: f64,
    pub best_accuracy: f64,
    pub best_score: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct TypingResult {
    pub run_id: String,
    pub story_title: String,
    pub story_when: DateTime<Utc>,
    pub correct_words: i64,
    pub wrong_words: i64,
    pub duration_seconds: i64,
    pub accuracy: f64,
    pub wpm: f64,
    pub score: i64,
    pub created_at: DateTime<Utc>,
    pub created_at_epoch_seconds: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct TypingSubmission {
    pub run_id: String,
    pub story_when: DateTime<Utc>,
    pub correct_words: i64,
    pub wrong_words: i64,
    pub duration_seconds: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct PrivateProfile {
    pub user: UserProfile,
    pub history: Vec<TypingResult>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TypingMetrics {
    pub accuracy: f64,
    pub wpm: f64,
    pub score: i64,
}

pub fn calculate_typing_metrics(
    correct_words: i64,
    wrong_words: i64,
    duration_seconds: i64,
) -> Result<TypingMetrics, &'static str> {
    let total_words = correct_words
        .checked_add(wrong_words)
        .ok_or("Word count is too large")?;

    if !(1..=600).contains(&duration_seconds) {
        return Err("Duration must be between 1 and 600 seconds");
    }
    if correct_words < 0 || wrong_words < 0 || !(1..=2_000).contains(&total_words) {
        return Err("Word counts are invalid");
    }

    let accuracy = correct_words as f64 / total_words as f64;
    let wpm = correct_words as f64 / (duration_seconds as f64 / 60.0);
    let score = (wpm * accuracy).round() as i64;

    Ok(TypingMetrics {
        accuracy,
        wpm,
        score,
    })
}

#[cfg(any(feature = "server", test))]
pub fn validate_run_id(run_id: &str) -> Result<(), &'static str> {
    if !(8..=80).contains(&run_id.len())
        || !run_id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err("Run ID is invalid");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{calculate_typing_metrics, validate_run_id};

    #[test]
    fn calculates_server_authoritative_metrics() {
        let metrics = calculate_typing_metrics(45, 5, 60).unwrap();

        assert!((metrics.accuracy - 0.9).abs() < f64::EPSILON);
        assert!((metrics.wpm - 45.0).abs() < f64::EPSILON);
        assert_eq!(metrics.score, 41);
    }

    #[test]
    fn rejects_invalid_result_bounds() {
        assert!(calculate_typing_metrics(0, 0, 60).is_err());
        assert!(calculate_typing_metrics(10, 0, 0).is_err());
        assert!(calculate_typing_metrics(-1, 2, 60).is_err());
        assert!(calculate_typing_metrics(2_001, 0, 60).is_err());
    }

    #[test]
    fn validates_safe_firestore_run_ids() {
        assert!(validate_run_id("run-1721376000000").is_ok());
        assert!(validate_run_id("../bad").is_err());
        assert!(validate_run_id("short").is_err());
    }
}
