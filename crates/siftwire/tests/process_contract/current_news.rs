use std::fmt::Write as _;

use super::*;

#[test]
fn current_news_reoffers_unselected_stories_without_markers_and_archives_counts() -> Result<()> {
    let temp = TempDir::new()?;
    let feed = temp.path().join("news.xml");
    let database = temp
        .path()
        .join("news.sqlite")
        .to_string_lossy()
        .into_owned();
    let environment = BTreeMap::from([("SIFTWIRE_EVAL_ALLOW_FILE_URLS", "1")]);
    write_news_feed(&feed, false)?;
    let feed_url = url::Url::from_file_path(&feed).map_err(|()| anyhow::anyhow!("fixture URL"))?;
    let configured = invoke_json(
        &["config", "--db", &database],
        &json!({"action":"upsert_source", "source": {
            "key":"news", "label":"News", "kind":"rss", "url":feed_url,
            "section":"technology", "threshold":"high", "enabled":true,
            "url_canonicalization":"google_news_article_url", "outlet_extraction":"title_suffix"
        }}),
        &environment,
    )?;
    assert_eq!(
        configured.get("rejected"),
        Some(&json!(false)),
        "configuration rejected"
    );
    invoke_json(
        &["config", "--db", &database],
        &json!({"action":"set_brief_options", "max_delivery_items":5}),
        &environment,
    )?;
    let db = rusqlite::Connection::open(&database)?;
    let first = run_news(&database, &environment)?;
    let first_run: RunResult = decode(first.clone())?;
    assert_eq!(
        first_run.candidates.len(),
        7,
        "first fetch must not truncate to one or five stories"
    );
    assert_eq!(
        first_run.candidate_slots, 5,
        "delivery capacity must not cap discovery"
    );
    assert!(
        first_run
            .candidates
            .iter()
            .any(|item| item.title == "Story 6 - Outlet"),
        "tail story missing"
    );
    let count: i64 = db.query_row("SELECT COUNT(*) FROM source_state", [], |row| row.get(0))?;
    assert_eq!(count, 0, "new optional feed wrote a marker");
    assert_news_counts_and_archive(&database, &first, &environment)?;

    let selected = first_run.candidates.first().context("selected story")?;
    assert_eq!(
        selected.title, "Story 0 - Outlet",
        "fixture selection differs"
    );
    assert_eq!(
        selected.url, "file:///nonexistent/news-0",
        "optional feed URL was rewritten or decoded"
    );
    // File-backed fixture links are evidence, not renderable delivery hyperlinks.
    let message = format!("- {}\n\nFeed health changes - NEW: news.", selected.title);
    let prepared =
        assert_normal_prepared_plan(&database, &first_run.run_id, &message, &environment)?;
    let confirmed: DeliveryResult = decode(invoke_json(
        &["brief", "--db", &database],
        &json!({"action":"confirm_delivery", "run_id":first_run.run_id, "delivery_plan_id":prepared.delivery_plan_id}),
        &environment,
    )?)?;
    assert_eq!(
        confirmed.final_answer,
        format!("Current brief\n\n{message}"),
        "confirmed body differs"
    );

    assert_news_reoffer_and_failure(&database, &db, &feed, &selected.url, &environment)?;
    assert_legacy_fetch_count(&database, &db, &first_run.run_id, &environment)?;
    Ok(())
}

fn assert_news_reoffer_and_failure(
    database: &str,
    db: &rusqlite::Connection,
    feed: &std::path::Path,
    selected_url: &str,
    environment: &BTreeMap<&str, &str>,
) -> Result<()> {
    db.execute_batch("INSERT INTO source_state VALUES ('news','story-6','story-6','Old marker title','file:///nonexistent/news-6','2026-01-01','2026-01-01T00:00:00Z');")?;
    let marker = marker_bytes(db)?;
    write_news_feed(feed, true)?;
    let repeated: RunResult = decode(run_news(database, environment)?)?;
    assert_eq!(
        repeated.candidates.len(),
        7,
        "unselected stories or changed headline were lost"
    );
    assert!(
        repeated
            .candidates
            .iter()
            .any(|item| item.title == "Story 6 - Outlet"),
        "unselected tail story was not reoffered after reorder"
    );
    assert!(
        repeated
            .candidates
            .iter()
            .any(|item| item.title == "Story 0 updated - Outlet" && item.url == selected_url),
        "changed headline on identical URL must reach selection"
    );
    assert_eq!(
        repeated.suppressed_recent.len(),
        1,
        "confirmed exact headline must be suppressed"
    );
    assert_eq!(
        repeated
            .suppressed_recent
            .first()
            .and_then(|item| item.get("title")),
        Some(&json!("Story 0 - Outlet")),
        "wrong story suppressed"
    );
    assert_eq!(
        marker_bytes(db)?,
        marker,
        "successful optional fetch changed existing marker bytes"
    );

    fs::remove_file(feed)?;
    let failed = run_news(database, environment)?;
    assert_eq!(
        failed.pointer("/fetch_status/0/status"),
        Some(&json!("error")),
        "missing fixture must fail fetching"
    );
    assert_eq!(
        failed.pointer("/fetch_status/0/new_items"),
        Some(&serde_json::Value::Null),
        "failed fetch invented a count"
    );
    assert!(
        failed.pointer("/fetch_status/0/current_news").is_none(),
        "failed fetch invented window statistics"
    );
    let failed_id = failed
        .get("run_id")
        .and_then(serde_json::Value::as_str)
        .context("failed run ID")?;
    let archived = invoke_cli_json(
        &["runs", "show", failed_id, "--json", "--db", database],
        environment,
    )?;
    assert_eq!(
        archived.pointer("/fetch/0/new_items"),
        Some(&serde_json::Value::Null),
        "archive invented a count for a failed current-news check"
    );
    assert_eq!(
        marker_bytes(db)?,
        marker,
        "failed optional fetch changed marker bytes"
    );
    let sent: i64 = db.query_row("SELECT COUNT(*) FROM sent_item", [], |row| row.get(0))?;
    assert_eq!(sent, 1, "unconfirmed or failed run changed delivery state");
    Ok(())
}

fn run_news(database: &str, environment: &BTreeMap<&str, &str>) -> Result<serde_json::Value> {
    invoke_json(
        &["brief", "--db", database],
        &json!({"action":"run_brief", "dry_run":false}),
        environment,
    )
}

fn write_news_feed(path: &std::path::Path, reordered: bool) -> Result<()> {
    let now = chrono::Utc::now();
    let published = now.to_rfc2822();
    let stale = now
        .checked_sub_signed(chrono::TimeDelta::days(2))
        .context("stale date")?
        .to_rfc2822();
    let future = now
        .checked_add_signed(chrono::TimeDelta::days(2))
        .context("future date")?
        .to_rfc2822();
    // Seven stories cross the old five-item path; story 6 stays beyond it even after reorder.
    let order = if reordered {
        [5, 4, 3, 2, 1, 0, 6]
    } else {
        [0, 1, 2, 3, 4, 5, 6]
    };
    let mut body = String::from("<rss version=\"2.0\"><channel><title>News</title>");
    for index in order {
        let changed = if reordered && index == 0 {
            " updated"
        } else {
            ""
        };
        write!(
            body,
            "<item><title>Story {index}{changed} - Outlet</title><link>file:///nonexistent/news-{index}</link><guid>story-{index}</guid><pubDate>{published}</pubDate></item>"
        )?;
    }
    if reordered {
        write!(
            body,
            "<item><title>Story 0 - Outlet</title><link>file:///nonexistent/exact-repeat</link><guid>repeat</guid><pubDate>{published}</pubDate></item>"
        )?;
    }
    for (name, date) in [
        ("missing", ""),
        ("invalid", "not-a-date"),
        ("stale", stale.as_str()),
        ("future", future.as_str()),
    ] {
        write!(
            body,
            "<item><title>{name}</title><link>file:///nonexistent/{name}</link><guid>{name}</guid>"
        )?;
        if !date.is_empty() {
            write!(body, "<pubDate>{date}</pubDate>")?;
        }
        body.push_str("</item>");
    }
    body.push_str("</channel></rss>");
    fs::write(path, body)?;
    Ok(())
}

fn marker_bytes(db: &rusqlite::Connection) -> Result<String> {
    db.query_row(
        "SELECT json_array(hex(source_key),hex(latest_identity),hex(latest_feed_identity),hex(latest_title),hex(latest_url),hex(latest_published_at),hex(checked_at)) FROM source_state",
        [],
        |row| row.get(0),
    ).context("read marker bytes")
}

fn assert_news_counts_and_archive(
    database: &str,
    run: &serde_json::Value,
    environment: &BTreeMap<&str, &str>,
) -> Result<()> {
    let fetch = run.pointer("/fetch_status/0").context("fetch status")?;
    assert_eq!(
        fetch.get("items"),
        Some(&json!(11)),
        "fetch count must include ineligible items"
    );
    assert_eq!(
        fetch.get("new_items"),
        Some(&serde_json::Value::Null),
        "snapshot must not claim novelty"
    );
    let stats = fetch
        .get("current_news")
        .context("current-news statistics")?;
    for (field, expected) in [
        ("eligible_items", 7),
        ("stale_items", 1),
        ("undated_items", 2),
        ("future_items", 1),
    ] {
        assert_eq!(stats.get(field), Some(&json!(expected)), "wrong {field}");
    }
    let since = chrono::DateTime::parse_from_rfc3339(
        stats
            .get("since")
            .and_then(serde_json::Value::as_str)
            .context("window start")?,
    )?;
    let until = chrono::DateTime::parse_from_rfc3339(
        stats
            .get("until")
            .and_then(serde_json::Value::as_str)
            .context("window end")?,
    )?;
    assert_eq!(
        until.signed_duration_since(since),
        chrono::TimeDelta::hours(24),
        "window is not rolling 24 hours"
    );
    let run_id = run
        .get("run_id")
        .and_then(serde_json::Value::as_str)
        .context("run ID")?;
    let detail = invoke_cli_json(
        &["runs", "show", run_id, "--json", "--db", database],
        environment,
    )?;
    assert_eq!(
        detail.pointer("/fetch/0"),
        Some(fetch),
        "archived fetch evidence differs from process output"
    );
    Ok(())
}

fn assert_legacy_fetch_count(
    database: &str,
    db: &rusqlite::Connection,
    current_run_id: &str,
    environment: &BTreeMap<&str, &str>,
) -> Result<()> {
    db.execute_batch("INSERT INTO brief_run (id,started_at,dry_run,status,summary) VALUES ('legacy','2026-01-01T00:00:00Z',0,'ok','legacy'); INSERT INTO fetch_log (run_id,source_key,status,error,item_count,new_item_count,created_at,source_label) VALUES ('legacy','news','ok','',11,4,'2026-01-01T00:00:00Z','Old news'), ('legacy','failed','error','old failure',0,0,'2026-01-01T00:00:00Z','Old failure');")?;
    let detail = invoke_cli_json(
        &["runs", "show", "legacy", "--json", "--db", database],
        environment,
    )?;
    assert_eq!(
        detail.pointer("/fetch/0/new_items"),
        Some(&json!(4)),
        "historical numeric count changed"
    );
    assert_eq!(
        detail.pointer("/fetch/1/new_items"),
        Some(&json!(0)),
        "historical failure placeholder was reinterpreted"
    );
    assert!(
        detail.pointer("/fetch/0/current_news").is_none(),
        "legacy count was relabeled as current news"
    );
    // A nonzero legacy storage column must stay unused when snapshot evidence exists.
    db.execute(
        "UPDATE fetch_log SET new_item_count = 4 WHERE run_id = ?1",
        [current_run_id],
    )?;
    let current = invoke_cli_json(
        &["runs", "show", current_run_id, "--json", "--db", database],
        environment,
    )?;
    assert_eq!(
        current.pointer("/fetch/0/new_items"),
        Some(&serde_json::Value::Null),
        "raw legacy storage count leaked into snapshot output"
    );
    assert_eq!(
        current.pointer("/fetch/0/current_news/eligible_items"),
        Some(&json!(7)),
        "snapshot evidence changed"
    );
    Ok(())
}
