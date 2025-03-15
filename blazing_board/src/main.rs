mod backend;
mod models;

use async_std::task::sleep;
use dioxus::prelude::*;
use jiff::Timestamp;
use models::Story;
use wasm_bindgen::prelude::*;

use backend::get_story;

const DEFAULT_TITLE: &str = "";

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
    let mut current_text = use_signal(String::new);
    let mut user_words = use_signal(Vec::<String>::new);
    let mut start_typing_at = use_signal(|| None);
    let mut all_nb_correct = use_signal(|| 0);
    let mut all_nb_wrong = use_signal(|| 0);
    let re_story = use_resource(|| async move {
        get_story().await.unwrap_or(Story {
            ..Default::default()
        })
    });
    let story = re_story().unwrap_or(Story {
        ..Default::default()
    });
    let last_title = story.clone().title.clone().unwrap_or(DEFAULT_TITLE.to_string());
    let sentence_to_write_words = story
        .story
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
            div { id: "TypingTitle", "{last_title}" }
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
            if timer_value() == 60 {
                div { id: "tips",
                    "Write as quickly as you can, pressing the space bar after each word."
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
                div {
                    div { "Sources of the text" }
                    for source in story.sources {
                        div {
                            a { href: source, "{last_title}" }
                        }
                    }
                }
            }
        }
    }
}
