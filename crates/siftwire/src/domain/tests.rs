#![expect(
    clippy::panic_in_result_fn,
    reason = "behavior tests return Result for setup while assertions report contract failures"
)]

use anyhow::{Result, bail};

use super::{
    OUTLET_EXTRACTION_NONE, OUTLET_POLICY_ALLOW, OutletPolicy, SCHEDULE_FILTER_ALL,
    SCHEDULE_FILTER_STANDINGS_TOP_TWO, SCHEDULE_FORMAT_RIOT, SOURCE_KIND_RSS, SOURCE_KIND_SCHEDULE,
    Source, THRESHOLD_MEDIUM, URL_CANONICALIZATION_NONE, normalize_outlet_policies,
    normalize_source, normalize_sources,
};

fn valid_source(url: &str) -> Source {
    Source {
        key: " Example.Feed ".to_owned(),
        label: " Example Feed ".to_owned(),
        kind: " RSS ".to_owned(),
        url: url.to_owned(),
        section: " Technology ".to_owned(),
        enabled: true,
        ..Source::default()
    }
}

#[test]
fn source_and_outlet_normalization_preserve_the_durable_contract() -> Result<()> {
    let source = normalize_source(valid_source(" https://example.com/feed.xml "))?;
    assert_eq!(source.key, "example.feed", "source key was not normalized");
    assert_eq!(source.label, "Example Feed", "source label was not trimmed");
    assert_eq!(
        source.kind, SOURCE_KIND_RSS,
        "source kind was not normalized"
    );
    assert_eq!(source.section, "technology", "section was not normalized");
    assert_eq!(
        source.threshold, THRESHOLD_MEDIUM,
        "threshold default changed"
    );
    assert_eq!(
        source.url_canonicalization, URL_CANONICALIZATION_NONE,
        "canonicalization default changed"
    );
    assert_eq!(
        source.outlet_extraction, OUTLET_EXTRACTION_NONE,
        "outlet extraction default changed"
    );
    assert_eq!(
        source.schedule_filter, SCHEDULE_FILTER_ALL,
        "schedule filter default changed"
    );

    let policies = normalize_outlet_policies(vec![OutletPolicy {
        name: " Example Outlet ".to_owned(),
        aliases: vec!["Example".to_owned(), " example ".to_owned(), String::new()],
        enabled: true,
        ..OutletPolicy::default()
    }])?;
    let Some(policy) = policies.first() else {
        bail!("normalized outlet policy was missing");
    };
    assert_eq!(policy.name, "Example Outlet", "policy name was not trimmed");
    assert_eq!(policy.policy, OUTLET_POLICY_ALLOW, "policy default changed");
    assert_eq!(policy.aliases, ["Example"], "aliases were not compacted");

    let duplicate = normalize_sources(vec![
        valid_source("https://example.com/one.xml"),
        Source {
            key: "example.feed".to_owned(),
            label: "Other".to_owned(),
            kind: "rss".to_owned(),
            url: "https://example.com/two.xml".to_owned(),
            section: "technology".to_owned(),
            ..Source::default()
        },
    ]);
    let Err(error) = duplicate else {
        bail!("duplicate normalized source key was accepted");
    };
    assert!(
        error
            .to_string()
            .contains("duplicate source key \"example.feed\""),
        "duplicate key diagnostic changed: {error}"
    );
    Ok(())
}

#[test]
fn standings_filter_is_valid_only_for_riot_sources() -> Result<()> {
    let riot = normalize_source(Source {
        key: "lol_example".to_owned(),
        label: "Example League".to_owned(),
        kind: SOURCE_KIND_SCHEDULE.to_owned(),
        url: "https://esports.example/persisted/gw/getSchedule?hl=en-US&leagueId=42".to_owned(),
        section: "sports".to_owned(),
        enabled: true,
        schedule_format: SCHEDULE_FORMAT_RIOT.to_owned(),
        schedule_filter: SCHEDULE_FILTER_STANDINGS_TOP_TWO.to_owned(),
        api_key: "public-key".to_owned(),
        ..Source::default()
    })?;

    let Err(error) = normalize_source(Source {
        url: "https://esports.example/schedule?leagueId=42".to_owned(),
        ..riot.clone()
    }) else {
        bail!("Riot standings filter accepted a non-Gateway URL");
    };
    assert!(
        error.to_string().contains("persisted Gateway"),
        "Riot Gateway diagnostic changed: {error}"
    );

    let Err(error) = normalize_source(Source {
        schedule_format: super::SCHEDULE_FORMAT_ESPN.to_owned(),
        api_key: String::new(),
        ..riot
    }) else {
        bail!("ESPN source accepted the Riot standings filter");
    };
    assert!(
        error.to_string().contains("applies only to the riot"),
        "schedule filter diagnostic changed: {error}"
    );
    Ok(())
}

#[test]
fn fetch_urls_reject_every_credential_shape() -> Result<()> {
    for url in [
        "https://user@example.com/feed.xml",
        "https://user:secret@example.com/feed.xml",
        "https://@example.com/feed.xml",
        "https://user%40name@example.com/feed.xml",
    ] {
        let result = normalize_source(valid_source(url));
        let Err(error) = result else {
            bail!("credential-bearing URL was accepted: {url}");
        };
        assert!(
            error.to_string().contains("must not include credentials"),
            "credential diagnostic changed for {url}: {error}"
        );
    }

    let source = normalize_source(valid_source(
        "https://example.com/feed.xml?author=user@example.net",
    ))?;
    assert_eq!(
        source.url, "https://example.com/feed.xml?author=user@example.net",
        "an at-sign outside the authority was treated as credentials"
    );
    Ok(())
}
