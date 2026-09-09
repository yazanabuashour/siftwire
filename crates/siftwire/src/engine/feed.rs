use anyhow::{Context, Result, bail, ensure};
use chrono::SecondsFormat;
use feed_rs::model::{Entry, FeedType, Link};
use quick_xml::NsReader;
use quick_xml::events::Event;
use quick_xml::name::{Namespace, QName, ResolveResult};
use regex::Regex;

use crate::domain::Source;

use super::model::FetchedItem;

pub fn parse_feed(source: &Source, body: &[u8]) -> Result<Vec<FetchedItem>> {
    let parser = feed_rs::parser::Builder::new()
        .sanitize_content(false)
        .id_generator(|links, title, base_uri| missing_identity(links, title.as_ref(), base_uri))
        .build();
    let feed = parser
        .parse(body)
        .map_err(anyhow::Error::new)
        .context("parse feed XML")?;
    if feed.feed_type == FeedType::JSON {
        bail!("parse feed XML: unsupported feed root \"json\"");
    }
    let dates = if source.is_current_news() {
        let dates =
            publication_dates(body, &feed.feed_type).context("read feed publication XML")?;
        ensure!(
            dates.len() == feed.entries.len(),
            "parse feed XML: publication entries do not align with feed entries"
        );
        dates
    } else if feed.feed_type == FeedType::Atom {
        raw_dates(body, "entry", "updated")?
    } else {
        raw_dates(body, "item", "pubDate")?
    };
    Ok(feed
        .entries
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| {
            fetched_item(
                entry,
                dates.get(index).map(String::as_str),
                source.is_current_news(),
            )
        })
        .collect())
}

fn fetched_item(entry: &Entry, raw_date: Option<&str>, current_news: bool) -> Option<FetchedItem> {
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
    // Optional news never falls back from absent, empty or invalid publication metadata.
    let published_at = if current_news {
        raw_date.unwrap_or_default().to_owned()
    } else {
        let parsed_date = entry
            .updated
            .or(entry.published)
            .map_or_else(String::new, |date| {
                date.to_rfc3339_opts(SecondsFormat::AutoSi, true)
            });
        raw_date
            .filter(|raw| !raw.is_empty())
            .map_or(parsed_date, str::to_owned)
    };
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

const ATOM_NAMESPACE: Namespace<'_> = Namespace(b"http://www.w3.org/2005/Atom");
const RSS_NAMESPACE: Namespace<'_> = Namespace(b"http://purl.org/rss/1.0/");
const DC_NAMESPACE: Namespace<'_> = Namespace(b"http://purl.org/dc/elements/1.1/");

fn publication_dates(body: &[u8], kind: &FeedType) -> Result<Vec<String>> {
    let mut reader = NsReader::from_reader(body);
    reader.config_mut().expand_empty_elements = true;
    loop {
        match reader.read_resolved_event()? {
            (namespace, Event::Start(root)) => {
                if *kind == FeedType::Atom && root.local_name().as_ref() == b"entry" {
                    let declared = publication_namespace(&namespace, kind);
                    return Ok(entry_publication(&mut reader, kind, declared)?
                        .into_iter()
                        .collect());
                }
                return container_publications(
                    &mut reader,
                    kind,
                    matches!(kind, FeedType::RSS0 | FeedType::RSS2),
                );
            }
            (_, Event::Eof) => bail!("missing feed root"),
            _ => {}
        }
    }
}

fn publication_namespace(namespace: &ResolveResult<'_>, kind: &FeedType) -> bool {
    if *kind == FeedType::Atom {
        *namespace == ResolveResult::Bound(ATOM_NAMESPACE)
    } else {
        matches!(namespace, ResolveResult::Unbound)
            || *namespace == ResolveResult::Bound(RSS_NAMESPACE)
    }
}

fn container_publications(
    reader: &mut NsReader<&[u8]>,
    kind: &FeedType,
    find_channel: bool,
) -> Result<Vec<String>> {
    let mut dates = Vec::new();
    loop {
        match reader.read_resolved_event()? {
            (namespace, Event::Start(element)) => {
                let name = element.local_name();
                // feed-rs chooses the first direct channel by local name, regardless of namespace.
                if find_channel && name.as_ref() == b"channel" {
                    return container_publications(reader, kind, false);
                }
                let declared = publication_namespace(&namespace, kind);
                // Mirror feed-rs's default namespace only for entry alignment, not publication trust.
                let feed_namespace = declared
                    || matches!(
                        namespace,
                        ResolveResult::Unbound | ResolveResult::Unknown(_)
                    );
                let entry_name: &[u8] = if *kind == FeedType::Atom {
                    b"entry"
                } else {
                    b"item"
                };
                if !find_channel && feed_namespace && name.as_ref() == entry_name {
                    if let Some(date) = entry_publication(reader, kind, declared)? {
                        dates.push(date);
                    }
                } else {
                    reader.read_to_end(element.name())?;
                }
            }
            (_, Event::End(_) | Event::Eof) => return Ok(dates),
            _ => {}
        }
    }
}

fn entry_publication(
    reader: &mut NsReader<&[u8]>,
    kind: &FeedType,
    declared: bool,
) -> Result<Option<String>> {
    let mut publication = None;
    let mut dublin_core = None;
    let mut has_link = false;
    loop {
        match reader.read_resolved_event()? {
            (namespace, Event::Start(element)) => {
                let name = element.local_name();
                let native = publication_namespace(&namespace, kind);
                let date_name: &[u8] = if *kind == FeedType::Atom {
                    b"published"
                } else {
                    b"pubDate"
                };
                if native && name.as_ref() == date_name {
                    let value = publication_text(reader)?;
                    publication.get_or_insert(value);
                } else if *kind != FeedType::Atom
                    && namespace == ResolveResult::Bound(DC_NAMESPACE)
                    && name.as_ref() == b"date"
                {
                    let value = publication_text(reader)?;
                    dublin_core.get_or_insert(value);
                } else if *kind == FeedType::RSS1
                    && (native || matches!(namespace, ResolveResult::Unknown(_)))
                    && name.as_ref() == b"link"
                {
                    has_link |= feed_link_has_text(reader, element.name())?;
                } else {
                    reader.read_to_end(element.name())?;
                }
            }
            (_, Event::End(_)) => {
                // RSS1 alone drops linkless entries; Atom/RSS2 keep entries even without titles.
                return Ok((*kind != FeedType::RSS1 || has_link).then(|| {
                    if declared {
                        publication.or(dublin_core).unwrap_or_default()
                    } else {
                        String::new()
                    }
                }));
            }
            (_, Event::Eof) => bail!("unclosed feed entry"),
            _ => {}
        }
    }
}

fn publication_text(reader: &mut NsReader<&[u8]>) -> Result<String> {
    let mut value = String::new();
    let mut nested = false;
    loop {
        match reader.read_event()? {
            Event::Text(text) => value.push_str(&text.decode()?),
            Event::CData(text) => value.push_str(&text.decode()?),
            Event::GeneralRef(reference) => {
                if let Some(character) = reference.resolve_char_ref()? {
                    value.push(character);
                } else {
                    let name = reference.decode()?;
                    if let Some(resolved) = quick_xml::escape::resolve_predefined_entity(&name) {
                        value.push_str(resolved);
                    } else {
                        value.push('&');
                        value.push_str(&name);
                        value.push(';');
                    }
                }
            }
            Event::Start(element) => {
                nested = true;
                reader.read_to_end(element.name())?;
            }
            Event::End(_) => {
                return Ok(if nested {
                    String::new()
                } else {
                    clean_text(&value)
                });
            }
            Event::Eof => bail!("unclosed publication element"),
            Event::Empty(_)
            | Event::Comment(_)
            | Event::Decl(_)
            | Event::PI(_)
            | Event::DocType(_) => {}
        }
    }
}

fn feed_link_has_text(reader: &mut NsReader<&[u8]>, end: QName<'_>) -> Result<bool> {
    // Match feed-rs child_as_text: only a leading nonempty text/CDATA/reference creates a link.
    loop {
        let has_text = match reader.read_event()? {
            Event::Text(text) if !text.is_empty() => true,
            Event::CData(text) if !text.is_empty() => true,
            Event::GeneralRef(_) => true,
            Event::Start(element) => {
                reader.read_to_end(element.name())?;
                false
            }
            Event::End(_) => return Ok(false),
            Event::Eof => bail!("unclosed RSS link"),
            Event::Empty(_)
            | Event::Text(_)
            | Event::CData(_)
            | Event::Comment(_)
            | Event::Decl(_)
            | Event::PI(_)
            | Event::DocType(_) => continue,
        };
        reader.read_to_end(end)?;
        return Ok(has_text);
    }
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
