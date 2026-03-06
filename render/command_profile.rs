/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Shared command-surface profile state helpers.
//!
//! These helpers keep category pin/recency persistence aligned across
//! command-surface modes (Search/Context/Radial).

use super::action_registry::{
    ActionCategory, CATEGORY_PIN_ORDER_PERSIST_KEY, CATEGORY_RECENCY_PERSIST_KEY,
    category_from_persisted_name, category_persisted_name,
};

pub(super) fn load_category_recency(ctx: &egui::Context) -> Vec<ActionCategory> {
    let raw = ctx
        .data_mut(|d| d.get_persisted::<Vec<String>>(egui::Id::new(CATEGORY_RECENCY_PERSIST_KEY)))
        .unwrap_or_default();
    raw.into_iter()
        .filter_map(|entry| category_from_persisted_name(&entry))
        .collect()
}

pub(super) fn persist_category_recency(ctx: &egui::Context, recency: &[ActionCategory]) {
    let raw: Vec<String> = recency
        .iter()
        .map(|category| category_persisted_name(*category).to_string())
        .collect();
    ctx.data_mut(|d| d.insert_persisted(egui::Id::new(CATEGORY_RECENCY_PERSIST_KEY), raw));
}

pub(super) fn record_recent_category(ctx: &egui::Context, category: ActionCategory) {
    let mut recency = load_category_recency(ctx);
    recency.retain(|entry| *entry != category);
    recency.insert(0, category);
    recency.truncate(4);
    persist_category_recency(ctx, &recency);
}

pub(super) fn load_pinned_categories(ctx: &egui::Context) -> Vec<ActionCategory> {
    let raw = ctx
        .data_mut(|d| d.get_persisted::<Vec<String>>(egui::Id::new(CATEGORY_PIN_ORDER_PERSIST_KEY)))
        .unwrap_or_default();
    raw.into_iter()
        .filter_map(|entry| category_from_persisted_name(&entry))
        .collect()
}

pub(super) fn persist_pinned_categories(ctx: &egui::Context, pinned: &[ActionCategory]) {
    let raw: Vec<String> = pinned
        .iter()
        .map(|category| category_persisted_name(*category).to_string())
        .collect();
    ctx.data_mut(|d| d.insert_persisted(egui::Id::new(CATEGORY_PIN_ORDER_PERSIST_KEY), raw));
}

pub(super) fn toggle_category_pin(ctx: &egui::Context, category: ActionCategory) {
    let mut pinned = load_pinned_categories(ctx);
    if pinned.contains(&category) {
        pinned.retain(|entry| *entry != category);
    } else {
        pinned.push(category);
    }
    persist_pinned_categories(ctx, &pinned);
}
