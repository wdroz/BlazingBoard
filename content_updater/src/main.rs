use anyhow::Result;
use std::{thread, time};
use headless_chrome::{Browser, LaunchOptions};

fn query() -> Result<()> {
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

    println!("first story ok");

    // Locate the comments link in the first story
    let comments_link: headless_chrome::Element<'_> = first_story.find_element("a")?;
    println!("comments link ok");
    // Extract the href attribute of the comments link
    let href = comments_link.get_attribute_value("href")?;
    println!("href ok");
    if let Some(link) = href {
        println!("Comments link: {}", link);
        tab.navigate_to(&link)?;
        thread::sleep(time::Duration::from_secs(3));
        let comment_tree = tab.find_element("table.comment-tree")?;
        let all_comments_text = comment_tree.get_inner_text()?;
        println!("{}", all_comments_text);
        println!("{}", tab.get_title()?);
    } else {
        println!("No comments link found.");
    }
    Ok(())
}

fn main() -> Result<()> {
    query()
}
