#[cfg(feature = "server")]
mod auth;
mod backend;
mod components;
mod models;

use async_std::task::sleep;
use backend::{get_private_profile, get_story, save_typing_result};
use components::{
    avatar::{AvatarImageSize, ImageAvatar},
    button::{Button, ButtonSize, ButtonVariant},
};
use dioxus::prelude::*;
use jiff::Timestamp;
use models::{PrivateProfile, Story, TypingSubmission, calculate_typing_metrics};
use wasm_bindgen::prelude::*;

const DEFAULT_TITLE: &str = "";
const TEST_DURATION_SECONDS: i64 = 60;

const FAVICON: Asset = asset!("/assets/favicon.ico");
const APP_CSS: Asset = asset!("/assets/main.css");
const COMPONENTS_CSS: Asset = asset!("/assets/dx-components-theme.css");
const GITHUB_LOGO: Asset = asset!("/assets/github_logo.png");
const HEADER_MAIN: Asset = asset!("assets/logo_blazing_board.png");

fn main() {
    #[cfg(feature = "server")]
    dioxus::serve(|| async move {
        use dioxus::server::axum::routing::{get, post};

        Ok(dioxus::server::router(App)
            .route("/auth/github", get(auth::github_login))
            .route("/auth/github/callback", get(auth::github_callback))
            .route("/auth/logout", post(auth::github_logout)))
    });

    #[cfg(not(feature = "server"))]
    dioxus::launch(App);
}

#[wasm_bindgen]
pub fn get_timestamp_seconds_now_wasm() -> i64 {
    let now: Timestamp = Timestamp::now();
    now.as_second()
}

#[wasm_bindgen]
pub fn get_timestamp_milliseconds_now_wasm() -> i64 {
    let now: Timestamp = Timestamp::now();
    now.as_millisecond()
}

#[component]
fn App() -> Element {
    rsx! {
        document::Link { rel: "icon", href: FAVICON }
        document::Link { rel: "stylesheet", href: COMPONENTS_CSS }
        document::Link { rel: "stylesheet", href: APP_CSS }
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
    let mut current_text = use_signal(String::new);
    let mut user_words = use_signal(Vec::<String>::new);
    let mut started_at = use_signal(|| None::<i64>);
    let mut run_id = use_signal(String::new);
    let mut correct_words = use_signal(|| 0_i64);
    let mut wrong_words = use_signal(|| 0_i64);
    let mut duration_seconds = use_signal(|| 0_i64);
    let mut timer_value = use_signal(|| TEST_DURATION_SECONDS);
    let mut running = use_signal(|| false);
    let mut finished = use_signal(|| false);
    let mut submitted_run = use_signal(|| None::<String>);
    let mut save_message = use_signal(String::new);

    let re_story = use_resource(|| async move {
        get_story().await.unwrap_or(Story {
            ..Default::default()
        })
    });
    let profile_resource =
        use_resource(|| async move { get_private_profile().await.unwrap_or(None) });

    let story = re_story().unwrap_or(Story {
        ..Default::default()
    });
    let profile = profile_resource().unwrap_or(None);
    let last_title = story
        .title
        .clone()
        .unwrap_or_else(|| DEFAULT_TITLE.to_string());
    let sentence_to_write_words = story
        .story
        .split_whitespace()
        .map(|w| w.to_string())
        .collect::<Vec<String>>();

    let sentence_to_write_chunks = sentence_to_write_words
        .chunks(15)
        .map(|chunk| chunk.to_vec())
        .collect::<Vec<Vec<String>>>();

    let nb_chunks_to_write = sentence_to_write_chunks.len();

    let _ = use_coroutine(move |_: UnboundedReceiver<i32>| async move {
        loop {
            sleep(std::time::Duration::from_secs(1)).await;
            if running() {
                if timer_value() <= 1 {
                    timer_value.set(0);
                    duration_seconds.set(TEST_DURATION_SECONDS);
                    running.set(false);
                    finished.set(true);
                } else {
                    timer_value.set(timer_value() - 1);
                }
            }
        }
    });

    use_effect(move || {
        let profile_is_loaded = profile_resource().unwrap_or(None).is_some();
        let current_run_id = run_id();
        let should_save = finished()
            && profile_is_loaded
            && !current_run_id.is_empty()
            && submitted_run().as_deref() != Some(current_run_id.as_str());

        if should_save {
            submitted_run.set(Some(current_run_id.clone()));
            save_message.set("Saving result…".to_string());
            let submission = TypingSubmission {
                run_id: current_run_id,
                story_when: story.when,
                correct_words: correct_words(),
                wrong_words: wrong_words(),
                duration_seconds: duration_seconds(),
            };
            let mut profile_resource = profile_resource;

            spawn(async move {
                match save_typing_result(submission).await {
                    Ok(_) => {
                        save_message.set("Saved to your private history.".to_string());
                        profile_resource.restart();
                    }
                    Err(_) => {
                        save_message.set("This result could not be saved.".to_string());
                    }
                }
            });
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

    let metrics =
        calculate_typing_metrics(correct_words(), wrong_words(), duration_seconds().max(1)).ok();
    let accuracy_percent = metrics
        .map(|current| current.accuracy * 100.0)
        .unwrap_or(0.0);
    let wpm = metrics.map(|current| current.wpm).unwrap_or(0.0);
    let score = metrics.map(|current| current.score).unwrap_or(0);

    rsx! {
        div { id: "TypingWords",
            ProfileBar { profile: profile.clone() }
            div { id: "TypingTitle", "{last_title}" }
            img { src: HEADER_MAIN, id: "brand-logo" }
            div { id: "timer", "{timer_value}" }
            div { id: "words",
                for (i , word) in current_chunk.iter().enumerate() {
                    if i < current_word_in_chunk_index() {
                        if user_words().len() > i {
                            if user_words()[i] == *word {
                                div { class: "previous_correct", "{word}" }
                            } else {
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
            if !running() && !finished() {
                div { id: "tips",
                    "Write as quickly as you can, pressing the space bar after each word."
                }
            }
            if current_chunk_index() < nb_chunks_to_write && !finished() {
                input {
                    id: "textUser",
                    oninput: move |event| {
                        let current_chunk_clone = current_chunk.clone();
                        async move {
                            if started_at().is_none() {
                                started_at.set(Some(get_timestamp_seconds_now_wasm()));
                                run_id.set(format!(
                                    "run-{}",
                                    get_timestamp_milliseconds_now_wasm()
                                ));
                                running.set(true);
                            }
                            let data = event.value();
                            if data.ends_with(" ") {
                                let typed_word = data.trim().to_string();
                                let mut new_words = user_words().to_vec();
                                new_words.push(typed_word.clone());
                                user_words.set(new_words);

                                let word_index = current_word_in_chunk_index();
                                if current_chunk_clone
                                    .get(word_index)
                                    .is_some_and(|expected| expected == &typed_word)
                                {
                                    correct_words.set(correct_words() + 1);
                                } else {
                                    wrong_words.set(wrong_words() + 1);
                                }

                                let next_word_index = word_index + 1;
                                if next_word_index >= current_chunk_clone.len() {
                                    let next_chunk_index = current_chunk_index() + 1;
                                    if next_chunk_index >= nb_chunks_to_write {
                                        current_word_in_chunk_index.set(next_word_index);
                                        duration_seconds.set(
                                            started_at()
                                                .map(|start| {
                                                    (get_timestamp_seconds_now_wasm() - start)
                                                        .clamp(1, 600)
                                                })
                                                .unwrap_or(1),
                                        );
                                        running.set(false);
                                        finished.set(true);
                                    } else {
                                        current_word_in_chunk_index.set(0);
                                        current_chunk_index.set(next_chunk_index);
                                        user_words.set(vec![]);
                                    }
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
                    autocomplete: "off",
                    autocapitalize: "off",
                    spellcheck: "false",
                    aria_label: "Type the highlighted word",
                }
            } else {
                section { id: "results",
                    h2 { "Run complete" }
                    div { class: "result-grid",
                        ResultStat { label: "WPM", value: format!("{wpm:.0}") }
                        ResultStat {
                            label: "Accuracy",
                            value: format!("{accuracy_percent:.0}%"),
                        }
                        ResultStat { label: "Score", value: score.to_string() }
                        ResultStat {
                            label: "Time",
                            value: format!("{}s", duration_seconds()),
                        }
                    }
                    Button {
                        size: ButtonSize::Lg,
                        onclick: move |_| {
                            current_chunk_index.set(0);
                            current_word_in_chunk_index.set(0);
                            current_text.set(String::new());
                            user_words.set(Vec::new());
                            started_at.set(None);
                            run_id.set(String::new());
                            correct_words.set(0);
                            wrong_words.set(0);
                            duration_seconds.set(0);
                            timer_value.set(TEST_DURATION_SECONDS);
                            running.set(false);
                            finished.set(false);
                            submitted_run.set(None);
                            save_message.set(String::new());
                        },
                        "Try again"
                    }
                    if !save_message().is_empty() {
                        p { class: "save-message", "{save_message}" }
                    } else if profile.is_none() {
                        p { class: "save-message",
                            "Sign in with GitHub to keep future results."
                        }
                    }
                }

                if let Some(private_profile) = profile.clone() {
                    HistoryPanel { profile: private_profile }
                }

                div { class: "sources",
                    div { "Text sources" }
                    for source in story.sources {
                        a { href: source, target: "_blank", rel: "noreferrer", "{last_title}" }
                    }
                }
            }
        }
    }
}

#[component]
fn ProfileBar(profile: Option<PrivateProfile>) -> Element {
    rsx! {
        header { class: "profile-bar",
            if let Some(private_profile) = profile {
                div { class: "profile-identity",
                    ImageAvatar {
                        size: AvatarImageSize::Small,
                        src: private_profile.user.avatar_url.clone(),
                        alt: format!("{}'s GitHub avatar", private_profile.user.login),
                        "{private_profile.user.login.chars().next().unwrap_or('?')}"
                    }
                    span { "@{private_profile.user.login}" }
                }
                form { action: "/auth/logout", method: "post",
                    Button {
                        r#type: "submit",
                        size: ButtonSize::Sm,
                        variant: ButtonVariant::Ghost,
                        "Log out"
                    }
                }
            } else {
                form { action: "/auth/github", method: "get",
                    Button {
                        r#type: "submit",
                        size: ButtonSize::Sm,
                        variant: ButtonVariant::Outline,
                        "Log in with GitHub"
                    }
                }
            }
        }
    }
}

#[component]
fn ResultStat(label: &'static str, value: String) -> Element {
    rsx! {
        div { class: "result-stat",
            strong { "{value}" }
            span { "{label}" }
        }
    }
}

#[component]
fn HistoryPanel(profile: PrivateProfile) -> Element {
    rsx! {
        section { class: "history-panel",
            h2 { "Private profile" }
            div { class: "profile-summary",
                span { "{profile.user.total_runs} runs" }
                span { "Best {profile.user.best_wpm:.0} WPM" }
                span { "Best score {profile.user.best_score}" }
            }
            if profile.history.is_empty() {
                p { "Your completed runs will appear here." }
            } else {
                div { class: "history-list",
                    for result in profile.history.iter() {
                        div { class: "history-row", key: "{result.run_id}",
                            div {
                                strong { "{result.score} pts" }
                                span { "{result.story_title}" }
                            }
                            div { class: "history-metrics",
                                span { "{result.wpm:.0} WPM" }
                                span { "{result.accuracy * 100.0:.0}%" }
                                span { "{result.created_at.format(\"%Y-%m-%d\")}" }
                            }
                        }
                    }
                }
            }
        }
    }
}
