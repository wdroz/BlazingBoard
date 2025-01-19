use async_std::task::sleep;
use dioxus::prelude::*;
use jiff::Timestamp;
use wasm_bindgen::prelude::*;

#[cfg(feature = "server")]
use chrono::{DateTime, Utc};
#[cfg(feature = "server")]
use firestore::{FirestoreDb, FirestoreDbOptions, FirestoreQueryDirection, FirestoreResult};
#[cfg(feature = "server")]
use futures::stream::StreamExt;
#[cfg(feature = "server")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "server")]
use std::env;
use std::sync::Arc;
#[cfg(feature = "server")]
use tokio::sync::{Mutex, OnceCell};
#[cfg(feature = "server")]
#[cfg(feature = "server")]
#[derive(Debug, Clone, Deserialize, Serialize)]
struct Story {
    sources: Vec<String>,
    story: String,
    when: DateTime<Utc>,
}

#[cfg(feature = "server")]
static CLIENT: OnceCell<FirestoreDb> = OnceCell::const_new();
#[cfg(feature = "server")]
static LAST_TIME_REQ: OnceCell<Arc<Mutex<i64>>> = OnceCell::const_new();
#[cfg(feature = "server")]
static LAST_TIME_REQ_RESULT: OnceCell<Arc<Mutex<String>>> = OnceCell::const_new();

const NO_JS_MESSAGE: &str = "This site requires JavaScript to function properly";
const DEFAULT_TEXT: &str = include_str!("../assets/texts/01.txt");

const FAVICON: Asset = asset!("/assets/favicon.ico");
const MAIN_CSS: Asset = asset!("/assets/main.css");
const GITHUB_LOGO: Asset = asset!("/assets/github_logo.png");
const HEADER_MAIN: Asset = asset!("assets/logo_blazing_board.png");

fn main() {
    dioxus::launch(App);
}

#[wasm_bindgen]
pub fn get_timestamp_seconds_now_wasm() -> i64 {
    let now: Timestamp = Timestamp::now();
    now.as_second()
}

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
async fn initialize_last_time_req_result() -> Arc<Mutex<String>> {
    Arc::new(Mutex::new(DEFAULT_TEXT.to_string()))
}
#[cfg(feature = "server")]
async fn get_last_time_req_result() -> Arc<Mutex<String>> {
    LAST_TIME_REQ_RESULT
        .get_or_init(initialize_last_time_req_result)
        .await
        .clone()
}

#[component]
fn App() -> Element {
    rsx! {
        document::Link { rel: "icon", href: FAVICON }
        document::Link { rel: "stylesheet", href: MAIN_CSS }
        a {
            id: "gh",
            href: "https://github.com/wdroz/BlazingBoard",
            target: "_blank",
            img { src: GITHUB_LOGO }
            span { "repo" }
        }
        TypingWords {}
    }
}

#[component]
pub fn TypingWords() -> Element {
    let mut current_chunk_index = use_signal(|| 0);
    let mut current_word_in_chunk_index = use_signal(|| 0);
    let mut current_text = use_signal(|| String::new());
    let mut user_words = use_signal(|| Vec::<String>::new());
    let mut start_typing_at = use_signal(|| None);
    let mut all_nb_correct = use_signal(|| 0);
    let mut all_nb_wrong = use_signal(|| 0);

    let response_sentence_to_write =
        use_resource(|| async move { get_text().await.unwrap_or(NO_JS_MESSAGE.to_string()) });
    let sentence_to_write_words = response_sentence_to_write()
        .unwrap_or(NO_JS_MESSAGE.to_string())
        .split_whitespace()
        .map(|w| w.to_string())
        .collect::<Vec<String>>();

    // Group words into chunks of 15
    let sentence_to_write_chunks = sentence_to_write_words
        .chunks(15)
        .map(|chunk| chunk.to_vec())
        .collect::<Vec<Vec<String>>>();

    let nb_chunks_to_write = sentence_to_write_chunks.len();
    let mut nb_correct = 0;
    let mut nb_wrong = 0;
    let mut accuracy = 0.0;
    let mut nb_seconds = 0;
    let mut wpm = 0.0;

    let mut timer_value = use_signal(|| 60);
    let mut time_should_decrement = use_signal(|| false);

    let _ = use_coroutine(move |_: UnboundedReceiver<i32>| async move {
        loop {
            sleep(std::time::Duration::from_secs(1)).await;
            if time_should_decrement() {
                timer_value.set(timer_value() - 1);
            }
        }
    });

    let current_chunk = {
        let index = current_chunk_index();
        if index < nb_chunks_to_write {
            sentence_to_write_chunks[index].clone()
        } else {
            vec![]
        }
    };
    let next_chunk = {
        let index = current_chunk_index();
        if index + 1 < nb_chunks_to_write {
            sentence_to_write_chunks[index + 1].clone()
        } else {
            vec![]
        }
    };

    rsx! {
        div { id: "TypingWords",
            img { src: HEADER_MAIN, id: "main" }
            div { id: "timer", "{timer_value}" }
            div { id: "words",
                for (i , word) in current_chunk.iter().enumerate() {
                    if i < current_word_in_chunk_index() {
                        if user_words().len() > i {
                            if user_words()[i] == *word.clone() {
                                {
                                    nb_correct += 1;
                                }
                                div { class: "previous_correct", "{word}" }
                            } else {
                                {
                                    nb_wrong += 1;
                                }
                                div { class: "previous_wrong", "{word}" }
                            }
                        }
                    } else if i == current_word_in_chunk_index() {
                        div { id: "current", "{word}" }
                    } else {
                        div { "{word}" }
                    }
                }
                div { class: "break" }
                for word in next_chunk.iter() {
                    div { "{word}" }
                }
            }
            if current_chunk_index() < nb_chunks_to_write && timer_value() > 0 {
                input {
                    id: "textUser",
                    oninput: move |event| {
                        let current_chunk_clone = current_chunk.clone();
                        async move {
                            time_should_decrement.set(true);
                            if start_typing_at().is_none() {
                                start_typing_at.set(Some(get_timestamp_seconds_now_wasm()));
                            }
                            let data = event.value();
                            if data.ends_with(" ") {
                                let mut new_words = user_words().to_vec();
                                new_words.push(data.trim().to_string());
                                user_words.set(new_words);
                                let next_word_index = current_word_in_chunk_index() + 1;
                                if next_word_index >= current_chunk_clone.len() {
                                    current_word_in_chunk_index.set(0);
                                    current_chunk_index.set(current_chunk_index() + 1);
                                    all_nb_correct.set(all_nb_correct() + nb_correct);
                                    all_nb_wrong.set(all_nb_wrong() + nb_wrong);
                                    nb_correct = 0;
                                    nb_wrong = 0;
                                    user_words.set(vec![]);
                                } else {
                                    current_word_in_chunk_index.set(next_word_index);
                                }
                                current_text.set(String::new());
                            } else {
                                current_text.set(data.clone());
                            }
                        }
                    },
                    value: "{current_text}",
                    autofocus: true,
                }
            } else {
                {
                    all_nb_correct();
                    all_nb_wrong();
                    time_should_decrement.set(false);
                    accuracy = f64::from(all_nb_correct() + nb_correct)
                        / f64::from(all_nb_correct() + nb_correct + all_nb_wrong() + nb_wrong);
                    if let Some(start_typing_at_some) = start_typing_at() {
                        nb_seconds = get_timestamp_seconds_now_wasm() - start_typing_at_some;
                        let nb_minutes = nb_seconds as f64 / 60.0;
                        wpm = f64::from(all_nb_correct() + nb_correct) / nb_minutes;
                    }
                }
                div {
                    "Accuracy:  {all_nb_correct() + nb_correct} / {all_nb_correct() + nb_correct + all_nb_wrong() + nb_wrong} = {100.0 * accuracy:.0}%"
                }
                div { "time(s):  {nb_seconds}" }
                div { "wpm:  {wpm:.0}" }
            }
        }
    }
}

#[server(TextServer)]
async fn get_text() -> Result<String, ServerFnError> {
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
    let last_time_req_result = get_last_time_req_result().await;
    let mut last_time_result = last_time_req_result.lock().await;
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
                let filtered_story = latest_story.story.replace('\n', " ");
                *last_time_result = filtered_story.clone();
                Ok(filtered_story)
            },
            None => Err(ServerFnError::ServerError("No stories found".into())),
        }
    } else {

        Ok((*last_time_result.clone()).to_string())
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
