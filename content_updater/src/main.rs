use anyhow::{anyhow, Result};
use std::{thread, time};
use headless_chrome::{Browser, LaunchOptions};

struct HNQueryResult {
    link: String,
    title: String,
    raw_text: String,
}

fn query() -> Result<HNQueryResult> {
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

fn main() -> Result<()> {
    if let Ok(hn_qr) = query()  {
        println!("{}", hn_qr.raw_text);
        println!("{}", hn_qr.title);
        println!("{}", hn_qr.link);
        Ok(())
    }
    else {
        Err(anyhow!("Issue while scraping"))
    }
}
