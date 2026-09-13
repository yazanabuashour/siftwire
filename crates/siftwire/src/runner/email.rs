use anyhow::{Context, Result};
use chrono::{DateTime, Timelike, Utc};
use chrono_tz::Tz;
use email_ui::{Document, Header, Image, Notice, Row, ScheduleGroup, ScheduleRow, Section};

use crate::contract::SportsUpdate;
use crate::domain::SOURCE_KIND_GITHUB_RELEASE;
use crate::storage::{RunDeliveryContext, RunItemRow};

pub fn render(
    selected: &[&RunItemRow],
    context: &RunDeliveryContext,
    started_at: &str,
    timezone: Tz,
) -> Result<String> {
    let document = document(selected, context, started_at, timezone)?;
    Ok(email_ui::render(&document)?.html)
}

fn document(
    selected: &[&RunItemRow],
    context: &RunDeliveryContext,
    started_at: &str,
    timezone: Tz,
) -> Result<Document> {
    let started_at = DateTime::parse_from_rfc3339(started_at)
        .context("stored brief start time is invalid")?
        .with_timezone(&Utc)
        .with_timezone(&timezone);
    let edition = if started_at.hour() < 12 {
        "Morning"
    } else {
        "Evening"
    };
    let mut releases = Vec::new();
    let mut news = Vec::new();
    for item in selected {
        if item.kind == SOURCE_KIND_GITHUB_RELEASE {
            releases.push(Row {
                title: item.title.clone(),
                url: item.url.clone(),
                eyebrow: item.source_label.clone(),
            });
        } else {
            news.push(news_row(item));
        }
    }
    let count_label = |count: usize, singular: &str, plural: &str| {
        format!("{count} {}", if count == 1 { singular } else { plural })
    };
    let preheader = format!(
        "{}, {}, {}.",
        count_label(news.len(), "story", "stories"),
        count_label(releases.len(), "release", "releases"),
        count_label(
            context.sports_updates.len(),
            "sports update",
            "sports updates"
        )
    );
    let mut sections = Vec::new();
    if !releases.is_empty() {
        sections.push(Section::Cards {
            heading: "New versions".to_owned(),
            rows: releases,
        });
    }
    if !news.is_empty() {
        sections.push(Section::Stories {
            heading: "News".to_owned(),
            rows: news,
        });
    }
    if !context.sports_updates.is_empty() {
        let groups = [("Upcoming", "upcoming"), ("Results", "final")]
            .into_iter()
            .filter_map(|(heading, status)| {
                let rows = context
                    .sports_updates
                    .iter()
                    .filter(|update| update.status == status)
                    .map(|update| schedule_row(update, timezone))
                    .collect::<Vec<_>>();
                (!rows.is_empty()).then(|| ScheduleGroup {
                    heading: heading.to_owned(),
                    rows,
                })
            })
            .collect();
        sections.push(Section::Schedule {
            heading: "Sports".to_owned(),
            groups,
        });
    }
    Ok(Document {
        title: format!("SiftWire {edition} brief"),
        heading: "SiftWire".to_owned(),
        preheader,
        header: Header {
            edition: edition.to_owned(),
            date: started_at.format("%a, %b %-d").to_string(),
        },
        footer: format!("Times: {timezone}"),
        sections,
        notice: (!context.health_footnote.is_empty()).then(|| Notice {
            label: "Source health".to_owned(),
            text: context.health_footnote.clone(),
        }),
    })
}

fn news_row(item: &RunItemRow) -> Row {
    let section = if item.section.is_empty() {
        item.source_label.clone()
    } else {
        item.section.replace(['_', '-'], " ")
    };
    let outlet = if item.outlet.is_empty() {
        item.source_label.as_str()
    } else {
        item.outlet.as_str()
    };
    Row {
        title: item.title.clone(),
        url: item.url.clone(),
        eyebrow: format!("{section} · {outlet}"),
    }
}

fn schedule_row(update: &SportsUpdate, timezone: Tz) -> ScheduleRow {
    let when = update.starts_at.with_timezone(&timezone);
    ScheduleRow {
        weekday: when.format("%a").to_string(),
        day: when.format("%-d").to_string(),
        month: when.format("%b").to_string(),
        title: update.title.clone(),
        url: update.url.clone(),
        metadata: format!("{} · {}", update.competition, when.format("%-I:%M %p %Z")),
        images: update
            .images
            .iter()
            .map(|image| Image {
                url: image.url.clone(),
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::panic_in_result_fn,
        reason = "behavior assertions report the rendered email contract"
    )]

    use super::render;
    use crate::contract::{SportsImage, SportsUpdate};
    use crate::storage::{RunDeliveryContext, RunItemRow};

    #[test]
    fn folio_brief_preserves_content_and_readable_metadata() -> anyhow::Result<()> {
        let item = RunItemRow {
            title: "Release <one>".to_owned(),
            url: "https://example.test/?a=1&b=2".to_owned(),
            source_label: "Example".to_owned(),
            kind: crate::domain::SOURCE_KIND_GITHUB_RELEASE.to_owned(),
            ..RunItemRow::default()
        };
        let news = RunItemRow {
            title: "News <one>".to_owned(),
            source_label: "Example & Co".to_owned(),
            ..RunItemRow::default()
        };
        let context = RunDeliveryContext {
            max_delivery_items: 7,
            sports_timezone: "America/Chicago".to_owned(),
            sports_updates: vec![
                SportsUpdate {
                    status: "upcoming".to_owned(),
                    title: "Alpha vs Beta".to_owned(),
                    competition: "Example League".to_owned(),
                    url: "https://example.test/game".to_owned(),
                    starts_at: chrono::DateTime::parse_from_rfc3339("2026-08-29T12:00:00Z")?
                        .with_timezone(&chrono::Utc),
                    images: vec![SportsImage {
                        url: "https://images.example/alpha.png".to_owned(),
                        alt: "Alpha".to_owned(),
                    }],
                    ..SportsUpdate::default()
                },
                SportsUpdate {
                    status: "final".to_owned(),
                    title: "Gamma 2–1 Delta".to_owned(),
                    competition: "Example Cup".to_owned(),
                    starts_at: chrono::DateTime::parse_from_rfc3339("2026-08-28T00:00:00Z")?
                        .with_timezone(&chrono::Utc),
                    ..SportsUpdate::default()
                },
            ],
            health_footnote: "Source <one> & source two unavailable".to_owned(),
        };
        let evening = render(
            &[&item, &news],
            &context,
            "2026-08-28T00:00:00Z",
            chrono_tz::America::Chicago,
        )?;
        assert_eq!(
            evening,
            include_str!("../../tests/fixtures/email/rich-evening.html")
        );
        Ok(())
    }

    #[test]
    fn complementary_html_parity() -> anyhow::Result<()> {
        let releases = [
            RunItemRow {
                title: "Version \"two\" & 'three' <four>".to_owned(),
                source_label: "Release & <source>".to_owned(),
                kind: crate::domain::SOURCE_KIND_GITHUB_RELEASE.to_owned(),
                ..RunItemRow::default()
            },
            RunItemRow {
                title: "Second release".to_owned(),
                url: "https://example.test/release?x=\"yes\"&y='no'".to_owned(),
                kind: crate::domain::SOURCE_KIND_GITHUB_RELEASE.to_owned(),
                ..RunItemRow::default()
            },
        ];
        let news = [
            RunItemRow {
                title: "Story & <tag> \"quoted\" 'single'".to_owned(),
                url: "https://example.test/news?x=1&y=2".to_owned(),
                source_label: "Unused source".to_owned(),
                section: "world_news-updates".to_owned(),
                outlet: "Outlet & <Co>".to_owned(),
                ..RunItemRow::default()
            },
            RunItemRow {
                title: "Second story".to_owned(),
                source_label: "Fallback \"source\"".to_owned(),
                section: "technology".to_owned(),
                ..RunItemRow::default()
            },
        ];
        let context = RunDeliveryContext {
            max_delivery_items: 7,
            sports_timezone: "Asia/Tokyo".to_owned(),
            sports_updates: vec![SportsUpdate {
                status: "upcoming".to_owned(),
                title: "Alpha & Beta <final>".to_owned(),
                competition: "League \"one\" & 'two'".to_owned(),
                starts_at: chrono::DateTime::parse_from_rfc3339("2026-01-01T18:05:00Z")?
                    .with_timezone(&chrono::Utc),
                images: (1..=3)
                    .map(|index| SportsImage {
                        url: format!("https://images.example/{index}.png?a=1&b=2"),
                        alt: "Not rendered".to_owned(),
                    })
                    .collect(),
                ..SportsUpdate::default()
            }],
            health_footnote: "Health \"one\" & 'two' <three>".to_owned(),
        };
        let [first_news, second_news] = &news;
        let [first_release, second_release] = &releases;
        let morning = render(
            &[first_news, first_release, second_news, second_release],
            &context,
            "2026-01-01T23:00:00Z",
            chrono_tz::Asia::Tokyo,
        )?;
        assert_eq!(
            morning,
            include_str!("../../tests/fixtures/email/morning-escaped-images.html")
        );
        let empty_context = RunDeliveryContext {
            sports_updates: vec![],
            health_footnote: String::new(),
            ..context
        };
        let empty = render(&[], &empty_context, "2026-01-01T12:00:00Z", chrono_tz::UTC)?;
        assert_eq!(
            empty,
            include_str!("../../tests/fixtures/email/empty-noon.html")
        );
        let mut unknown_context = empty_context;
        unknown_context.sports_updates.push(SportsUpdate {
            status: "in_progress".to_owned(),
            ..SportsUpdate::default()
        });
        let unknown = render(
            &[],
            &unknown_context,
            "2026-01-01T11:59:00Z",
            chrono_tz::UTC,
        )?;
        assert_eq!(
            unknown,
            include_str!("../../tests/fixtures/email/unknown-sports-morning.html")
        );
        Ok(())
    }
}
