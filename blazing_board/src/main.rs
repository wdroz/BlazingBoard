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
        Hero {}
        Echo {}
    }
}

#[component]
pub fn Hero() -> Element {
    let sentence_to_write = "Please write this text";
    let mut current_word_indice = use_signal(|| 0);
    let mut current_text =use_signal(|| String::new()); 
    rsx! {
        div { id: "hero",
            img { src: HEADER_MAIN, id: "main" }
            div { id: "words",
                for (i , word) in sentence_to_write.split(" ").enumerate() {
                    if i == current_word_indice() {
                        div { id: "current", "{word}" }
                    } else {
                        div { "{word}" }
                    }
                }
            }
            label { id: "textToWrite", "Please write this text" }
            input {
                id: "textUser",
                oninput: move |event| async move {
                    let data = event.value();
                    let words: Vec<&str> = data.split(" ").collect();
                    current_word_indice.set((words.len() - 1) + current_word_indice());
                    if let Some(last) = words.last() {
                        current_text.set(last.to_string());
                    }
                },
                value: "{current_text}",
            }
        }
    }
}

/// Echo component that demonstrates fullstack server functions.
#[component]
fn Echo() -> Element {
    let mut response = use_signal(|| String::new());

    rsx! {
        div { id: "echo",
            h4 { "ServerFn Echo" }
            input {
                placeholder: "Type here to echo...",
                oninput: move |event| async move {
                    let data = echo_server(event.value()).await.unwrap();
                    response.set(data);
                },
            }

            if !response().is_empty() {
                p {
                    "Server echoed: "
                    i { "{response}" }
                }
            }
        }
    }
}

/// Echo the user input on the server.
#[server(EchoServer)]
async fn echo_server(input: String) -> Result<String, ServerFnError> {
    Ok(input)
}
