#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FetchedItem {
    pub title: String,
    pub url: String,
    pub published_at: String,
    pub identity: String,
    pub feed_identity: String,
    pub outlet: String,
    pub rss_source: String,
}

impl FetchedItem {
    #[must_use]
    pub fn feed_identity(&self) -> &str {
        if self.feed_identity.trim().is_empty() {
            &self.identity
        } else {
            &self.feed_identity
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct UnresolvedItem {
    pub disposition: crate::contract::ItemDisposition,
    pub title: String,
    pub url: String,
    pub reason: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FetchOutput {
    pub items: Vec<FetchedItem>,
    pub sports_updates: Vec<crate::contract::SportsUpdate>,
    pub unresolved: Vec<UnresolvedItem>,
    pub truncated: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolveError {
    Timeout,
    Message(String),
}

impl ResolveError {
    pub fn reason(&self) -> String {
        match self {
            Self::Timeout => "url canonicalization timed out".to_owned(),
            Self::Message(message) => message.clone(),
        }
    }

    pub fn is_backoff_signal(&self) -> bool {
        matches!(self, Self::Timeout)
            || matches!(self, Self::Message(message) if message.starts_with("HTTP 429"))
    }
}
