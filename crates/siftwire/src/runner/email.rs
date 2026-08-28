use std::fmt::Write as _;

use anyhow::{Context, Result};
use chrono::{DateTime, Timelike, Utc};
use chrono_tz::Tz;

use crate::contract::{SportsImage, SportsUpdate};
use crate::domain::SOURCE_KIND_GITHUB_RELEASE;
use crate::storage::{RunDeliveryContext, RunItemRow};

const TEXT: &str = "#202124";
const MUTED: &str = "#5f6368";
const ACCENT: &str = "#6d4aff";
const SECONDARY: &str = "#a14200";
const SUCCESS: &str = "#137333";
const PANEL: &str = "#f7f5f1";
const BORDER: &str = "#dadce0";

pub fn render(
    selected: &[&RunItemRow],
    context: &RunDeliveryContext,
    started_at: &str,
    timezone: Tz,
) -> Result<String> {
    let started_at = DateTime::parse_from_rfc3339(started_at)
        .context("stored brief start time is invalid")?
        .with_timezone(&Utc)
        .with_timezone(&timezone);
    let edition = if started_at.hour() < 12 {
        "Morning"
    } else {
        "Evening"
    };
    let releases = selected
        .iter()
        .copied()
        .filter(|item| item.kind == SOURCE_KIND_GITHUB_RELEASE)
        .collect::<Vec<_>>();
    let news = selected
        .iter()
        .copied()
        .filter(|item| item.kind != SOURCE_KIND_GITHUB_RELEASE)
        .collect::<Vec<_>>();
    let preheader = format!(
        "{} stories, {} releases, {} sports updates.",
        news.len(),
        releases.len(),
        context.sports_updates.len()
    );
    let mut output = document_start(edition, &started_at, &preheader);
    if !releases.is_empty() {
        append_releases(&mut output, &releases);
    }
    if !news.is_empty() {
        append_news(&mut output, &news);
    }
    append_sports(&mut output, &context.sports_updates, timezone);
    append_health(&mut output, &context.health_footnote);
    let _result = write!(
        output,
        r#"<tr><td class="mobile-pad" style="padding:12px 20px;border-top:1px solid {BORDER};font-size:11px;line-height:16px;color:{MUTED};">Times: {timezone}</td></tr>
</table></td></tr></table>
</body>
</html>"#,
    );
    Ok(output)
}

fn document_start(edition: &str, started_at: &DateTime<Tz>, preheader: &str) -> String {
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="color-scheme" content="light">
<meta name="supported-color-schemes" content="light">
<title>SiftWire {edition} brief</title>
<style>
@media only screen and (max-width: 680px) {{
  .email-shell {{ width:100% !important; }}
  .mobile-pad {{ padding-left:16px !important; padding-right:16px !important; }}
  .story-title {{ font-size:16px !important; line-height:21px !important; }}
  .date-cell {{ width:45px !important; }}
}}
</style>
</head>
<body style="margin:0;padding:0;background:#ffffff;color:{TEXT};font-family:Arial,Helvetica,sans-serif;">
<div style="display:none;max-height:0;overflow:hidden;opacity:0;color:transparent;">{preheader}</div>
<table role="presentation" width="100%" cellspacing="0" cellpadding="0" border="0" style="width:100%;background:#ffffff;">
<tr><td align="center" style="padding:0;">
<table role="presentation" width="680" cellspacing="0" cellpadding="0" border="0" class="email-shell" style="width:680px;max-width:680px;background:#ffffff;border-top:3px solid {ACCENT};">
<tr><td class="mobile-pad" style="padding:13px 20px 12px;border-bottom:1px solid {BORDER};">
<table role="presentation" width="100%" cellspacing="0" cellpadding="0" border="0"><tr>
<td valign="top"><div style="font-family:Georgia,'Times New Roman',serif;font-size:21px;line-height:25px;font-weight:bold;color:{TEXT};">SiftWire</div><div style="margin-top:3px;font-size:11px;line-height:15px;color:{MUTED};">{preheader}</div></td>
<td align="right" valign="top" style="font-size:10px;line-height:15px;letter-spacing:1.2px;text-transform:uppercase;color:{SECONDARY};">{edition}<br><span style="color:{MUTED};">{date}</span></td>
</tr></table>
</td></tr>"#,
        preheader = html_escape(preheader),
        date = started_at.format("%a, %b %-d"),
    )
}

fn append_releases(output: &mut String, releases: &[&RunItemRow]) {
    section_start(output, "New versions");
    let _result = write!(
        output,
        r#"<table role="presentation" width="100%" cellspacing="0" cellpadding="0" border="0" style="background:{PANEL};border:1px solid {BORDER};">"#,
    );
    for item in releases {
        let title = linked_title(
            &item.title,
            &item.url,
            "font-family:Georgia,'Times New Roman',serif;font-size:15px;line-height:20px;color:#202124;text-decoration:underline;text-decoration-color:#6d4aff;",
        );
        let _result = write!(
            output,
            r#"<tr><td style="padding:10px 12px;border-bottom:1px solid {BORDER};">
<div style="font-size:9px;line-height:13px;letter-spacing:.8px;text-transform:uppercase;color:{SECONDARY};margin-bottom:3px;">{source}</div>
{title}
</td></tr>"#,
            source = html_escape(&item.source_label),
        );
    }
    output.push_str("</table></td></tr>");
}

fn append_news(output: &mut String, news: &[&RunItemRow]) {
    section_start(output, "News");
    for item in news {
        let section = if item.section.is_empty() {
            item.source_label.clone()
        } else {
            display_label(&item.section)
        };
        let outlet = if item.outlet.is_empty() {
            item.source_label.as_str()
        } else {
            item.outlet.as_str()
        };
        let title = linked_title(
            &item.title,
            &item.url,
            "font-family:Georgia,'Times New Roman',serif;font-size:17px;line-height:23px;color:#202124;text-decoration:underline;text-decoration-color:#6d4aff;",
        );
        let _result = write!(
            output,
            r#"<table role="presentation" width="100%" cellspacing="0" cellpadding="0" border="0" style="border-bottom:1px solid {BORDER};"><tr><td style="padding:10px 0 11px;">
<div style="font-size:9px;line-height:13px;letter-spacing:.7px;text-transform:uppercase;color:{SECONDARY};margin-bottom:4px;">{section} · {outlet}</div>
{title}
</td></tr></table>"#,
            section = html_escape(&section),
            outlet = html_escape(outlet),
        );
    }
    output.push_str("</td></tr>");
}

fn append_sports(output: &mut String, updates: &[SportsUpdate], timezone: Tz) {
    if updates.is_empty() {
        return;
    }
    section_start(output, "Sports");
    append_sports_group(output, "Upcoming", "upcoming", updates, timezone);
    append_sports_group(output, "Results", "final", updates, timezone);
    output.push_str("</td></tr>");
}

fn append_sports_group(
    output: &mut String,
    heading: &str,
    status: &str,
    updates: &[SportsUpdate],
    timezone: Tz,
) {
    let matching = updates
        .iter()
        .filter(|update| update.status == status)
        .collect::<Vec<_>>();
    if matching.is_empty() {
        return;
    }
    let _result = write!(
        output,
        r#"<table role="presentation" width="100%" cellspacing="0" cellpadding="0" border="0" style="margin-bottom:12px;background:{PANEL};border:1px solid {BORDER};">
<tr><td colspan="3" style="padding:8px 10px;border-bottom:1px solid {BORDER};font-size:10px;line-height:14px;font-weight:bold;letter-spacing:.8px;text-transform:uppercase;color:{ACCENT};">{heading}</td></tr>"#,
    );
    for update in matching {
        append_sports_update(output, update, timezone, status);
    }
    output.push_str("</table>");
}

fn append_sports_update(output: &mut String, update: &SportsUpdate, timezone: Tz, status: &str) {
    let when = update.starts_at.with_timezone(&timezone);
    let date_color = if status == "final" {
        SUCCESS
    } else {
        SECONDARY
    };
    let _result = write!(
        output,
        r#"<tr><td class="date-cell" width="52" valign="middle" style="width:52px;padding:9px 6px 9px 10px;border-bottom:1px solid {BORDER};">
<div style="font-size:9px;line-height:12px;text-transform:uppercase;color:{date_color};">{weekday}</div>
<div style="font-size:19px;line-height:21px;font-weight:bold;color:{TEXT};">{day}</div>
<div style="font-size:9px;line-height:12px;text-transform:uppercase;color:{MUTED};">{month}</div>
</td>"#,
        weekday = when.format("%a"),
        day = when.format("%-d"),
        month = when.format("%b"),
    );
    append_images(output, &update.images);
    let title = linked_title(
        &update.title,
        &update.url,
        "font-size:14px;line-height:19px;font-weight:bold;color:#202124;text-decoration:underline;text-decoration-color:#6d4aff;",
    );
    let _result = write!(
        output,
        r#"<td valign="middle" style="padding:9px 10px;border-bottom:1px solid {BORDER};">
{title}
<div style="margin-top:3px;font-size:10px;line-height:15px;color:{MUTED};">{competition} · {time}</div>
</td></tr>"#,
        competition = html_escape(&update.competition),
        time = when.format("%-I:%M %p %Z"),
    );
}

fn append_images(output: &mut String, images: &[SportsImage]) {
    if images.is_empty() {
        return;
    }
    output.push_str(
        r#"<td width="66" valign="middle" style="width:66px;padding:8px 4px;border-bottom:1px solid #dadce0;"><table role="presentation" cellspacing="0" cellpadding="0" border="0"><tr>"#,
    );
    let plate = "#292b31";
    for image in images.iter().take(2) {
        let _result = write!(
            output,
            r#"<td style="padding-right:3px;"><img src="{url}" alt="" width="28" height="28" style="display:block;width:28px;height:28px;border:1px solid {BORDER};border-radius:50%;background:{plate};object-fit:contain;"></td>"#,
            url = html_escape(&image.url),
        );
    }
    output.push_str("</tr></table></td>");
}

fn section_start(output: &mut String, heading: &str) {
    let _result = write!(
        output,
        r#"<tr><td class="mobile-pad" style="padding:17px 20px 0;"><h2 style="margin:0 0 9px;font-family:Georgia,'Times New Roman',serif;font-size:21px;line-height:26px;font-weight:normal;color:{TEXT};">{heading}</h2>"#,
        heading = html_escape(heading),
    );
}

fn append_health(output: &mut String, health: &str) {
    if health.is_empty() {
        return;
    }
    let _result = write!(
        output,
        r#"<tr><td class="mobile-pad" style="padding:5px 20px 14px;"><div style="padding:9px 10px;background:{PANEL};border-left:3px solid {SECONDARY};font-size:11px;line-height:16px;color:{MUTED};"><strong style="color:{TEXT};">Source health</strong> · {health}</div></td></tr>"#,
        health = html_escape(health),
    );
}

fn linked_title(title: &str, url: &str, style: &str) -> String {
    let title = html_escape(title);
    if url.is_empty() {
        format!(r#"<span style="{style}">{title}</span>"#)
    } else {
        format!(
            r#"<a href="{}" style="{style}">{title}</a>"#,
            html_escape(url)
        )
    }
}

fn display_label(value: &str) -> String {
    value.replace(['_', '-'], " ")
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
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
    fn compact_light_brief_preserves_edition_and_escaping() -> anyhow::Result<()> {
        let item = RunItemRow {
            title: "Release <one>".to_owned(),
            url: "https://example.test/?a=1&b=2".to_owned(),
            source_label: "Example".to_owned(),
            kind: crate::domain::SOURCE_KIND_GITHUB_RELEASE.to_owned(),
            ..RunItemRow::default()
        };
        let context = RunDeliveryContext {
            max_delivery_items: 7,
            sports_timezone: "America/Chicago".to_owned(),
            sports_updates: vec![SportsUpdate {
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
            }],
            health_footnote: String::new(),
        };
        let evening = render(
            &[&item],
            &context,
            "2026-08-28T00:00:00Z",
            chrono_tz::America::Chicago,
        )?;
        assert!(evening.contains("Evening"));
        assert!(evening.contains("background:#ffffff"));
        assert!(!evening.contains("Your evening brief"));
        assert!(evening.contains("Release &lt;one&gt;"));
        assert!(evening.contains("a=1&amp;b=2"));
        assert!(evening.contains("https://images.example/alpha.png"));
        Ok(())
    }
}
