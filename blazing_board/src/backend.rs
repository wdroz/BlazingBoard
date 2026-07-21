#[cfg(feature = "server")]
use chrono::{NaiveDate, NaiveTime, Utc};
use dioxus::prelude::*;
use models::{Leaderboard, PrivateProfile, Story, TypingResult, TypingSubmission};
#[cfg(feature = "server")]
use models::{
    LEADERBOARD_TOP_N, LeaderboardEntry, LeaderboardScope, LeaderboardStoredEntry,
    RECENT_LEADERBOARD_DAYS, board_id_for_scope, challenge_date_string, is_allowed_recent_day,
    leaderboard_label, leaderboard_sort_key, parse_challenge_date, recent_challenge_dates,
};
#[cfg(feature = "server")]
use std::collections::HashMap;
#[cfg(feature = "server")]
use std::sync::Arc;
#[cfg(feature = "server")]
use std::time::{Duration as StdDuration, Instant};

#[cfg(feature = "server")]
use firestore::{FirestoreDb, FirestoreDbOptions, FirestoreQueryDirection, FirestoreTimestamp};
#[cfg(feature = "server")]
use futures::stream::StreamExt;

#[cfg(feature = "server")]
use std::env;
#[cfg(feature = "server")]
use tokio::sync::{Mutex, OnceCell};

use crate::models;
#[cfg(feature = "server")]
use crate::{
    auth::authenticated_user_id,
    models::{UserProfile, calculate_typing_metrics, validate_run_id},
};

#[cfg(feature = "server")]
const USERS_COLLECTION: &str = "users";
#[cfg(feature = "server")]
const TYPING_RESULTS_COLLECTION: &str = "typing_results";
#[cfg(feature = "server")]
const LEADERBOARDS_COLLECTION: &str = "leaderboards";
#[cfg(feature = "server")]
const LEADERBOARD_ENTRIES_COLLECTION: &str = "entries";
#[cfg(feature = "server")]
const LEADERBOARD_CACHE_TTL: StdDuration = StdDuration::from_secs(45);

#[cfg(feature = "server")]
static CLIENT: OnceCell<FirestoreDb> = OnceCell::const_new();
#[cfg(feature = "server")]
static STORY_CACHE: OnceCell<Arc<Mutex<HashMap<NaiveDate, Story>>>> = OnceCell::const_new();
#[cfg(feature = "server")]
static LEADERBOARD_CACHE: OnceCell<Arc<Mutex<HashMap<String, CachedLeaderboard>>>> =
    OnceCell::const_new();

#[cfg(feature = "server")]
#[derive(Clone)]
struct CachedLeaderboard {
    fetched_at: Instant,
    board: Leaderboard,
}

#[cfg(feature = "server")]
async fn initialize_story_cache() -> Arc<Mutex<HashMap<NaiveDate, Story>>> {
    Arc::new(Mutex::new(HashMap::new()))
}

#[cfg(feature = "server")]
async fn get_story_cache() -> Arc<Mutex<HashMap<NaiveDate, Story>>> {
    STORY_CACHE.get_or_init(initialize_story_cache).await.clone()
}

#[cfg(feature = "server")]
async fn initialize_leaderboard_cache() -> Arc<Mutex<HashMap<String, CachedLeaderboard>>> {
    Arc::new(Mutex::new(HashMap::new()))
}

#[cfg(feature = "server")]
async fn get_leaderboard_cache() -> Arc<Mutex<HashMap<String, CachedLeaderboard>>> {
    LEADERBOARD_CACHE
        .get_or_init(initialize_leaderboard_cache)
        .await
        .clone()
}

#[get("/api/story?day")]
pub async fn get_story(day: Option<String>) -> Result<Story, ServerFnError> {
    let today = Utc::now().date_naive();
    let challenge_date = resolve_challenge_day(day.as_deref(), today)?;

    let cache = get_story_cache().await;
    {
        let guard = cache.lock().await;
        if let Some(story) = guard.get(&challenge_date) {
            return Ok(story.clone());
        }
    }

    let story = load_story_for_day(challenge_date)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    cache.lock().await.insert(challenge_date, story.clone());
    Ok(story)
}

#[cfg(feature = "server")]
fn resolve_challenge_day(
    day: Option<&str>,
    today: NaiveDate,
) -> Result<NaiveDate, ServerFnError> {
    let challenge_date = match day {
        Some(value) => parse_challenge_date(value)
            .ok_or_else(|| ServerFnError::new("Challenge day must use YYYY-MM-DD"))?,
        None => today,
    };
    if !is_allowed_recent_day(challenge_date, today) {
        return Err(ServerFnError::new(format!(
            "Challenges are limited to the latest {RECENT_LEADERBOARD_DAYS} UTC days"
        )));
    }
    Ok(challenge_date)
}

#[cfg(feature = "server")]
async fn load_story_for_day(challenge_date: NaiveDate) -> Result<Story, String> {
    let db = get_client_db().await;
    // Freeze each challenge at the start of its UTC day. Using the end of the
    // selected day here can assign the same late-published story to both today
    // and yesterday until the next story is generated.
    let story_cutoff = story_cutoff_for_day(challenge_date);

    let mut story_stream = db
        .fluent()
        .select()
        .from("texts")
        .filter(|q| q.field("when").less_than(FirestoreTimestamp(story_cutoff)))
        .order_by([("when", FirestoreQueryDirection::Descending)])
        .limit(1)
        .obj::<Story>()
        .stream_query()
        .await
        .map_err(|e| e.to_string())?;

    match story_stream.next().await {
        Some(latest_story) => {
            let filtered_story = latest_story
                .story
                .replace('\n', " ")
                .replace(",", "")
                .replace(".", "")
                .replace(":", "")
                .replace(";", "")
                .replace("’", "'");
            Ok(Story {
                title: latest_story.title,
                sources: latest_story.sources,
                story: filtered_story,
                when: latest_story.when,
            })
        }
        None => Err("No stories found".to_string()),
    }
}

#[cfg(feature = "server")]
fn story_cutoff_for_day(challenge_date: NaiveDate) -> chrono::DateTime<Utc> {
    challenge_date.and_time(NaiveTime::MIN).and_utc()
}

#[get(
    "/api/profile",
    headers: dioxus::prelude::dioxus_fullstack::HeaderMap
)]
pub async fn get_private_profile() -> Result<Option<PrivateProfile>, ServerFnError> {
    let Some(user_id) = authenticated_user_id(&headers)
        .await
        .map_err(private_server_error)?
    else {
        return Ok(None);
    };

    let db = get_client_db().await;
    let user = db
        .fluent()
        .select()
        .by_id_in(USERS_COLLECTION)
        .obj::<UserProfile>()
        .one(&user_id)
        .await
        .map_err(private_server_error)?
        .ok_or_else(|| ServerFnError::new("The signed-in profile no longer exists"))?;
    let parent = db
        .parent_path(USERS_COLLECTION, &user_id)
        .map_err(private_server_error)?;
    let history = db
        .fluent()
        .select()
        .from(TYPING_RESULTS_COLLECTION)
        .parent(&parent)
        .order_by([(
            "created_at_epoch_seconds",
            FirestoreQueryDirection::Descending,
        )])
        .limit(20)
        .obj::<TypingResult>()
        .query()
        .await
        .map_err(private_server_error)?;

    Ok(Some(PrivateProfile { user, history }))
}

#[get("/api/leaderboard?scope&day")]
pub async fn get_leaderboard(
    scope: String,
    day: Option<String>,
) -> Result<Leaderboard, ServerFnError> {
    let scope = LeaderboardScope::parse(&scope)
        .ok_or_else(|| ServerFnError::new("Leaderboard scope must be day, week, or global"))?;
    let today = Utc::now().date_naive();
    let challenge_date = if scope == LeaderboardScope::Global {
        today
    } else {
        resolve_challenge_day(day.as_deref(), today)?
    };
    let board_id = board_id_for_scope(scope, challenge_date);
    if let Some(cached) = cached_leaderboard(&board_id).await {
        return Ok(cached);
    }

    let board = load_leaderboard_from_firestore(scope, &board_id, challenge_date)
        .await
        .map_err(private_server_error)?;
    store_leaderboard_cache(board.clone()).await;
    Ok(board)
}

#[get("/api/leaderboard/recent-days")]
pub async fn get_recent_leaderboard_days() -> Result<Vec<String>, ServerFnError> {
    let today = Utc::now().date_naive();
    Ok(recent_challenge_dates(today, RECENT_LEADERBOARD_DAYS)
        .into_iter()
        .map(challenge_date_string)
        .collect())
}

#[post(
    "/api/typing-results",
    headers: dioxus::prelude::dioxus_fullstack::HeaderMap
)]
pub async fn save_typing_result(
    submission: TypingSubmission,
) -> Result<TypingResult, ServerFnError> {
    let user_id = authenticated_user_id(&headers)
        .await
        .map_err(private_server_error)?
        .ok_or_else(|| ServerFnError::new("Sign in to save typing history"))?;

    validate_run_id(&submission.run_id).map_err(ServerFnError::new)?;
    let today = Utc::now().date_naive();
    let challenge_date = resolve_challenge_day(Some(submission.challenge_date.as_str()), today)?;
    let metrics = calculate_typing_metrics(
        submission.correct_words,
        submission.wrong_words,
        submission.duration_seconds,
    )
    .map_err(ServerFnError::new)?;

    let story = get_story(Some(challenge_date_string(challenge_date))).await?;
    if story.when.timestamp() != submission.story_when.timestamp() {
        return Err(ServerFnError::new(
            "The typing story changed before this result was saved",
        ));
    }

    let created_at = chrono::Utc::now();
    let result = TypingResult {
        run_id: submission.run_id,
        story_title: story.title.unwrap_or_else(|| "Daily story".to_string()),
        story_when: story.when,
        correct_words: submission.correct_words,
        wrong_words: submission.wrong_words,
        duration_seconds: submission.duration_seconds,
        accuracy: metrics.accuracy,
        wpm: metrics.wpm,
        score: metrics.score,
        created_at,
        created_at_epoch_seconds: created_at.timestamp(),
    };

    let (saved, touched_boards) = save_result_transaction(&user_id, result, challenge_date)
        .await
        .map_err(private_server_error)?;
    invalidate_leaderboard_cache(&touched_boards).await;
    Ok(saved)
}

#[cfg(feature = "server")]
async fn cached_leaderboard(board_id: &str) -> Option<Leaderboard> {
    let cache = get_leaderboard_cache().await;
    let guard = cache.lock().await;
    guard.get(board_id).and_then(|cached| {
        if cached.fetched_at.elapsed() <= LEADERBOARD_CACHE_TTL {
            Some(cached.board.clone())
        } else {
            None
        }
    })
}

#[cfg(feature = "server")]
async fn store_leaderboard_cache(board: Leaderboard) {
    let cache = get_leaderboard_cache().await;
    let mut guard = cache.lock().await;
    guard.insert(
        board.board_id.clone(),
        CachedLeaderboard {
            fetched_at: Instant::now(),
            board,
        },
    );
}

#[cfg(feature = "server")]
async fn invalidate_leaderboard_cache(board_ids: &[String]) {
    if board_ids.is_empty() {
        return;
    }
    let cache = get_leaderboard_cache().await;
    let mut guard = cache.lock().await;
    for board_id in board_ids {
        guard.remove(board_id);
    }
}

#[cfg(feature = "server")]
async fn load_leaderboard_from_firestore(
    scope: LeaderboardScope,
    board_id: &str,
    challenge_date: NaiveDate,
) -> firestore::FirestoreResult<Leaderboard> {
    let db = get_client_db().await;
    let parent = db.parent_path(LEADERBOARDS_COLLECTION, board_id)?;
    let stored = db
        .fluent()
        .select()
        .from(LEADERBOARD_ENTRIES_COLLECTION)
        .parent(&parent)
        .order_by([("sort_key", FirestoreQueryDirection::Descending)])
        .limit(LEADERBOARD_TOP_N)
        .obj::<LeaderboardStoredEntry>()
        .query()
        .await?;

    let entries = stored
        .into_iter()
        .enumerate()
        .map(|(index, entry)| LeaderboardEntry {
            rank: (index + 1) as i32,
            github_id: entry.github_id,
            login: entry.login,
            avatar_url: entry.avatar_url,
            score: entry.score,
            wpm: entry.wpm,
            accuracy: entry.accuracy,
            run_id: entry.run_id,
        })
        .collect();

    Ok(Leaderboard {
        scope,
        board_id: board_id.to_string(),
        label: leaderboard_label(scope, challenge_date),
        challenge_date: match scope {
            LeaderboardScope::Global => None,
            LeaderboardScope::Day | LeaderboardScope::Week => {
                Some(challenge_date_string(challenge_date))
            }
        },
        entries,
    })
}

#[cfg(feature = "server")]
async fn save_result_transaction(
    user_id: &str,
    result: TypingResult,
    challenge_date: NaiveDate,
) -> firestore::FirestoreResult<(TypingResult, Vec<String>)> {
    let db = get_client_db().await;
    let user_id = user_id.to_string();

    db.run_transaction(move |db, transaction| {
        let user_id = user_id.clone();
        let result = result.clone();
        Box::pin(async move {
            let parent = db.parent_path(USERS_COLLECTION, &user_id)?;
            let existing = db
                .fluent()
                .select()
                .by_id_in(TYPING_RESULTS_COLLECTION)
                .parent(&parent)
                .obj::<TypingResult>()
                .one(&result.run_id)
                .await?;
            if let Some(existing) = existing {
                return Ok((existing, Vec::new()));
            }

            let mut user = db
                .fluent()
                .select()
                .by_id_in(USERS_COLLECTION)
                .obj::<UserProfile>()
                .one(&user_id)
                .await?
                .ok_or_else(|| {
                    firestore::errors::FirestoreError::DataNotFoundError(
                        firestore::errors::FirestoreDataNotFoundError {
                            public: firestore::errors::FirestoreErrorPublicGenericDetails {
                                code: "profile_missing".to_string(),
                            },
                            data_detail_message: format!(
                                "Authenticated profile {user_id} no longer exists"
                            ),
                        },
                    )
                })?;

            user.total_runs += 1;
            user.best_wpm = user.best_wpm.max(result.wpm);
            user.best_accuracy = user.best_accuracy.max(result.accuracy);
            user.best_score = user.best_score.max(result.score);

            db.fluent()
                .update()
                .in_col(TYPING_RESULTS_COLLECTION)
                .document_id(&result.run_id)
                .parent(&parent)
                .object(&result)
                .add_to_transaction(transaction)?;
            db.fluent()
                .update()
                .in_col(USERS_COLLECTION)
                .document_id(&user_id)
                .object(&user)
                .add_to_transaction(transaction)?;

            let challenge_date_str = challenge_date_string(challenge_date);
            let candidate = LeaderboardStoredEntry {
                github_id: user.github_id.clone(),
                login: user.login.clone(),
                avatar_url: user.avatar_url.clone(),
                score: result.score,
                wpm: result.wpm,
                accuracy: result.accuracy,
                run_id: result.run_id.clone(),
                challenge_date: challenge_date_str,
                sort_key: leaderboard_sort_key(result.score, result.wpm),
                updated_at: result.created_at,
            };

            let board_ids = [
                board_id_for_scope(LeaderboardScope::Day, challenge_date),
                board_id_for_scope(LeaderboardScope::Week, challenge_date),
                board_id_for_scope(LeaderboardScope::Global, challenge_date),
            ];
            let mut touched_boards = Vec::new();
            for board_id in board_ids {
                if upsert_leaderboard_entry(&db, transaction, &board_id, &candidate).await? {
                    touched_boards.push(board_id);
                }
            }

            Ok((result, touched_boards))
        })
    })
    .await
}

#[cfg(feature = "server")]
async fn upsert_leaderboard_entry(
    db: &FirestoreDb,
    transaction: &mut firestore::FirestoreTransaction<'_>,
    board_id: &str,
    candidate: &LeaderboardStoredEntry,
) -> firestore::FirestoreResult<bool> {
    let parent = db.parent_path(LEADERBOARDS_COLLECTION, board_id)?;
    let existing = db
        .fluent()
        .select()
        .by_id_in(LEADERBOARD_ENTRIES_COLLECTION)
        .parent(&parent)
        .obj::<LeaderboardStoredEntry>()
        .one(&candidate.github_id)
        .await?;

    if existing
        .as_ref()
        .is_some_and(|entry| entry.sort_key >= candidate.sort_key)
    {
        return Ok(false);
    }

    db.fluent()
        .update()
        .in_col(LEADERBOARD_ENTRIES_COLLECTION)
        .document_id(&candidate.github_id)
        .parent(&parent)
        .object(candidate)
        .add_to_transaction(transaction)?;
    Ok(true)
}

#[cfg(feature = "server")]
fn private_server_error(error: impl std::fmt::Display) -> ServerFnError {
    eprintln!("Private server operation failed: {error}");
    ServerFnError::new("The server could not complete this request")
}

#[cfg(feature = "server")]
pub(crate) async fn get_client_db() -> &'static FirestoreDb {
    CLIENT
        .get_or_init(|| async {
            dotenvy::dotenv().ok();

            let project_id = env::var("PROJECT_ID").expect("PROJECT_ID not set");
            let database_id = env::var("DATABASE_ID").expect("DATABASE_ID not set");

            // Check for the "IAMTHEDEV" environment variable
            if env::var("IAMTHEDEV").is_err() {
                let db = FirestoreDb::with_options(
                    FirestoreDbOptions::new(project_id).with_database_id(database_id),
                )
                .await
                .expect("Failed to initialize FirestoreDb using PROJECT_ID and DATABASE_ID");
                return db;
            }

            // Initialize Firestore client using service account key file
            let db = FirestoreDb::with_options_service_account_key_file(
                FirestoreDbOptions::new(project_id).with_database_id(database_id),
                "key.json".into(),
            )
            .await
            .expect("Failed to initialize FirestoreDb using service account key file");

            db
        })
        .await
}

#[cfg(all(test, feature = "server"))]
mod tests {
    use super::story_cutoff_for_day;
    use chrono::{DateTime, NaiveDate, Utc};

    #[test]
    fn consecutive_challenges_have_distinct_story_cutoffs() {
        let yesterday = NaiveDate::from_ymd_opt(2026, 7, 20).unwrap();
        let today = NaiveDate::from_ymd_opt(2026, 7, 21).unwrap();
        let late_yesterday_story = DateTime::parse_from_rfc3339("2026-07-20T21:00:00Z")
            .unwrap()
            .with_timezone(&Utc);

        assert_eq!(
            story_cutoff_for_day(yesterday).to_rfc3339(),
            "2026-07-20T00:00:00+00:00"
        );
        assert_eq!(
            story_cutoff_for_day(today).to_rfc3339(),
            "2026-07-21T00:00:00+00:00"
        );
        assert!(story_cutoff_for_day(yesterday) < story_cutoff_for_day(today));
        assert!(late_yesterday_story >= story_cutoff_for_day(yesterday));
        assert!(late_yesterday_story < story_cutoff_for_day(today));
    }
}
