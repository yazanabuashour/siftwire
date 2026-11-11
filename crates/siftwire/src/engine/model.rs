#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FetchedItem {
    pub title: String,
    pub summary: String,
    pub url: String,
    pub published_at: String,
    pub identity: String,
    pub outlet: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FetchOutput {
    pub items: Vec<FetchedItem>,
    pub sports_updates: Vec<crate::contract::SportsUpdate>,
}
