use chrono::{DateTime, Utc};
#[cfg(any(feature = "server", test))]
use chrono::{Datelike, Duration, NaiveDate};
use serde::{Deserialize, Serialize};

/// How many past UTC challenge days are exposed on the day leaderboard.
#[cfg(any(feature = "server", test))]
pub const RECENT_LEADERBOARD_DAYS: i64 = 10;
/// Top entries returned for each leaderboard board.
#[cfg(any(feature = "server", test))]
pub const LEADERBOARD_TOP_N: u32 = 50;

#[cfg(any(feature = "server", test))]
pub const GLOBAL_BOARD_ID: &str = "global";

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

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LeaderboardScope {
    Day,
    Week,
    Global,
}

impl LeaderboardScope {
    #[cfg(any(feature = "server", test))]
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "day" | "daily" => Some(Self::Day),
            "week" | "weekly" => Some(Self::Week),
            "global" | "all" => Some(Self::Global),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Day => "day",
            Self::Week => "week",
            Self::Global => "global",
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct LeaderboardEntry {
    pub rank: i32,
    pub github_id: String,
    pub login: String,
    pub avatar_url: String,
    pub score: i64,
    pub wpm: f64,
    pub accuracy: f64,
    pub run_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Leaderboard {
    pub scope: LeaderboardScope,
    pub board_id: String,
    pub label: String,
    pub challenge_date: Option<String>,
    pub entries: Vec<LeaderboardEntry>,
}

/// Denormalized per-user best for a board. Stored under
/// `leaderboards/{board_id}/entries/{github_id}`.
#[cfg(any(feature = "server", test))]
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct LeaderboardStoredEntry {
    pub github_id: String,
    pub login: String,
    pub avatar_url: String,
    pub score: i64,
    pub wpm: f64,
    pub accuracy: f64,
    pub run_id: String,
    pub challenge_date: String,
    /// `score * 100_000 + round(wpm * 100)` so a single-field order breaks ties.
    pub sort_key: i64,
    pub updated_at: DateTime<Utc>,
}

#[cfg(any(feature = "server", test))]
pub fn challenge_date_string(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

#[cfg(any(feature = "server", test))]
pub fn parse_challenge_date(value: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").ok()
}

#[cfg(any(feature = "server", test))]
pub fn day_board_id(date: NaiveDate) -> String {
    format!("day-{}", challenge_date_string(date))
}

#[cfg(any(feature = "server", test))]
pub fn week_board_id(date: NaiveDate) -> String {
    let week = date.iso_week();
    format!("week-{:04}-W{:02}", week.year(), week.week())
}

#[cfg(any(feature = "server", test))]
pub fn leaderboard_sort_key(score: i64, wpm: f64) -> i64 {
    score
        .saturating_mul(100_000)
        .saturating_add((wpm * 100.0).round() as i64)
}

#[cfg(any(feature = "server", test))]
pub fn recent_challenge_dates(today: NaiveDate, n: i64) -> Vec<NaiveDate> {
    let n = n.max(1);
    (0..n).map(|offset| today - Duration::days(offset)).collect()
}

#[cfg(any(feature = "server", test))]
pub fn board_id_for_scope(scope: LeaderboardScope, date: NaiveDate) -> String {
    match scope {
        LeaderboardScope::Day => day_board_id(date),
        LeaderboardScope::Week => week_board_id(date),
        LeaderboardScope::Global => GLOBAL_BOARD_ID.to_string(),
    }
}

#[cfg(any(feature = "server", test))]
pub fn leaderboard_label(scope: LeaderboardScope, date: NaiveDate) -> String {
    match scope {
        LeaderboardScope::Day => challenge_date_string(date),
        LeaderboardScope::Week => {
            let week = date.iso_week();
            format!("Week {} · {}", week.week(), week.year())
        }
        LeaderboardScope::Global => "All time".to_string(),
    }
}

#[cfg(any(feature = "server", test))]
pub fn is_allowed_recent_day(day: NaiveDate, today: NaiveDate) -> bool {
    recent_challenge_dates(today, RECENT_LEADERBOARD_DAYS).contains(&day)
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
    use super::{
        LeaderboardScope, board_id_for_scope, calculate_typing_metrics, challenge_date_string,
        day_board_id, is_allowed_recent_day, leaderboard_sort_key, parse_challenge_date,
        recent_challenge_dates, validate_run_id, week_board_id,
    };
    use chrono::NaiveDate;

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

    #[test]
    fn builds_stable_board_ids() {
        let date = NaiveDate::from_ymd_opt(2026, 7, 20).unwrap();
        assert_eq!(challenge_date_string(date), "2026-07-20");
        assert_eq!(day_board_id(date), "day-2026-07-20");
        assert_eq!(week_board_id(date), "week-2026-W30");
        assert_eq!(
            board_id_for_scope(LeaderboardScope::Global, date),
            "global"
        );
        assert_eq!(
            leaderboard_sort_key(41, 45.0),
            41 * 100_000 + 4_500
        );
    }

    #[test]
    fn limits_recent_day_access_to_configured_window() {
        let today = NaiveDate::from_ymd_opt(2026, 7, 20).unwrap();
        let days = recent_challenge_dates(today, 10);
        assert_eq!(days.len(), 10);
        assert_eq!(days[0], today);
        assert_eq!(days[9], NaiveDate::from_ymd_opt(2026, 7, 11).unwrap());
        assert!(is_allowed_recent_day(days[9], today));
        assert!(!is_allowed_recent_day(
            NaiveDate::from_ymd_opt(2026, 7, 10).unwrap(),
            today
        ));
        assert_eq!(parse_challenge_date("2026-07-20"), Some(today));
    }

    #[test]
    fn parses_leaderboard_scopes() {
        assert_eq!(LeaderboardScope::parse("day"), Some(LeaderboardScope::Day));
        assert_eq!(
            LeaderboardScope::parse("WEEKLY"),
            Some(LeaderboardScope::Week)
        );
        assert_eq!(
            LeaderboardScope::parse("global"),
            Some(LeaderboardScope::Global)
        );
        assert_eq!(LeaderboardScope::parse("nope"), None);
    }
}
