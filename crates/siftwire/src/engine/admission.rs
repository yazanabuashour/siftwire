use unicode_normalization::UnicodeNormalization;
use url::Url;

use crate::contract::BriefItem;
use crate::domain::Source;

/// Conservative admission for optional news, not publisher authentication.
/// Required feeds and structured schedules retain their separate contracts.
pub(super) fn rejection_reason(source: &Source, item: &BriefItem) -> Option<&'static str> {
    if has_embedded_loader(&item.url) {
        return Some("embedded_loader_url");
    }
    if source.section == "sports" {
        return sports_rejection_reason(&item.title);
    }
    None
}

fn has_embedded_loader(value: &str) -> bool {
    Url::parse(value).is_ok_and(|url| {
        url.query_pairs().any(|(_key, value)| {
            let value = value.trim().to_ascii_lowercase();
            value.starts_with("data:") || value.starts_with("javascript:")
        })
    })
}

fn sports_rejection_reason(value: &str) -> Option<&'static str> {
    let folded = value.nfkc().collect::<String>().to_lowercase();
    let words = folded
        .split(|character: char| !character.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>();
    let title = words.join(" ");
    if ["watch today ", "where to watch ", "how to watch "]
        .iter()
        .any(|prefix| title.starts_with(prefix))
        || (title.starts_with("watch ")
            && words
                .iter()
                .any(|word| matches!(*word, "stream" | "streams" | "streaming" | "broadcast")))
    {
        return Some("sports_watch_guide");
    }
    let decorated = value
        .trim()
        .chars()
        .next()
        .is_some_and(|first| matches!(first, '!' | '[' | '【' | '(' | ')' | '^' | '+' | '~'));
    let stream_marker = words.iter().any(|word| {
        matches!(
            *word,
            "live"
                | "livefree"
                | "livestream"
                | "livestreams"
                | "watchlive"
                | "stream"
                | "streams"
                | "streaming"
                | "direct"
        )
    });
    let promotional = [
        "livefree",
        "live free",
        "free live",
        "stream free",
        "streams free",
        "en direct gratuit",
        "en direct gratuity",
        "en direct streaming",
    ]
    .iter()
    .any(|phrase| title.contains(phrase))
        || (decorated && stream_marker)
        || ["livestream", "watchlive", "official tv", "live now"]
            .iter()
            .any(|prefix| title.starts_with(prefix))
        || (words.iter().any(|word| matches!(*word, "vs" | "v"))
            && title.contains("live broadcast"));
    promotional.then_some("promotional_livestream")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admission_keeps_scope_and_decodes_query_values() {
        let sports = Source {
            section: "sports".to_owned(),
            ..Source::default()
        };
        for url in [
            "https://publisher.test/tour?xml=%64ata%3Avideo%2Fmp4%3Bbase64%2CFAKE",
            "https://other.test/story?content=JavaScript%3Aalert(1)",
        ] {
            let item = BriefItem {
                title: "Ordinary title".to_owned(),
                url: url.to_owned(),
                ..BriefItem::default()
            };
            assert_eq!(
                rejection_reason(&sports, &item),
                Some("embedded_loader_url"),
                "encoded loader admitted: {url}"
            );
        }
        for title in [
            "How to watch Team Alpha vs Team Beta",
            "Where to watch Team Alpha live",
        ] {
            let item = BriefItem {
                title: title.to_owned(),
                ..BriefItem::default()
            };
            assert_eq!(
                rejection_reason(&sports, &item),
                Some("sports_watch_guide"),
                "watch guide admitted"
            );
        }
        let item = BriefItem {
            title: "Free live streaming tier launched - Wire".to_owned(),
            url: "https://publisher.test/news?topic=live&format=xml".to_owned(),
            ..BriefItem::default()
        };
        assert_eq!(
            rejection_reason(
                &Source {
                    section: "technology".to_owned(),
                    ..Source::default()
                },
                &item
            ),
            None,
            "sports wording rule leaked into technology news"
        );
        assert!(
            has_embedded_loader("https://publisher.test/?xml=data:text/plain,hello"),
            "data loader admitted"
        );
        assert!(
            !has_embedded_loader(
                "https://publisher.test/news?xml=https%3A%2F%2Fpublisher.test%2Ffeed"
            ),
            "ordinary XML query mistaken for a loader"
        );
    }
}
