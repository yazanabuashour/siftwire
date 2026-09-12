#[cfg(test)]
mod normalization_tests;

use crate::storage::{RUN_ITEM_CANDIDATE, RunDetail, RunItemRow};

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum DeliveryStatus {
    Sent,
    NotSelected,
    NotDelivered,
    Unknown,
}

pub(super) fn delivery_status(row: &RunItemRow, detail: &RunDetail) -> DeliveryStatus {
    use DeliveryStatus::{NotDelivered, NotSelected, Sent, Unknown};
    if detail.summary.delivered_at.is_none() {
        return NotDelivered;
    }
    let Some(plan) = &detail.delivery_plan else {
        return Unknown;
    };
    if detail.summary.message.as_deref() != Some(plan.message.as_str()) {
        return Unknown;
    }
    if plan
        .items
        .iter()
        .any(|item| item.run_item_ids.contains(&row.id))
    {
        return Sent;
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
    Unknown
}
