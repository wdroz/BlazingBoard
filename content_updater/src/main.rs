use anyhow::{anyhow, Result};
use std::{thread, time};
use headless_chrome::{Browser, LaunchOptions};
use serde::{Deserialize, Serialize};
use firestore::{FirestoreDb, FirestoreDbOptions, FirestoreQueryDirection, FirestoreResult};
use genai::chat::{ChatMessage, ChatRequest};
use genai::Client;
use chrono::{DateTime, Utc};
use std::env;

const MODEL: &str = "gpt-4o";

struct HNQueryResult {
    link: String,
    title: String,
    raw_text: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct Story {
    sources: Vec<String>,
    story: String,
    #[serde(with = "firestore::serialize_as_timestamp")]
    when: DateTime<Utc>,
}

async fn get_db() -> Result<FirestoreDb>{
    dotenvy::dotenv().ok();

    let project_id = env::var("PROJECT_ID").expect("PROJECT_ID not set");
    let database_id = env::var("DATABASE_ID").expect("DATABASE_ID not set");

    // Check for the "IAMTHEDEV" environment variable
    if env::var("IAMTHEDEV").is_err() {
        let db: FirestoreDb = FirestoreDb::with_options(
            FirestoreDbOptions::new(project_id).with_database_id(database_id),
        )
        .await
        .expect("Failed to initialize FirestoreDb using PROJECT_ID and DATABASE_ID");
        return Ok(db);
    }

    // Initialize Firestore client using service account key file
    let db = FirestoreDb::with_options_service_account_key_file(
        FirestoreDbOptions::new(project_id).with_database_id(database_id),
        "key.json".into(),
    )
    .await
    .expect("Failed to initialize FirestoreDb using service account key file");

    Ok(db)
}

async fn save_story(text_entry: &Story) -> Result<()> {
    if let Ok(db) = get_db().await {
        let object_returned: Story = db.fluent()
            .insert()
            .into("texts")
            .generate_document_id()
            .object(text_entry)
            .execute()
            .await?;
    }
    Ok(())
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

async fn generate_typing_text_entry(hn_query_result: &HNQueryResult) -> Result<Story> {
    let client = Client::default();
    let chat_req_str = format!("For a typing training program, you need to create a positive and interesting text based the hacker news article {}. You will need to infer the content of the article from the comments, please answer close to 250 words in lowercase and without ponctuation.", hn_query_result.title);
	let mut chat_req = ChatRequest::default().with_system(chat_req_str);
	// This is similar to sending initial system chat messages (which will be cumulative with system chat messages)

    let question = hn_query_result.raw_text.clone();

    chat_req = chat_req.append_message(ChatMessage::user(question));

    println!("\n--- Answer:");
    let chat_res = client.exec_chat(MODEL, chat_req.clone(), None).await?;

    if let Some(text_result) = chat_res.content_text_as_str() {
        Ok(Story {
            sources: vec![hn_query_result.link.clone()],
            story: text_result.to_string(),
            when: Utc::now(),
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
            match save_story(&story_entry).await {
                Ok(()) => Ok(()),
                _ => Err(anyhow!("Unable to save the story to the cloud")),
            }
        }
        else {
            Err(anyhow!("Unable to generate the story"))
        }
    }
    else {
        Err(anyhow!("Issue while scraping"))
    }
}
