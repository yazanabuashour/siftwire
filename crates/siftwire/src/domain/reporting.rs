use super::{
    SOURCE_KIND_GITHUB_RELEASE, SOURCE_KIND_RSS, SOURCE_KIND_SCHEDULE, THRESHOLD_ALWAYS,
    THRESHOLD_HIGH, THRESHOLD_MEDIUM,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Reporting {
    Required,
    Sports,
    Major,
    Highlights,
}

impl Reporting {
    fn from_fields(kind: &str, threshold: &str) -> Self {
        if kind == SOURCE_KIND_SCHEDULE {
            Self::Sports
        } else if threshold == THRESHOLD_ALWAYS || kind == SOURCE_KIND_GITHUB_RELEASE {
            Self::Required
        } else if threshold == THRESHOLD_HIGH {
            Self::Major
        } else {
            Self::Highlights
        }
    }

    pub fn from_recorded(kind: &str, threshold: &str) -> Option<Self> {
        let known = matches!(kind, SOURCE_KIND_SCHEDULE | SOURCE_KIND_GITHUB_RELEASE)
            || (kind == SOURCE_KIND_RSS
                && matches!(
                    threshold,
                    THRESHOLD_ALWAYS | THRESHOLD_HIGH | THRESHOLD_MEDIUM
                ));
        known.then(|| Self::from_fields(kind, threshold))
    }
}

impl super::Source {
    pub fn reporting(&self) -> Reporting {
        Reporting::from_fields(&self.kind, &self.threshold)
    }

    pub fn is_current_news(&self) -> bool {
        self.kind == SOURCE_KIND_RSS
            && matches!(self.reporting(), Reporting::Major | Reporting::Highlights)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reporting_uses_current_source_policy() {
        for (kind, threshold, expected) in [
            (SOURCE_KIND_SCHEDULE, THRESHOLD_ALWAYS, Reporting::Sports),
            (
                SOURCE_KIND_GITHUB_RELEASE,
                THRESHOLD_HIGH,
                Reporting::Required,
            ),
            (SOURCE_KIND_RSS, THRESHOLD_ALWAYS, Reporting::Required),
            (SOURCE_KIND_RSS, THRESHOLD_HIGH, Reporting::Major),
            (SOURCE_KIND_RSS, THRESHOLD_MEDIUM, Reporting::Highlights),
        ] {
            assert_eq!(Reporting::from_recorded(kind, threshold), Some(expected));
        }
        assert_eq!(Reporting::from_recorded("", ""), None);
    }
}
