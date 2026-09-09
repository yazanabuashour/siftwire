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
            "rss" => self.fetch_feed(source),
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
        let (items, unresolved, truncated) = process_feed_items(&self.google, source, parsed)?;
        Ok(FetchOutput {
            items,
            unresolved,
            truncated,
            ..FetchOutput::default()
        })
    }

    fn fetch_github_releases(&self, source: &Source) -> Result<FetchOutput> {
        let endpoint = format!(
            "https://api.github.com/repos/{}/releases?per_page=10",
            source.repo
        );
        let body = self.client.get(&endpoint)?;
        Ok(FetchOutput {
            items: parse_releases(source, &body)?,
            ..FetchOutput::default()
        })
    }
}

impl Default for Fetcher {
    fn default() -> Self {
        Self::new()
    }
}

fn parse_releases(source: &Source, body: &[u8]) -> Result<Vec<FetchedItem>> {
    let releases: Vec<GitHubRelease> =
        serde_json::from_slice(body).context("parse GitHub releases JSON")?;
    Ok(releases
        .into_iter()
        .filter_map(|release| release_item(source, release))
        .collect())
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
    })
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::panic_in_result_fn,
        reason = "release behavior tests return Result for fixture decoding"
    )]

    use super::parse_releases;
    use crate::domain::Source;
    use anyhow::Result;

    #[test]
    fn releases_filter_unpublished_tags_and_preserve_name_and_url_fallbacks() -> Result<()> {
        let items = parse_releases(
            &Source {
                repo: "owner/project".to_owned(),
                ..Source::default()
            },
            br#"[
                {"tag_name":"v3","name":" Release three ","html_url":"https://github.com/owner/project/releases/tag/v3","published_at":"2026-04-23T00:00:00Z","draft":false,"prerelease":false},
                {"tag_name":"v2","name":null,"html_url":"","published_at":"2026-04-22T00:00:00Z","draft":false,"prerelease":false},
                {"tag_name":" v1 ","name":" ","html_url":"","published_at":"2026-04-21T00:00:00Z","draft":false,"prerelease":false},
                {"tag_name":"draft","name":"Draft","html_url":"","published_at":"","draft":true,"prerelease":false},
                {"tag_name":"preview","name":"Preview","html_url":"","published_at":"","draft":false,"prerelease":true},
                {"tag_name":" ","name":"Missing tag","html_url":"","published_at":"","draft":false,"prerelease":false}
            ]"#,
        )?;
        assert_eq!(
            items
                .iter()
                .map(|item| (
                    item.title.as_str(),
                    item.identity.as_str(),
                    item.url.as_str(),
                    item.published_at.as_str()
                ))
                .collect::<Vec<_>>(),
            [
                (
                    "owner/project Release three",
                    "v3",
                    "https://github.com/owner/project/releases/tag/v3",
                    "2026-04-23T00:00:00Z"
                ),
                (
                    "owner/project v2",
                    "v2",
                    "https://github.com/owner/project/releases/tag/v2",
                    "2026-04-22T00:00:00Z"
                ),
                (
                    "owner/project v1",
                    "v1",
                    "https://github.com/owner/project/releases/tag/v1",
                    "2026-04-21T00:00:00Z"
                ),
            ],
            "release filtering, identity, or fallbacks changed"
        );
        assert!(
            parse_releases(&Source::default(), b"not JSON").is_err(),
            "malformed release response was accepted"
        );
        Ok(())
    }
}
