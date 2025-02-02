use async_std::task::sleep;
use dioxus::prelude::*;
use jiff::Timestamp;
use models::Story;
use wasm_bindgen::prelude::*;

#[cfg(feature = "server")]
use firestore::{FirestoreDb, FirestoreDbOptions, FirestoreQueryDirection, FirestoreResult};
#[cfg(feature = "server")]
use futures::stream::StreamExt;

#[cfg(feature = "server")]
use std::env;
use std::sync::Arc;
#[cfg(feature = "server")]
use tokio::sync::{Mutex, OnceCell};

use crate::models;

#[cfg(feature = "server")]
static CLIENT: OnceCell<FirestoreDb> = OnceCell::const_new();
#[cfg(feature = "server")]
static LAST_TIME_REQ: OnceCell<Arc<Mutex<i64>>> = OnceCell::const_new();

#[cfg(feature = "server")]
static LAST_STORY: OnceCell<Arc<Mutex<Story>>> = OnceCell::const_new();

pub fn get_timestamp_seconds_now() -> i64 {
    let now: Timestamp = Timestamp::now();
    now.as_second()
}

#[cfg(feature = "server")]
async fn initialize_last_time_req() -> Arc<Mutex<i64>> {
    Arc::new(Mutex::new(0))
}
#[cfg(feature = "server")]
async fn get_last_time_req() -> Arc<Mutex<i64>> {
    LAST_TIME_REQ
        .get_or_init(initialize_last_time_req)
        .await
        .clone()
}

#[cfg(feature = "server")]
async fn initialize_last_story() -> Arc<Mutex<Story>> {
    Arc::new(Mutex::new(Story {
        ..Default::default()
    }))
}
#[cfg(feature = "server")]
async fn get_last_story() -> Arc<Mutex<Story>> {
    LAST_STORY.get_or_init(initialize_last_story).await.clone()
}

#[server(StoryServer)]
pub async fn get_story() -> Result<Story, ServerFnError> {
    let last_time_req = get_last_time_req().await;
    let mut last_time = last_time_req.lock().await;
    let mut should_continue = false;
    if *last_time == 0i64 {
        *last_time = get_timestamp_seconds_now();
        should_continue = true;
    } else {
        let current = get_timestamp_seconds_now();
        if (current - *last_time) > 60 * 60 {
            *last_time = get_timestamp_seconds_now();
            should_continue = true
        }
    }
    let last_result_story = get_last_story().await;
    let mut last_story = last_result_story.lock().await;
    if should_continue {
        let db = get_client_db().await;

        // Query the 'stories' collection for the latest story
        let mut story_stream = db
            .fluent()
            .select()
            .from("texts")
            .order_by([("when", FirestoreQueryDirection::Descending)])
            .limit(1)
            .obj::<Story>()
            .stream_query()
            .await
            .map_err::<ServerFnError, _>(|e| ServerFnError::ServerError(e.to_string()))?;

        // Retrieve the latest story
        match story_stream.next().await {
            Some(latest_story) => {
                let filtered_story = latest_story
                    .story
                    .replace('\n', " ")
                    .replace(",", "")
                    .replace(".", "")
                    .replace(":", "")
                    .replace(";", "");
                *last_story = latest_story.clone();
                Ok(Story {
                    title: latest_story.title,
                    sources: latest_story.sources,
                    story: filtered_story,
                    when: latest_story.when,
                })
            }
            None => Err(ServerFnError::ServerError("No stories found".into())),
        }
    } else {
        Ok(last_story.clone())
    }
}

#[cfg(feature = "server")]
async fn get_client_db() -> &'static FirestoreDb {
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
