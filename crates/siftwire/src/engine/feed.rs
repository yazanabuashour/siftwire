use anyhow::{Context, Result, bail};
use chrono::SecondsFormat;
use feed_rs::model::{Entry, FeedType, Link};
use regex::Regex;

use super::model::FetchedItem;

pub fn parse_feed(body: &[u8]) -> Result<Vec<FetchedItem>> {
    let parser = feed_rs::parser::Builder::new()
        .sanitize_content(false)
        .id_generator(|links, title, base_uri| missing_identity(links, title.as_ref(), base_uri))
        .build();
    let feed = parser
        .parse(body)
        .map_err(anyhow::Error::new)
        .context("parse feed XML")?;
    let dates = match feed.feed_type {
        FeedType::Atom => raw_dates(body, "entry", "updated")?,
        FeedType::RSS0 | FeedType::RSS1 | FeedType::RSS2 => raw_dates(body, "item", "pubDate")?,
        FeedType::JSON => bail!("parse feed XML: unsupported feed root \"json\""),
    };
    Ok(feed
        .entries
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| fetched_item(entry, dates.get(index).map(String::as_str)))
        .collect())
}

fn fetched_item(entry: &Entry, raw_date: Option<&str>) -> Option<FetchedItem> {
    let title = clean_text(entry.title.as_ref()?.content.as_str());
    if title.is_empty() {
        return None;
    }
    let url = entry_link(&entry.links);
    let identity = if entry.id.trim().is_empty() {
        if url.is_empty() {
            title.clone()
        } else {
            url.clone()
        }
    } else {
        entry.id.trim().to_owned()
    };
    let parsed_date = entry
        .updated
        .or(entry.published)
        .map_or_else(String::new, |date| {
            date.to_rfc3339_opts(SecondsFormat::AutoSi, true)
        });
    let published_at = raw_date
        .filter(|raw| !raw.is_empty())
        .map_or(parsed_date, str::to_owned);
    Some(FetchedItem {
        title,
        url,
        published_at,
        identity: identity.clone(),
        feed_identity: identity,
        outlet: String::new(),
    })
}

fn missing_identity(
    links: &[Link],
    title: Option<&feed_rs::model::Text>,
    _base_uri: Option<&str>,
) -> String {
    let link = entry_link(links);
    if link.is_empty() {
        title.map_or_else(String::new, |text| clean_text(&text.content))
    } else {
        link
    }
}

fn entry_link(links: &[Link]) -> String {
    links
        .iter()
        .find(|link| {
            link.rel
                .as_deref()
                .is_none_or(|relation| relation.is_empty() || relation == "alternate")
        })
        .or_else(|| links.first())
        .map_or_else(String::new, |link| link.href.trim().to_owned())
}

fn raw_dates(body: &[u8], block_name: &str, date_name: &str) -> Result<Vec<String>> {
    let block_pattern = Regex::new(&format!(
        r"(?is)<(?:[a-z0-9_-]+:)?{block_name}(?:\s[^>]*)?>(.*?)</(?:[a-z0-9_-]+:)?{block_name}\s*>"
    ))
    .context("compile feed item pattern")?;
    let date_pattern = child_pattern(date_name)?;
    let tags_pattern = Regex::new(r"(?is)<[^>]+>").context("compile XML tag pattern")?;
    let xml = String::from_utf8_lossy(body);
    Ok(block_pattern
        .captures_iter(&xml)
        .filter_map(|block| block.get(1))
        .map(|contents| child_text(contents.as_str(), &date_pattern, &tags_pattern))
        .collect())
}

fn child_pattern(name: &str) -> Result<Regex> {
    Regex::new(&format!(
        r"(?is)<(?:[a-z0-9_-]+:)?{name}(?:\s[^>]*)?>(.*?)</(?:[a-z0-9_-]+:)?{name}\s*>"
    ))
    .with_context(|| format!("compile feed {name} pattern"))
}

fn child_text(contents: &str, pattern: &Regex, tags: &Regex) -> String {
    pattern
        .captures(contents)
        .and_then(|captures| captures.get(1))
        .map_or_else(String::new, |value| xml_text(value.as_str(), tags))
}

fn xml_text(value: &str, tags: &Regex) -> String {
    let trimmed = value.trim();
    if let Some(cdata) = trimmed
        .strip_prefix("<![CDATA[")
        .and_then(|text| text.strip_suffix("]]>"))
    {
        return clean_text(cdata);
    }
    let without_tags = tags.replace_all(trimmed, " ");
    clean_text(&decode_xml_entities(&without_tags))
}

fn decode_xml_entities(value: &str) -> String {
    let mut decoded = String::with_capacity(value.len());
    let mut characters = value.chars();
    while let Some(character) = characters.next() {
        if character != '&' {
            decoded.push(character);
            continue;
        }
        let mut reference = String::new();
        let mut terminated = false;
        for candidate in characters.by_ref() {
            if candidate == ';' {
                terminated = true;
                break;
            }
            reference.push(candidate);
        }
        if terminated {
            if let Some(character) = decode_xml_reference(&reference) {
                decoded.push(character);
            } else {
                decoded.push('&');
                decoded.push_str(&reference);
                decoded.push(';');
            }
        } else {
            decoded.push('&');
            decoded.push_str(&reference);
        }
    }
    decoded
}

fn decode_xml_reference(reference: &str) -> Option<char> {
    let code = match reference {
        "lt" => return Some('<'),
        "gt" => return Some('>'),
        "quot" => return Some('"'),
        "apos" => return Some('\''),
        "amp" => return Some('&'),
        _ => reference
            .strip_prefix("#x")
            .and_then(|hex| u32::from_str_radix(hex, 16).ok())
            .or_else(|| {
                reference
                    .strip_prefix('#')
                    .and_then(|decimal| decimal.parse::<u32>().ok())
            })?,
    };
    valid_xml_character(code)
        .then_some(code)
        .and_then(char::from_u32)
}

const fn valid_xml_character(code: u32) -> bool {
    matches!(
        code,
        0x9 | 0xA | 0xD | 0x20..=0xD7FF | 0xE000..=0xFFFD | 0x1_0000..=0x10_FFFF
    )
}

fn clean_text(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}
