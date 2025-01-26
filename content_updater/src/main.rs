use anyhow::{anyhow, Result};
use std::{thread, time};
use headless_chrome::{Browser, LaunchOptions};

use genai::chat::printer::print_chat_stream;
use genai::chat::{ChatMessage, ChatRequest};
use genai::Client;

const MODEL: &str = "gpt-4o";

struct HNQueryResult {
    link: String,
    title: String,
    raw_text: String,
}

struct TypingTextEntry {
    sources: Vec<String>,
    story: String,
}

async fn query() -> Result<HNQueryResult> {
    let browser = Browser::new(
        LaunchOptions::default_builder()
            .sandbox(false)
            .build()
            .expect("Could not find chrome-executable"),
    )?;
    let tab = browser.new_tab()?;

    // Navigate to the page
    tab.navigate_to("https://hn.algolia.com/?dateRange=last24h&page=0&prefix=false&query=&sort=byPopularity&type=story")?;

    // Allow some time for the page to load
    thread::sleep(time::Duration::from_secs(3));

    // Find the first story
    let first_story = tab.find_element("article.Story")?;

    // Locate the comments link in the first story
    let comments_link: headless_chrome::Element<'_> = first_story.find_element("a")?;
    // Extract the href attribute of the comments link
    let href = comments_link.get_attribute_value("href")?;
    if let Some(link) = href {
        tab.navigate_to(&link)?;
        thread::sleep(time::Duration::from_secs(3));
        let comment_tree = tab.find_element("table.comment-tree")?;
        let all_comments_text = comment_tree.get_inner_text()?;
        let hn_qr = HNQueryResult {
            link: link,
            title: tab.get_title()?,
            raw_text: all_comments_text,
        };
        Ok(hn_qr)
    } else {
        println!("No comments link found.");
        Err(anyhow!("No comments link found."))
    }
}

async fn generate_typing_text_entry(hn_query_result: &HNQueryResult) -> Result<TypingTextEntry> {
    let client = Client::default();
    let chat_req_str = format!("For a typing training program, you need to create a positive and interesting text based the hacker news article {}. You will need to infer the content of the article from the comments, please answer close to 250 words in lowercase and without ponctuation.", hn_query_result.title);
	let mut chat_req = ChatRequest::default().with_system(chat_req_str);
	// This is similar to sending initial system chat messages (which will be cumulative with system chat messages)

    let question = hn_query_result.raw_text.clone();

    chat_req = chat_req.append_message(ChatMessage::user(question));

    println!("\n--- Answer:");
    let chat_res = client.exec_chat(MODEL, chat_req.clone(), None).await?;

    if let Some(text_result) = chat_res.content_text_as_str() {
        Ok(TypingTextEntry {
            sources: vec![hn_query_result.link.clone()],
            story: text_result.to_string()
        })
    }
    else {
        Err(anyhow!("Issue with the LLM answers"))
    }

}

#[tokio::main]
async fn main() -> Result<()> {
    if let Ok(hn_qr) = query().await  {
        println!("{}", hn_qr.title);
        println!("{}", hn_qr.link);
        if let Ok(story_entry) = generate_typing_text_entry(&hn_qr).await {
            println!("{}", story_entry.story);
            Ok(())
        }
        else {
            Err(anyhow!("Unable to generate the story"))
        }
    }
    else {
        Err(anyhow!("Issue while scraping"))
    }
}
