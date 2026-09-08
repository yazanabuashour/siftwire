#[cfg(test)]
mod normalization_tests;
#[cfg(test)]
mod tests;

use crate::contract::{DeliveryItem, SportsUpdate};
use crate::domain::SOURCE_KIND_SCHEDULE;
use crate::storage::{
    RUN_ITEM_CANDIDATE, RUN_ITEM_MUST_INCLUDE, RunDeliveryContext, RunDetail, RunItemRow,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum DeliveryStatus {
    Sent,
    SentAsSports,
    NotSelected,
    NotDelivered,
    Unknown,
}

impl DeliveryStatus {
    pub(super) const fn selected(self) -> bool {
        matches!(self, Self::Sent | Self::SentAsSports)
    }
}

// A sports URL can identify a whole event or card. Require the recorded source,
// compatibility title, start time and URL, and reject ambiguous projections.
pub(super) fn sports_row<'a>(
    update: &SportsUpdate,
    context: &RunDeliveryContext,
    rows: &'a [RunItemRow],
) -> Option<&'a RunItemRow> {
    if update.status != "upcoming" {
        return None;
    }
    let projected =
        crate::engine::sports_update_item(update, context.sports_timezone.parse().ok()?);
    let mut matching = rows.iter().filter(|row| {
        row.category == RUN_ITEM_MUST_INCLUDE
            && row.kind == SOURCE_KIND_SCHEDULE
            && row.source_key == update.source_key
            && row.title == projected.title
            && row.url == projected.url
            && row.published_at == projected.published_at
    });
    let row = matching.next()?;
    matching.next().is_none().then_some(row)
}

fn same_item(row: &RunItemRow, item: &DeliveryItem) -> bool {
    row.title == item.title
        && super::delivery::normalize_delivery_url(&row.url).unwrap_or_default() == item.url
        && row.kind == item.kind
}

pub(super) fn delivery_status(row: &RunItemRow, detail: &RunDetail) -> DeliveryStatus {
    use DeliveryStatus::{NotDelivered, NotSelected, Sent, SentAsSports, Unknown};
    if detail.summary.delivered_at.is_none() {
        return NotDelivered;
    }
    if let Some(plan) = &detail.delivery_plan {
        if detail.summary.message.as_deref() != Some(plan.message.as_str()) {
            return Unknown;
        }
        if plan.items.iter().any(|item| {
            item.run_item_ids
                .as_ref()
                .is_some_and(|ids| !row.id.is_empty() && ids.contains(&row.id))
        }) {
            return if row.kind == SOURCE_KIND_SCHEDULE
                && detail
                    .delivery_context
                    .as_ref()
                    .is_some_and(|context| !context.sports_updates.is_empty())
            {
                SentAsSports
            } else {
                Sent
            };
        }
        if row.category == RUN_ITEM_CANDIDATE {
            let index = detail
                .items
                .iter()
                .filter(|item| item.category == RUN_ITEM_CANDIDATE)
                .position(|item| std::ptr::eq(item, row));
            if index.is_some_and(|index| !plan.candidate_indexes.contains(&index)) {
                return NotSelected;
            }
        }
        if row.kind == SOURCE_KIND_SCHEDULE {
            return legacy_sports_status(row, detail);
        }
        // Old plans have no references. Both sides must identify exactly one item.
        let mut matches = plan
            .items
            .iter()
            .filter(|item| item.run_item_ids.is_none() && same_item(row, item));
        if let Some(item) = matches.next()
            && matches.next().is_none()
            && detail
                .items
                .iter()
                .filter(|row| {
                    matches!(
                        row.category.as_str(),
                        RUN_ITEM_CANDIDATE | RUN_ITEM_MUST_INCLUDE
                    ) && same_item(row, item)
                })
                .count()
                == 1
        {
            return Sent;
        }
        return Unknown;
    }
    // Legacy free-text delivery parsing cannot prove that a missing link was
    // excluded. Only a unique exact title/normalized-URL pair proves inclusion.
    if row.kind == SOURCE_KIND_SCHEDULE {
        return legacy_sports_status(row, detail);
    }
    let url = super::delivery::normalize_delivery_url(&row.url).unwrap_or_default();
    if detail.delivery_context.as_ref().is_some_and(|context| {
        context.sports_updates.iter().any(|update| {
            update.title == row.title
                && super::delivery::normalize_delivery_url(&update.url).unwrap_or_default() == url
        })
    }) {
        return Unknown;
    }
    if detail
        .sent_items
        .iter()
        .filter(|item| item.title == row.title && item.url == url)
        .count()
        == 1
        && detail
            .items
            .iter()
            .filter(|item| {
                matches!(
                    item.category.as_str(),
                    RUN_ITEM_CANDIDATE | RUN_ITEM_MUST_INCLUDE
                ) && item.title == row.title
                    && super::delivery::normalize_delivery_url(&item.url).unwrap_or_default() == url
            })
            .count()
            == 1
    {
        Sent
    } else {
        Unknown
    }
}

fn legacy_sports_status(row: &RunItemRow, detail: &RunDetail) -> DeliveryStatus {
    let Some(context) = &detail.delivery_context else {
        return DeliveryStatus::Unknown;
    };
    let mut updates = context.sports_updates.iter().filter(|update| {
        sports_row(update, context, &detail.items).is_some_and(|matched| std::ptr::eq(matched, row))
    });
    let Some(update) = updates.next() else {
        return DeliveryStatus::Unknown;
    };
    if updates.next().is_some() {
        return DeliveryStatus::Unknown;
    }
    let url = super::delivery::normalize_delivery_url(&update.url).unwrap_or_default();
    // Distinct fixtures may share both the short title and URL. Context must
    // disambiguate the delivered projection too, not just the compatibility row.
    if context
        .sports_updates
        .iter()
        .filter(|other| {
            other.title == update.title
                && super::delivery::normalize_delivery_url(&other.url).unwrap_or_default() == url
        })
        .count()
        != 1
    {
        return DeliveryStatus::Unknown;
    }
    let confirmed = detail.delivery_plan.as_ref().map_or_else(
        || {
            detail
                .sent_items
                .iter()
                .filter(|item| item.title == update.title && item.url == url)
                .count()
                == 1
                && !detail.items.iter().any(|item| {
                    item.kind != SOURCE_KIND_SCHEDULE
                        && item.title == update.title
                        && super::delivery::normalize_delivery_url(&item.url).unwrap_or_default()
                            == url
                })
        },
        |plan| {
            plan.items
                .iter()
                .filter(|item| {
                    item.run_item_ids.is_none()
                        && item.kind == SOURCE_KIND_SCHEDULE
                        && item.title == update.title
                        && item.url == url
                })
                .count()
                == 1
        },
    );
    if confirmed {
        DeliveryStatus::SentAsSports
    } else {
        DeliveryStatus::Unknown
    }
}
