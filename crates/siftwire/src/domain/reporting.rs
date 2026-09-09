use super::{
    SOURCE_KIND_ATOM, SOURCE_KIND_GITHUB_RELEASE, SOURCE_KIND_RSS, SOURCE_KIND_SCHEDULE,
    THRESHOLD_ALWAYS, THRESHOLD_AUDIT, THRESHOLD_HIGH, THRESHOLD_MEDIUM,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Reporting {
    Required,
    Sports,
    // Read compatibility for recorded items and disabled legacy sources only.
    Observe,
    Major,
    Highlights,
}

impl Reporting {
    fn from_fields(kind: &str, threshold: &str, always_report: bool) -> Self {
        // Sports recur outside normal candidate slots, regardless of legacy flags.
        if kind == SOURCE_KIND_SCHEDULE {
            Self::Sports
        } else if always_report
            || threshold == THRESHOLD_ALWAYS
            || kind == SOURCE_KIND_GITHUB_RELEASE
        {
            Self::Required
        } else if threshold == THRESHOLD_AUDIT {
            Self::Observe
        } else if threshold == THRESHOLD_HIGH {
            Self::Major
        } else {
            Self::Highlights
        }
    }

    pub fn from_recorded(kind: &str, threshold: &str, always_report: bool) -> Option<Self> {
        let known = matches!(kind, SOURCE_KIND_SCHEDULE | SOURCE_KIND_GITHUB_RELEASE)
            || (matches!(kind, SOURCE_KIND_RSS | SOURCE_KIND_ATOM)
                && (always_report
                    || matches!(
                        threshold,
                        THRESHOLD_ALWAYS | THRESHOLD_AUDIT | THRESHOLD_HIGH | THRESHOLD_MEDIUM
                    )));
        known.then(|| Self::from_fields(kind, threshold, always_report))
    }
}

impl super::Source {
    pub fn reporting(&self) -> Reporting {
        Reporting::from_fields(&self.kind, &self.threshold, false)
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
    fn precedence_preserves_every_legacy_classification() {
        for kind in [
            "rss",
            "atom",
            SOURCE_KIND_GITHUB_RELEASE,
            SOURCE_KIND_SCHEDULE,
        ] {
            for threshold in ["always", "audit", "high", "medium"] {
                for always in [false, true] {
                    let reporting = Reporting::from_fields(kind, threshold, always);
                    let required = always
                        || threshold == "always"
                        || matches!(kind, "github_release" | "sports_schedule");
                    assert_eq!(
                        matches!(reporting, Reporting::Required | Reporting::Sports),
                        required
                    );
                    assert_eq!(
                        reporting == Reporting::Observe,
                        !required && threshold == "audit"
                    );
                    if kind == SOURCE_KIND_SCHEDULE {
                        assert_eq!(reporting, Reporting::Sports);
                    } else if always || threshold == "always" || kind == SOURCE_KIND_GITHUB_RELEASE
                    {
                        assert_eq!(reporting, Reporting::Required);
                    }
                    assert_eq!(
                        Reporting::from_recorded(kind, threshold, always),
                        Some(reporting)
                    );
                }
            }
        }
        assert_eq!(
            Reporting::from_fields("rss", "high", false),
            Reporting::Major
        );
        assert_eq!(
            Reporting::from_fields("rss", "medium", false),
            Reporting::Highlights
        );
    }

    #[test]
    fn incomplete_history_does_not_invent_a_policy() {
        assert_eq!(Reporting::from_recorded("", "", false), None);
        assert_eq!(Reporting::from_recorded("", "always", true), None);
        assert_eq!(Reporting::from_recorded("rss", "", false), None);
        assert_eq!(
            Reporting::from_recorded("rss", "", true),
            Some(Reporting::Required)
        );
        assert_eq!(
            Reporting::from_recorded("sports_schedule", "", false),
            Some(Reporting::Sports)
        );
    }
}
