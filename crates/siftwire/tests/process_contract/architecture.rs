use super::*;

#[test]
fn config_preserves_raw_sources_cursors_and_historical_reporting() -> Result<()> {
    let temp = TempDir::new()?;
    let database = temp
        .path()
        .join("config.sqlite")
        .to_string_lossy()
        .into_owned();
    let args = ["config", "--db", &database];
    let environment = BTreeMap::new();
    let configured = invoke_json(
        &args,
        &json!({"action":"upsert_source", "source": {
            "key":"feed", "label":"Recorded label", "kind":"rss", "url":"https://example.test/feed",
            "section":"technology", "threshold":"always", "enabled":true,
            "priority_rank":i64::MIN, "dedup_group":"preserve",
            "outlet_extraction":"title_suffix"
        }}),
        &environment,
    )?;
    assert_eq!(
        configured.pointer("/source_reporting/feed"),
        Some(&json!("required"))
    );
    let source = configured.pointer("/sources/0").context("source")?.clone();
    let db = rusqlite::Connection::open(&database)?;
    db.execute_batch("INSERT INTO source_state VALUES ('feed', 'identity', 'feed-identity', 'Seen title', 'https://example.test/seen', '2026-01-01', '2026-01-01T00:00:00Z'); INSERT INTO brief_run (id, started_at, dry_run, status, summary) VALUES ('old', '2026-01-01T00:00:00Z', 0, 'ok', 'recorded'); INSERT INTO brief_run_item (run_id,category,source_key,source_label,kind,section,threshold,priority_rank,published_at,outlet,title,url,reason,detail) VALUES ('old','must_include','feed','Recorded label','rss','technology','always',-9223372036854775808,'','','Old story','https://example.test/story','','');")?;
    invoke_json(
        &args,
        &json!({"action":"set_brief_options", "max_delivery_items":8}),
        &environment,
    )?;
    let inspected = invoke_json(&args, &json!({"action":"inspect_config"}), &environment)?;
    assert_eq!(
        inspected.pointer("/sources/0"),
        Some(&source),
        "unrelated options must not rewrite source fields"
    );
    let mut renamed = source;
    renamed.as_object_mut().context("source object")?.extend([
        ("label".to_owned(), json!("Current label")),
        ("threshold".to_owned(), json!("high")),
        ("priority_rank".to_owned(), json!(i64::MAX)),
    ]);
    let updated = invoke_json(
        &args,
        &json!({"action":"upsert_source", "source":renamed}),
        &environment,
    )?;
    assert_eq!(
        updated.pointer("/source_reporting/feed"),
        Some(&json!("major"))
    );
    let cursor: (String, String) = db.query_row(
        "SELECT latest_identity, latest_feed_identity FROM source_state WHERE source_key = 'feed'",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    assert_eq!(cursor, ("identity".to_owned(), "feed-identity".to_owned()));
    let detail = invoke_cli_json(
        &["runs", "show", "old", "--json", "--db", &database],
        &environment,
    )?;
    assert_eq!(
        detail.pointer("/must_include/0/reporting"),
        Some(&json!("required"))
    );
    assert_eq!(
        detail.pointer("/must_include/0/source_label"),
        Some(&json!("Recorded label"))
    );
    assert_eq!(
        detail.pointer("/must_include/0/priority_rank"),
        Some(&json!(i64::MIN))
    );
    assert_eq!(
        detail.pointer("/must_include/0/delivery_status"),
        Some(&json!("not_delivered"))
    );
    let inspected = invoke_json(&args, &json!({"action":"inspect_config"}), &environment)?;
    let rejected = invoke_json(
        &args,
        &json!({"action":"replace_outlet_policies", "outlets":[
            {"name":"First", "aliases":["Example.com"], "policy":"allow", "enabled":true},
            {"name":"Second", "aliases":["example.news"], "policy":"block", "enabled":true}
        ]}),
        &environment,
    )?;
    assert_eq!(rejected.get("rejected"), Some(&json!(true)));
    let preserved = invoke_json(&args, &json!({"action":"inspect_config"}), &environment)?;
    assert_eq!(preserved.get("outlets"), inspected.get("outlets"));
    Ok(())
}

#[test]
fn required_items_leave_the_shared_news_and_forum_allowance_intact() -> Result<()> {
    let temp = TempDir::new()?;
    let database = temp
        .path()
        .join("sections.sqlite")
        .to_string_lossy()
        .into_owned();
    let environment = BTreeMap::from([("SIFTWIRE_EVAL_ALLOW_FILE_URLS", "1")]);
    for (key, section, threshold) in [
        ("blog", "blogs", "always"),
        ("news", "technology", "medium"),
        ("forum", "forums", "medium"),
    ] {
        let feed = temp.path().join(format!("{key}.xml"));
        fs::write(
            &feed,
            format!(
                "<rss version=\"2.0\"><channel><title>Fixture</title><item><title>{key} update</title><link>https://example.test/{key}</link><guid>{key}</guid><pubDate>{}</pubDate></item></channel></rss>",
                chrono::Utc::now().to_rfc2822()
            ),
        )?;
        let url = url::Url::from_file_path(&feed).map_err(|()| anyhow::anyhow!("fixture URL"))?;
        invoke_json(
            &["config", "--db", &database],
            &json!({"action":"upsert_source", "source":{
                "key":key, "label":key, "kind":"rss", "url":url, "section":section,
                "threshold":threshold, "enabled":true
            }}),
            &environment,
        )?;
    }
    invoke_json(
        &["config", "--db", &database],
        &json!({"action":"set_brief_options", "max_delivery_items":2}),
        &environment,
    )?;
    let run: RunResult = decode(invoke_json(
        &["brief", "--db", &database],
        &json!({"action":"run_brief"}),
        &environment,
    )?)?;
    assert_eq!(run.must_include.len(), 1);
    assert_eq!(
        run.candidate_slots, 2,
        "required blog consumed optional capacity"
    );
    assert_eq!(run.candidates.len(), 2);
    invoke_json(
        &["config", "--db", &database],
        &json!({"action":"set_brief_options", "max_delivery_items":1}),
        &environment,
    )?;
    let prepared: DeliveryResult = decode(invoke_json(
        &["brief", "--db", &database],
        &json!({"action":"prepare_delivery", "run_id":run.run_id, "candidate_indexes":[0,1]}),
        &environment,
    )?)?;
    assert!(
        !prepared.rejected,
        "preparation lost the recorded optional allowance"
    );
    assert_eq!(
        prepared.message,
        "## Blogs\n\n- [blog update](<https://example.test/blog>)\n\n## News\n\n- [news update](<https://example.test/news>)\n\n## Forums\n\n- [forum update](<https://example.test/forum>)"
    );
    assert!(
        prepared
            .html
            .contains("1 blog post, 1 news story, 1 forum discussion.")
    );
    Ok(())
}

fn configure_annotation_sources(
    temp: &TempDir,
    database: &str,
    environment: &BTreeMap<&str, &str>,
) -> Result<()> {
    let feed = temp.path().join("feed.xml");
    fs::write(
        &feed,
        format!(
            r#"<rss version="2.0"><channel><title>Fixture</title><item><title>One - Allowed</title><link>file:///nonexistent/siftwire-one</link><guid>one</guid><pubDate>{published}</pubDate></item><item><title>Two - Watched</title><link>file:///nonexistent/siftwire-two</link><guid>two</guid><pubDate>{published}</pubDate></item><item><title>Three - Blocked</title><link>file:///nonexistent/siftwire-three</link><guid>three</guid><pubDate>{published}</pubDate></item></channel></rss>"#,
            published = chrono::Utc::now().to_rfc2822()
        ),
    )?;
    let feed_url = url::Url::from_file_path(&feed).map_err(|()| anyhow::anyhow!("fixture URL"))?;
    let args = ["config", "--db", database];
    invoke_json(
        &args,
        &json!({"action":"upsert_source", "source": {
            "key":"retained", "label":"Recorded retained", "kind":"rss", "url":feed_url,
            "section":"technology", "threshold":"medium", "enabled":true,
            "outlet_extraction":"title_suffix"
        }}),
        environment,
    )?;
    invoke_json(
        &args,
        &json!({"action":"replace_outlet_policies", "outlets":[
            {"name":"Allowed","policy":"allow","enabled":true},
            {"name":"Watched","policy":"watch","enabled":true},
            {"name":"Blocked","policy":"block","enabled":true}
        ]}),
        environment,
    )?;
    Ok(())
}

#[test]
fn run_annotations_distinguish_retained_and_dropped_with_recorded_labels() -> Result<()> {
    let temp = TempDir::new()?;
    let database = temp
        .path()
        .join("annotations.sqlite")
        .to_string_lossy()
        .into_owned();
    let environment = BTreeMap::from([("SIFTWIRE_EVAL_ALLOW_FILE_URLS", "1")]);
    configure_annotation_sources(&temp, &database, &environment)?;
    let run = invoke_json(
        &["brief", "--db", &database],
        &json!({"action":"run_brief","dry_run":false}),
        &environment,
    )?;
    let run_id = run
        .get("run_id")
        .and_then(serde_json::Value::as_str)
        .context("run ID")?;
    let detail = invoke_cli_json(
        &["runs", "show", run_id, "--json", "--db", &database],
        &environment,
    )?;
    let annotations = detail
        .get("annotations")
        .and_then(serde_json::Value::as_array)
        .context("annotations")?;
    assert_eq!(
        annotations.len(),
        2,
        "Allow and Watch retain their publisher annotations"
    );
    for annotation in annotations {
        assert_eq!(
            annotation.get("source_label"),
            Some(&json!("Recorded retained"))
        );
        assert_eq!(annotation.get("disposition"), Some(&json!("retained")));
    }
    for policy in ["allow", "watch"] {
        assert!(
            annotations
                .iter()
                .any(|item| item.pointer("/detail/policy") == Some(&json!(policy))),
            "missing {policy} annotation"
        );
    }
    assert_eq!(
        detail
            .get("candidates")
            .and_then(serde_json::Value::as_array)
            .context("candidates")?
            .len(),
        2,
        "current news offers both policy-retained items"
    );
    let drops = detail
        .get("dropped")
        .and_then(serde_json::Value::as_array)
        .context("drops")?;
    assert_eq!(drops.len(), 1, "Block excludes its matched item");
    let db = rusqlite::Connection::open(&database)?;
    db.execute_batch("UPDATE brief_source SET label = 'Current label';")?;
    let reread = invoke_cli_json(
        &["runs", "show", run_id, "--json", "--db", &database],
        &environment,
    )?;
    assert_eq!(reread.get("dropped"), detail.get("dropped"));
    assert_eq!(reread.get("annotations"), detail.get("annotations"));
    assert_eq!(
        reread.get("fetch"),
        detail.get("fetch"),
        "fetch labels must not use current config"
    );
    assert_eq!(
        reread.pointer("/fetch/0/source_label"),
        Some(&json!("Recorded retained"))
    );
    Ok(())
}

#[test]
fn archive_cli_pages_past_default_chunk_and_keeps_search_literal() -> Result<()> {
    let temp = TempDir::new()?;
    let database = temp
        .path()
        .join("pages.sqlite")
        .to_string_lossy()
        .into_owned();
    invoke_json(
        &["config", "--db", &database],
        &json!({"action":"init"}),
        &BTreeMap::new(),
    )?;
    let db = rusqlite::Connection::open(&database)?;
    // One more than the documented default chunk proves that it is not a cap.
    for index in 0..21 {
        db.execute("INSERT INTO brief_run (id,started_at,dry_run,status,summary) VALUES (?1,'2026-01-01T00:00:00Z',0,'ok','--db')", [format!("run-{index:02}")])?;
    }
    let first = invoke_cli_json(
        &["runs", "list", "--json", "--db", &database],
        &BTreeMap::new(),
    )?;
    assert_eq!(
        first
            .get("runs")
            .and_then(serde_json::Value::as_array)
            .context("runs")?
            .len(),
        20
    );
    let before = first
        .get("next_before")
        .and_then(serde_json::Value::as_str)
        .context("cursor")?;
    let second = invoke_cli_json(
        &[
            "runs", "list", "--json", "--before", before, "--search", "--db", "--db", &database,
        ],
        &BTreeMap::new(),
    )?;
    assert_eq!(second.pointer("/runs/0/run_id"), Some(&json!("run-00")));
    assert_eq!(second.get("next_before"), Some(&serde_json::Value::Null));
    let delivered = invoke_cli_json(
        &["runs", "list", "--json", "--delivered", "--db", &database],
        &BTreeMap::new(),
    )?;
    assert_eq!(delivered.get("runs"), Some(&json!([])));
    let unknown = invoke(
        &["runs", "show", "unknown", "--json", "--db", &database],
        None,
        &BTreeMap::new(),
    )?;
    assert_eq!(unknown.status.code(), Some(1));
    assert!(unknown.stdout.is_empty());
    assert!(text(&unknown.stderr).contains("run not found"));
    Ok(())
}
