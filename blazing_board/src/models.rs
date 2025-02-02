use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Story {
    pub sources: Vec<String>,
    pub story: String,
    pub title: Option<String>,
    pub when: DateTime<Utc>,
}

impl Default for Story {
    fn default() -> Story {
        Story {
            sources: vec!["https://doc.rust-lang.org/book/".to_string()],
            story: include_str!("../assets/texts/01.txt").to_string(),
            title: Some("The Rust Programming Language".to_string()),
            when: Utc::now(),
        }
    }
}
