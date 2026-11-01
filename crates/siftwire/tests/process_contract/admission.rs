use std::fmt::Write as _;

use super::*;

#[test]
fn polluted_sports_feed_excludes_promotions_but_preserves_reports_and_scores() -> Result<()> {
    let temp = TempDir::new()?;
    let database = temp
        .path()
        .join("admission.sqlite")
        .to_string_lossy()
        .into_owned();
    let environment = BTreeMap::from([("SIFTWIRE_EVAL_ALLOW_FILE_URLS", "1")]);
    let feed = polluted_feed(&temp)?;
    let configured = invoke_json(
        &["config", "--db", &database],
        &json!({"action":"replace_sources","sources":[
            {"key":"team_news","label":"Team news","kind":"rss","url":feed,
             "section":"sports","threshold":"high","enabled":true},
            {"key":"team_schedule","label":"Team schedule","kind":"sports_schedule",
             "url":write_sports_schedule(&temp)?,"section":"sports","threshold":"medium",
             "schedule_format":"espn","enabled":true}
        ]}),
        &environment,
    )?;
    assert_eq!(
        configured.get("rejected"),
        Some(&json!(false)),
        "fixture configuration rejected"
    );
    invoke_json(
        &["config", "--db", &database],
        &json!({"action":"set_brief_options","sports_timezone":"America/Chicago"}),
        &environment,
    )?;
    let run = invoke_json(
        &["brief", "--db", &database],
        &json!({"action":"run_brief","dry_run":false}),
        &environment,
    )?;
    let candidates = run
        .get("candidates")
        .and_then(serde_json::Value::as_array)
        .context("candidates")?;
    assert_eq!(
        candidates.len(),
        6,
        "spam entered candidates or genuine reporting was lost"
    );
    assert!(
        candidates
            .iter()
            .any(|item| item["title"] == "Team Alpha 2-0 Team Gamma - Match report"),
        "legitimate match reporting was discarded by admission"
    );
    assert!(
        candidates
            .iter()
            .any(|item| item["title"] == "League sues illegal livestream operators - Wire"),
        "reporting about streams was mistaken for a stream offer"
    );
    for title in [
        "Watch: Team Alpha win first championship - Wire",
        "“I resign”: Team Alpha head coach quits live on air - Wire",
    ] {
        assert!(
            candidates.iter().any(|item| item["title"] == title),
            "major reporting wrongly excluded: {title}"
        );
    }
    let dropped = run
        .get("suppressed")
        .and_then(serde_json::Value::as_array)
        .context("suppressed")?;
    assert_eq!(dropped.len(), 10, "polluted feed was not fully audited");
    assert_eq!(
        dropped
            .iter()
            .filter(|item| item["reason"] == "promotional_livestream")
            .count(),
        9,
        "promotional headlines were not dropped"
    );
    assert_eq!(
        dropped
            .iter()
            .filter(|item| item["reason"] == "embedded_loader_url")
            .count(),
        1,
        "disguised loader URL was not dropped"
    );
    assert_eq!(
        run.get("sports_updates")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(2),
        "fixtures/results were lost"
    );
    assert_eq!(
        run.get("candidate_slots"),
        Some(&json!(7)),
        "sports consumed editorial capacity"
    );
    assert_polluted_delivery(&database, &run, &environment)?;
    Ok(())
}

fn assert_polluted_delivery(
    database: &str,
    run: &serde_json::Value,
    environment: &BTreeMap<&str, &str>,
) -> Result<()> {
    let candidates = run
        .get("candidates")
        .and_then(serde_json::Value::as_array)
        .context("candidates")?;
    let major = candidates
        .iter()
        .position(|item| item["title"] == "Team Alpha head coach dismissed - Wire")
        .context("major development")?;
    let refused = invoke_json(
        &["brief", "--db", database],
        &json!({"action":"prepare_delivery","run_id":run["run_id"],"candidate_indexes":[6]}),
        environment,
    )?;
    assert_eq!(
        refused.get("rejected"),
        Some(&json!(true)),
        "dropped feed position remained selectable"
    );
    let prepared: DeliveryResult = decode(invoke_json(
        &["brief", "--db", database],
        &json!({"action":"prepare_delivery","run_id":run["run_id"],"candidate_indexes":[major]}),
        environment,
    )?)?;
    assert!(!prepared.rejected, "clean selection rejected");
    assert!(
        prepared.message.contains("Team Alpha head coach dismissed"),
        "major news lost"
    );
    assert!(
        prepared.message.contains("Team Gamma 0 - 2 Team Alpha"),
        "final score lost"
    );
    assert!(
        prepared.message.contains("Team Beta vs Team Alpha"),
        "upcoming fixture lost"
    );
    assert!(
        prepared.html.contains("Times: America/Chicago"),
        "configured time zone lost"
    );
    assert!(
        !prepared.html.contains("OPAQUE-SPAM") && !prepared.html.contains("data:video"),
        "spam reached the email preview"
    );
    let archive = invoke_cli_json(
        &[
            "runs",
            "show",
            run["run_id"].as_str().context("run ID")?,
            "--db",
            database,
            "--json",
        ],
        environment,
    )?;
    let rows = archive
        .get("dropped")
        .and_then(serde_json::Value::as_array)
        .context("archive dropped")?;
    assert_eq!(
        rows.iter()
            .filter(|item| item["reason"] == "promotional_livestream")
            .count(),
        9,
        "archive dropped evidence lost"
    );
    Ok(())
}

fn polluted_feed(temp: &TempDir) -> Result<url::Url> {
    let path = temp.path().join("polluted.xml");
    let published = chrono::Utc::now().to_rfc2822();
    let mut body = String::from("<rss version=\"2.0\"><channel><title>Team news</title>");
    let promotions = [
        "^~[EN~DIRECT] Team Alpha - Team Beta en direct gratuity",
        "【LIVESTREAMS】LiVe]Team Alpha vs Team Beta Live Free Right Now",
        "[LIVESTREAMs!TV] Team Alpha 𝐋𝐈𝐕𝐄 Ｆｒｅｅ 𝐒𝐭𝐑𝐞𝐀𝐦",
        "!(OFFICIAL-TV) Team Alpha vs Team Beta Live Soccer Match",
        "!+【【4K】】 Team Alpha Team Beta Live Broadcast",
        ")+ω#*[WATCHLIvE]HQ*!] Team Alpha v Team Beta in HS Football",
        "[[[ABC^Sports]] Team Alpha v Team Beta LIVE.",
        "!$>[LIVE!#NOW]* Team Alpha v Team Beta Live Broadcast",
    ];
    for (index, title) in promotions.into_iter().cycle().take(9).enumerate() {
        write!(
            body,
            "<item><title>{title} - Campus Publisher</title><link>https://news.google.com/rss/articles/OPAQUE-SPAM-{index}</link><guid>spam-{index}</guid><pubDate>{published}</pubDate></item>"
        )?;
    }
    for (index, title) in [
        "Team Alpha 2-0 Team Gamma - Match report",
        "Team Alpha head coach dismissed - Wire",
        "League sues illegal livestream operators - Wire",
        "Broadcaster wins live match rights - Wire",
        "Watch: Team Alpha win first championship - Wire",
        "“I resign”: Team Alpha head coach quits live on air - Wire",
    ]
    .into_iter()
    .enumerate()
    {
        write!(
            body,
            "<item><title>{title}</title><link>https://publisher.test/story-{index}</link><guid>story-{index}</guid><pubDate>{published}</pubDate></item>"
        )?;
    }
    write!(
        body,
        "<item><title>Team Alpha announcement</title><link>https://campus.test/tour?xml=data:video/mp4;base64,FAKE</link><guid>loader</guid><pubDate>{published}</pubDate></item></channel></rss>"
    )?;
    fs::write(&path, body)?;
    url::Url::from_file_path(&path).map_err(|()| anyhow::anyhow!("fixture URL"))
}
