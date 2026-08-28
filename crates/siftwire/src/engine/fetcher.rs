use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::domain::{SOURCE_KIND_SCHEDULE, Source};

use super::canonical::process_feed_items;
use super::feed::parse_feed;
use super::google::GoogleResolver;
use super::http::HttpClient;
use super::model::{FetchOutput, FetchedItem};
use super::schedule::SportsOptions;

pub struct Fetcher {
    client: HttpClient,
    google: GoogleResolver,
}

#[derive(Deserialize)]
struct GitHubRelease {
    tag_name: String,
    #[serde(deserialize_with = "crate::serde_util::null_default")]
    name: String,
    html_url: String,
    published_at: String,
    draft: bool,
    prerelease: bool,
}

impl Fetcher {
    #[must_use]
    pub fn new() -> Self {
        let client = HttpClient::new();
        Self {
            google: GoogleResolver::new(client.clone()),
            client,
        }
    }

    /// Fetches and processes one source.
    ///
    /// # Errors
    ///
    /// Returns an error when the source kind is unsupported or its response cannot be fetched or
    /// parsed. Per-item canonicalization failures stay in [`FetchOutput::unresolved`].
    pub fn fetch(
        &self,
        source: &Source,
        now: chrono::DateTime<chrono::Utc>,
        sports_options: &SportsOptions,
    ) -> Result<FetchOutput> {
        match source.kind.as_str() {
            "rss" | "atom" => self.fetch_feed(source),
            "github_release" => self.fetch_github_releases(source),
            SOURCE_KIND_SCHEDULE => {
                super::schedule::fetch_schedule(&self.client, source, now, sports_options)
            }
            unsupported => bail!("unsupported source kind {unsupported:?}"),
        }
    }

    fn fetch_feed(&self, source: &Source) -> Result<FetchOutput> {
        let body = self.client.get(&source.url)?;
        let parsed = parse_feed(&body)?;
        let (items, unresolved, truncated) =
            process_feed_items(&self.client, &self.google, source, parsed)?;
        Ok(FetchOutput {
            items,
            unresolved,
            truncated,
            ..FetchOutput::default()
        })
    }

    fn fetch_github_releases(&self, source: &Source) -> Result<FetchOutput> {
        let endpoint = if source.url.trim().is_empty() {
            format!(
                "https://api.github.com/repos/{}/releases?per_page=10",
                source.repo
            )
        } else {
            source.url.trim().to_owned()
        };
        let body = self.client.get(&endpoint)?;
        let releases: Vec<GitHubRelease> =
            serde_json::from_slice(&body).context("parse GitHub releases JSON")?;
        Ok(FetchOutput {
            items: releases
                .into_iter()
                .filter_map(|release| release_item(source, release))
                .collect(),
            ..FetchOutput::default()
        })
    }
}

impl Default for Fetcher {
    fn default() -> Self {
        Self::new()
    }
}

fn release_item(source: &Source, release: GitHubRelease) -> Option<FetchedItem> {
    let tag = release.tag_name.trim();
    if release.draft || release.prerelease || tag.is_empty() {
        return None;
    }
    let release_name = if release.name.trim().is_empty() {
        tag
    } else {
        release.name.trim()
    };
    let title = if source.repo.is_empty() {
        release_name.to_owned()
    } else {
        format!("{} {release_name}", source.repo)
    };
    let url = if release.html_url.is_empty() && !source.repo.is_empty() {
        format!("https://github.com/{}/releases/tag/{tag}", source.repo)
    } else {
        release.html_url
    };
    Some(FetchedItem {
        title,
        url,
        published_at: release.published_at,
        identity: tag.to_owned(),
        feed_identity: String::new(),
        outlet: String::new(),
        rss_source: String::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::{GitHubRelease, release_item};
    use crate::domain::Source;

    #[test]
    fn null_github_release_name_falls_back_to_tag() {
        let releases = serde_json::from_str::<Vec<GitHubRelease>>(
            r#"[{"tag_name":"v1.2.3","name":null,"html_url":"https://example.test/release","published_at":"2026-04-23T00:00:00Z","draft":false,"prerelease":false}]"#,
        );
        assert!(
            releases.is_ok(),
            "GitHub's nullable release name failed to decode"
        );
        let items = releases
            .into_iter()
            .flatten()
            .filter_map(|release| {
                release_item(
                    &Source {
                        repo: "owner/project".to_owned(),
                        ..Source::default()
                    },
                    release,
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            items.first().map(|item| item.title.as_str()),
            Some("owner/project v1.2.3"),
            "null release name did not use the tag fallback"
        );
    }
}
