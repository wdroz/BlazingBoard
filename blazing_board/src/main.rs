use async_std::task::sleep;
use dioxus::prelude::*;
use jiff::Timestamp;
use wasm_bindgen::prelude::*;

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
    let mut current_word_indice = use_signal(|| 0);
    let mut current_text = use_signal(|| String::new());
    let mut user_words = use_signal(|| Vec::<String>::new());
    let mut start_typing_at = use_signal(|| None);
    let response_sentence_to_write = use_resource(|| async move {
        get_text()
            .await
            .unwrap_or(NO_JS_MESSAGE.to_string())
    });
    let sentence_to_write_words = response_sentence_to_write()
        .unwrap_or(NO_JS_MESSAGE.to_string())
        .split_whitespace()
        .map(|w| w.to_string())
        .collect::<Vec<String>>();
    let nb_words_to_write = sentence_to_write_words.len();
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

    rsx! {
        div { id: "TypingWords",
            img { src: HEADER_MAIN, id: "main" }
            div { id: "timer", "{timer_value}" }
            div { id: "words",
                for (i , word) in sentence_to_write_words.into_iter().enumerate() {
                    if i < current_word_indice() {
                        if user_words().len() - 1 >= i {
                            if user_words()[i] == word {
                                {
                                    nb_correct = nb_correct + 1;
                                }
                                div { class: "previous_correct", "{word}" }
                            } else {
                                {
                                    nb_wrong = nb_wrong + 1;
                                }
                                div { class: "previous_wrong", "{word}" }
                            }
                        }
                    } else if i == current_word_indice() {
                        div { id: "current", "{word}" }
                    } else {
                        div { "{word}" }
                    }
                }
            }
            if user_words().len() < nb_words_to_write && timer_value() > 0 {
                input {
                    id: "textUser",
                    oninput: move |event| {
                        async move {
                            time_should_decrement.set(true);
                            if start_typing_at().is_none() {
                                start_typing_at.set(Some(get_timestamp_seconds_now()));
                            }
                            let data = event.value();
                            let words: Vec<&str> = data.split(" ").collect();
                            current_word_indice.set((words.len() - 1) + current_word_indice());
                            if let Some(last) = words.last() {
                                if data.ends_with(" ") {
                                    let mut newvec = user_words().to_vec();
                                    for w in words.clone() {
                                        if w != " " && w != "" {
                                            newvec.push(w.to_string());
                                        }
                                    }
                                    user_words.set(newvec);
                                }
                                current_text.set(last.to_string());
                            }
                        }
                    },
                    value: "{current_text}",
                }
            } else {
                {
                    time_should_decrement.set(false);
                    accuracy = f64::from(nb_correct) / f64::from(nb_correct + nb_wrong);
                    if let Some(start_typing_at_some) = start_typing_at() {
                        nb_seconds = get_timestamp_seconds_now() - start_typing_at_some;
                        let nb_minutes = nb_seconds as f64 / 60.0;
                        wpm = f64::from(nb_correct) / nb_minutes;
                    }
                }
                div { "Accuracy:  {nb_correct} / {nb_correct + nb_wrong} = {accuracy}" }
                div { "time(s):  {nb_seconds}" }
                div { "wpm:  {wpm}" }
            }
        }
    }
}

#[server(TextServer)]
async fn get_text() -> Result<String, ServerFnError> {
    let my_str = DEFAULT_TEXT;
    Ok(my_str.replace('\n', " ").to_string())
}
