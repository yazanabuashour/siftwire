use std::path::Path;

use anyhow::{Result, anyhow};
use url::Url;

use crate::filesystem::{create_dir_all, write_file};
use crate::types::Scenario;

const FEED_URL: &str = "https://github.blog/feed/";
const MISSING_URL: &str = "https://example.com/siftwire-missing.xml";

pub fn prepare(mut scenario: Scenario, run_dir: &Path) -> Result<Scenario> {
    let needs_feed = contains(&scenario, FEED_URL);
    let needs_missing = contains(&scenario, MISSING_URL);
    let needs_limits = scenario.id == "configured-max-delivery-items";
    let needs_history = scenario.id == "brief-run-history";
    if !(needs_feed || needs_missing || needs_limits || needs_history) {
        return Ok(scenario);
    }
    let fixture_dir = run_dir.join("fixtures");
    create_dir_all(&fixture_dir, 0o755)?;
    let feed_path = fixture_dir.join("github-blog.xml");
    if needs_feed {
        write_file(&feed_path, feed().as_bytes(), 0o644)?;
    }
    let limit_urls = if needs_limits {
        generated_feeds(&fixture_dir, "limit", "01")?
    } else {
        Vec::new()
    };
    let history_urls = if needs_history {
        generated_feeds(&fixture_dir, "history", "02")?
    } else {
        Vec::new()
    };
    rewrite(
        &mut scenario,
        &fixture_dir,
        &feed_path,
        &limit_urls,
        &history_urls,
    )?;
    Ok(scenario)
}

fn contains(scenario: &Scenario, value: &str) -> bool {
    scenario
        .turns
        .iter()
        .any(|turn| turn.prompt.contains(value))
}

fn generated_feeds(directory: &Path, kind: &str, hour: &str) -> Result<Vec<String>> {
    (1_u8..=3)
        .map(|number| {
            let path = directory.join(format!("{kind}-{number}.xml"));
            let content = generated_feed(kind, hour, number);
            write_file(&path, content.as_bytes(), 0o644)?;
            file_url(&path)
        })
        .collect()
}

fn generated_feed(kind: &str, hour: &str, number: u8) -> String {
    format!(
        "<?xml version=\"1.0\"?>\n<rss version=\"2.0\"><channel>\n<title>SiftWire {kind} fixture {number}</title>\n<item><title>SiftWire {kind} story {number}</title><link>https://fixture.example/{kind}-{number}</link><guid>{kind}-guid-{number}</guid><pubDate>Thu, 23 Apr 2026 {hour}:0{number}:00 GMT</pubDate></item>\n</channel></rss>"
    )
}

fn rewrite(
    scenario: &mut Scenario,
    fixture_dir: &Path,
    feed_path: &Path,
    limit_urls: &[String],
    history_urls: &[String],
) -> Result<()> {
    let feed_url = file_url(feed_path)?;
    let missing_url = file_url(&fixture_dir.join("missing.xml"))?;
    for turn in &mut scenario.turns {
        turn.prompt = turn
            .prompt
            .replace(FEED_URL, &feed_url)
            .replace(MISSING_URL, &missing_url);
        replace_generated(&mut turn.prompt, "limit", limit_urls);
        replace_generated(&mut turn.prompt, "history", history_urls);
    }
    Ok(())
}

fn replace_generated(prompt: &mut String, kind: &str, values: &[String]) {
    for (index, value) in (1_u8..).zip(values) {
        *prompt = prompt.replace(
            &format!("https://example.com/siftwire-{kind}-{index}.xml"),
            value,
        );
    }
}

fn file_url(path: &Path) -> Result<String> {
    Url::from_file_path(path)
        .map(String::from)
        .map_err(|()| anyhow!("cannot convert {} to a file URL", path.display()))
}

const fn feed() -> &'static str {
    "<?xml version=\"1.0\"?>\n<rss version=\"2.0\"><channel>\n<title>SiftWire fixture</title>\n<item><title>SiftWire fixture story - Fixture Outlet</title><link>https://fixture.example/story</link><guid>fixture-guid-1</guid><pubDate>Thu, 23 Apr 2026 01:00:00 GMT</pubDate></item>\n</channel></rss>"
}
