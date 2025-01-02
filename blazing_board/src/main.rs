use dioxus::prelude::*;

const FAVICON: Asset = asset!("/assets/favicon.ico");
const MAIN_CSS: Asset = asset!("/assets/main.css");
const HEADER_MAIN: Asset = asset!("assets/logo_blazing_board.png");

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    rsx! {
        document::Link { rel: "icon", href: FAVICON }
        document::Link { rel: "stylesheet", href: MAIN_CSS }
        TypingWords {}
    }
}

#[component]
pub fn TypingWords() -> Element {
    let mut current_word_indice = use_signal(|| 0);
    let mut current_text = use_signal(|| String::new());
    let mut user_words = use_signal(|| Vec::<String>::new());
    let response_sentence_to_write = use_resource(|| async move {
        get_text()
            .await
            .unwrap_or("Please write this text".to_string())
    });
    let sentence_to_write_words = response_sentence_to_write()
        .unwrap_or("Please write this text".to_string())
        .split_whitespace()
        .map(|w| w.to_string())
        .collect::<Vec<String>>();
    let nb_words_to_write = sentence_to_write_words.len();
    let mut nb_correct = 0;
    let mut nb_wrong = 0;
    let mut accuracy = 0.0;
    rsx! {
        div { id: "TypingWords",
            img { src: HEADER_MAIN, id: "main" }
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
            if user_words().len() < nb_words_to_write {
                input {
                    id: "textUser",
                    oninput: move |event| {
                        async move {
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
                    accuracy = f64::from(nb_correct) / f64::from(nb_correct + nb_wrong);
                }
                div { "Accuracy:  {nb_correct} / {nb_correct + nb_wrong} = {accuracy}" }
            }
        }
    }
}

#[server(TextServer)]
async fn get_text() -> Result<String, ServerFnError> {
    Ok("Please write this text".to_string())
}
