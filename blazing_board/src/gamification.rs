use chrono::{Duration, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

#[cfg(target_arch = "wasm32")]
const LOCAL_STATS_KEY: &str = "blazing-board.stats";
const LOCAL_STATS_VERSION: u8 = 1;

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
pub enum Badge {
    FirstSpark,
    PerfectBurn,
    FastHands,
    Inferno,
}

impl Badge {
    pub fn title(self) -> &'static str {
        match self {
            Self::FirstSpark => "First Spark",
            Self::PerfectBurn => "Perfect Burn",
            Self::FastHands => "Fast Hands",
            Self::Inferno => "Inferno",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::FirstSpark => "Finish your first run",
            Self::PerfectBurn => "Complete a run with 100% accuracy",
            Self::FastHands => "Reach 60 WPM",
            Self::Inferno => "Reach 100 WPM",
        }
    }

    fn announcement_priority(self) -> u8 {
        match self {
            Self::FirstSpark => 0,
            Self::FastHands => 1,
            Self::PerfectBurn => 2,
            Self::Inferno => 3,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct LocalStats {
    pub version: u8,
    pub last_completion_date: Option<String>,
    pub streak: u32,
    pub earned_badges: Vec<Badge>,
    pub best_wpm: f64,
    pub best_accuracy: f64,
    pub best_score: i64,
}

impl Default for LocalStats {
    fn default() -> Self {
        Self {
            version: LOCAL_STATS_VERSION,
            last_completion_date: None,
            streak: 0,
            earned_badges: Vec::new(),
            best_wpm: 0.0,
            best_accuracy: 0.0,
            best_score: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComboProgress {
    pub current: i64,
    pub best: i64,
    pub milestone: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaceStatus {
    Ahead,
    Behind,
    Even,
}

pub fn record_combo_word(current: i64, best: i64, correct: bool) -> ComboProgress {
    if !correct {
        return ComboProgress {
            current: 0,
            best,
            milestone: None,
        };
    }

    let current = current.saturating_add(1);
    ComboProgress {
        current,
        best: best.max(current),
        milestone: matches!(current, 10 | 25 | 50).then_some(current),
    }
}

pub fn current_challenge_date() -> String {
    Utc::now().format("%Y-%m-%d").to_string()
}

pub fn complete_daily_challenge(stats: &mut LocalStats, challenge_date: &str) -> bool {
    let Ok(today) = NaiveDate::parse_from_str(challenge_date, "%Y-%m-%d") else {
        return false;
    };
    let previous = stats
        .last_completion_date
        .as_deref()
        .and_then(|date| NaiveDate::parse_from_str(date, "%Y-%m-%d").ok());

    match previous {
        Some(date) if date >= today => false,
        Some(date) if date + Duration::days(1) == today => {
            stats.streak = stats.streak.saturating_add(1);
            stats.last_completion_date = Some(challenge_date.to_string());
            true
        }
        _ => {
            stats.streak = 1;
            stats.last_completion_date = Some(challenge_date.to_string());
            true
        }
    }
}

/// Updates stored personal bests. Returns true when score sets a new record.
pub fn update_personal_bests(stats: &mut LocalStats, wpm: f64, accuracy: f64, score: i64) -> bool {
    let is_new_record = score > stats.best_score;
    stats.best_wpm = stats.best_wpm.max(wpm);
    stats.best_accuracy = stats.best_accuracy.max(accuracy);
    stats.best_score = stats.best_score.max(score);
    is_new_record
}

pub fn pace_vs_best(correct_words: i64, elapsed_seconds: i64, best_wpm: f64) -> Option<PaceStatus> {
    if best_wpm <= 0.0 || elapsed_seconds < 3 || correct_words < 0 {
        return None;
    }

    let current_wpm = correct_words as f64 / (elapsed_seconds as f64 / 60.0);
    let delta = current_wpm - best_wpm;
    if delta > 1.0 {
        Some(PaceStatus::Ahead)
    } else if delta < -1.0 {
        Some(PaceStatus::Behind)
    } else {
        Some(PaceStatus::Even)
    }
}

pub fn award_badges(stats: &mut LocalStats, accuracy: f64, wpm: f64) -> Option<Badge> {
    let qualifying = [
        Badge::FirstSpark,
        Badge::PerfectBurn,
        Badge::FastHands,
        Badge::Inferno,
    ]
    .into_iter()
    .filter(|badge| match badge {
        Badge::FirstSpark => true,
        Badge::PerfectBurn => accuracy >= 1.0,
        Badge::FastHands => wpm >= 60.0,
        Badge::Inferno => wpm >= 100.0,
    });

    let mut newly_earned = Vec::new();
    for badge in qualifying {
        if !stats.earned_badges.contains(&badge) {
            stats.earned_badges.push(badge);
            newly_earned.push(badge);
        }
    }

    newly_earned
        .into_iter()
        .max_by_key(|badge| badge.announcement_priority())
}

pub fn load_local_stats() -> LocalStats {
    load_local_stats_from_browser()
        .filter(|stats| stats.version == LOCAL_STATS_VERSION)
        .unwrap_or_default()
}

pub fn save_local_stats(stats: &LocalStats) {
    save_local_stats_to_browser(stats);
}

#[cfg(target_arch = "wasm32")]
fn load_local_stats_from_browser() -> Option<LocalStats> {
    let storage = web_sys::window()?.local_storage().ok()??;
    let serialized = storage.get_item(LOCAL_STATS_KEY).ok()??;
    serde_json::from_str(&serialized).ok()
}

#[cfg(not(target_arch = "wasm32"))]
fn load_local_stats_from_browser() -> Option<LocalStats> {
    None
}

#[cfg(target_arch = "wasm32")]
fn save_local_stats_to_browser(stats: &LocalStats) {
    let Some(storage) = web_sys::window()
        .and_then(|window| window.local_storage().ok())
        .flatten()
    else {
        return;
    };
    let Ok(serialized) = serde_json::to_string(stats) else {
        return;
    };
    let _ = storage.set_item(LOCAL_STATS_KEY, &serialized);
}

#[cfg(not(target_arch = "wasm32"))]
fn save_local_stats_to_browser(_stats: &LocalStats) {}

#[cfg(test)]
mod tests {
    use super::{
        Badge, LocalStats, PaceStatus, award_badges, complete_daily_challenge, pace_vs_best,
        record_combo_word, update_personal_bests,
    };

    #[test]
    fn combo_reports_milestones_and_resets_on_a_mistake() {
        let milestone = record_combo_word(9, 9, true);
        assert_eq!(milestone.current, 10);
        assert_eq!(milestone.best, 10);
        assert_eq!(milestone.milestone, Some(10));

        let reset = record_combo_word(milestone.current, milestone.best, false);
        assert_eq!(reset.current, 0);
        assert_eq!(reset.best, 10);
        assert_eq!(reset.milestone, None);
    }

    #[test]
    fn daily_streak_increments_once_per_consecutive_day() {
        let mut stats = LocalStats::default();

        assert!(complete_daily_challenge(&mut stats, "2026-07-18"));
        assert_eq!(stats.streak, 1);
        assert!(!complete_daily_challenge(&mut stats, "2026-07-18"));
        assert_eq!(stats.streak, 1);
        assert!(complete_daily_challenge(&mut stats, "2026-07-19"));
        assert_eq!(stats.streak, 2);
    }

    #[test]
    fn daily_streak_resets_after_a_missed_day() {
        let mut stats = LocalStats::default();
        complete_daily_challenge(&mut stats, "2026-07-17");

        assert!(complete_daily_challenge(&mut stats, "2026-07-19"));
        assert_eq!(stats.streak, 1);
    }

    #[test]
    fn awards_every_qualified_badge_but_announces_only_the_rarest() {
        let mut stats = LocalStats::default();

        assert_eq!(award_badges(&mut stats, 1.0, 105.0), Some(Badge::Inferno));
        assert_eq!(
            stats.earned_badges,
            vec![
                Badge::FirstSpark,
                Badge::PerfectBurn,
                Badge::FastHands,
                Badge::Inferno,
            ]
        );
        assert_eq!(award_badges(&mut stats, 1.0, 105.0), None);
    }

    #[test]
    fn personal_best_tracks_score_and_peak_stats() {
        let mut stats = LocalStats::default();

        assert!(update_personal_bests(&mut stats, 40.0, 0.9, 36));
        assert!(!update_personal_bests(&mut stats, 35.0, 1.0, 30));
        assert!(update_personal_bests(&mut stats, 55.0, 0.95, 52));
        assert!((stats.best_wpm - 55.0).abs() < f64::EPSILON);
        assert!((stats.best_accuracy - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.best_score, 52);
    }

    #[test]
    fn pace_compares_current_wpm_to_personal_best() {
        assert_eq!(pace_vs_best(20, 15, 60.0), Some(PaceStatus::Ahead));
        assert_eq!(pace_vs_best(10, 15, 60.0), Some(PaceStatus::Behind));
        assert_eq!(pace_vs_best(15, 15, 60.0), Some(PaceStatus::Even));
        assert_eq!(pace_vs_best(20, 2, 60.0), None);
    }
}
