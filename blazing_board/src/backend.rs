#[cfg(feature = "server")]
use chrono::{Duration, NaiveDate, NaiveTime, Utc};
use dioxus::prelude::*;
use models::{PrivateProfile, Story, TypingResult, TypingSubmission};
#[cfg(feature = "server")]
use std::sync::Arc;

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
static CLIENT: OnceCell<FirestoreDb> = OnceCell::const_new();
#[cfg(feature = "server")]
static LAST_STORY: OnceCell<Arc<Mutex<Option<(NaiveDate, Story)>>>> = OnceCell::const_new();

#[cfg(feature = "server")]
async fn initialize_last_story() -> Arc<Mutex<Option<(NaiveDate, Story)>>> {
    Arc::new(Mutex::new(None))
}
#[cfg(feature = "server")]
async fn get_last_story() -> Arc<Mutex<Option<(NaiveDate, Story)>>> {
    LAST_STORY.get_or_init(initialize_last_story).await.clone()
}

#[server]
pub async fn get_story() -> Result<Story, ServerFnError> {
    let challenge_date = Utc::now().date_naive();
    let last_result_story = get_last_story().await;
    let mut last_story = last_result_story.lock().await;
    if let Some((cached_date, story)) = last_story.as_ref()
        && *cached_date == challenge_date
    {
        return Ok(story.clone());
    }

    let db = get_client_db().await;
    // Same cutoff on every instance: latest story published before tomorrow UTC.
    let next_day = (challenge_date + Duration::days(1))
        .and_time(NaiveTime::MIN)
        .and_utc();

    let mut story_stream = db
        .fluent()
        .select()
        .from("texts")
        .filter(|q| q.field("when").less_than(FirestoreTimestamp(next_day)))
        .order_by([("when", FirestoreQueryDirection::Descending)])
        .limit(1)
        .obj::<Story>()
        .stream_query()
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

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
            let daily_story = Story {
                title: latest_story.title,
                sources: latest_story.sources,
                story: filtered_story,
                when: latest_story.when,
            };
            *last_story = Some((challenge_date, daily_story.clone()));
            Ok(daily_story)
        }
        None => Err(ServerFnError::new("No stories found")),
    }
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
    let metrics = calculate_typing_metrics(
        submission.correct_words,
        submission.wrong_words,
        submission.duration_seconds,
    )
    .map_err(ServerFnError::new)?;

    let story = get_story().await?;
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

    save_result_transaction(&user_id, result)
        .await
        .map_err(private_server_error)
}

#[cfg(feature = "server")]
async fn save_result_transaction(
    user_id: &str,
    result: TypingResult,
) -> firestore::FirestoreResult<TypingResult> {
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
                return Ok(existing);
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

            Ok(result)
        })
    })
    .await
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
