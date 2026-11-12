use crate::domain::SOURCE_KIND_GITHUB_RELEASE;
use crate::storage::RunItemRow;

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum ItemSection {
    Releases,
    Blogs,
    News,
    Forums,
}

impl ItemSection {
    const ALL: [Self; 4] = [Self::Releases, Self::Blogs, Self::News, Self::Forums];

    fn of(item: &RunItemRow) -> Self {
        match (item.kind.as_str(), item.section.as_str()) {
            (SOURCE_KIND_GITHUB_RELEASE, _) | (_, "releases") => Self::Releases,
            (_, "blogs") => Self::Blogs,
            (_, "forums") => Self::Forums,
            _ => Self::News,
        }
    }

    pub(super) const fn heading(self) -> &'static str {
        match self {
            Self::Releases => "Releases",
            Self::Blogs => "Blogs",
            Self::News => "News",
            Self::Forums => "Forums",
        }
    }

    pub(super) fn count_label(self, count: usize) -> String {
        let (singular, plural) = match self {
            Self::Releases => ("release", "releases"),
            Self::Blogs => ("blog post", "blog posts"),
            Self::News => ("news story", "news stories"),
            Self::Forums => ("forum discussion", "forum discussions"),
        };
        format!("{count} {}", if count == 1 { singular } else { plural })
    }
}

pub(super) fn group_items<'a>(
    selected: &[&'a RunItemRow],
) -> Vec<(ItemSection, Vec<&'a RunItemRow>)> {
    ItemSection::ALL
        .into_iter()
        .filter_map(|section| {
            let items: Vec<_> = selected
                .iter()
                .copied()
                .filter(|item| ItemSection::of(item) == section)
                .collect();
            (!items.is_empty()).then_some((section, items))
        })
        .collect()
}
