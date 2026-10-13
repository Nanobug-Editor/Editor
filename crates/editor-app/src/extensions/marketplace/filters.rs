//! Retained discovery filters presented in a popover inside the shared search frame.
use super::*;
use crate::ui::controls::{Icon, popover};

/// Each choice changes existing catalog search inputs; it never grants install authority.
#[derive(Clone)]
enum Filter {
    Category(String),
    Tag(String),
    Sort(Sort),
    Clear,
}

impl ExtensionPanel {
    /// The leading trigger shares the input border; active conditions have one visible marker.
    pub(in crate::extensions) fn market_filters(&self, cx: &Context<Self>) -> AnyElement {
        let active = !self.market_view.category.is_empty()
            || !self.market_view.tag.is_empty()
            || self.market_view.sort != Sort::Relevance;
        let trigger = Button::new("market-filter-trigger")
            .debug_selector(|| "market-filter-trigger".into())
            .ghost()
            .small()
            .compact()
            .h(px(24.))
            .w(px(24.))
            .icon(Icon::default().data(include_bytes!("../../../assets/icons/filter.svg")))
            .accessibility_label(t!("plugins.market_filters").to_string())
            .tooltip(t!("plugins.market_filters").to_string())
            .when(active, |button| {
                button.child(
                    div()
                        .debug_selector(|| "market-filter-active".into())
                        .size(px(4.))
                        .rounded_full()
                        .bg(cx.theme().primary),
                )
            });
        let state = self.worker.state.lock().unwrap();
        let categories = state
            .market
            .catalog
            .as_ref()
            .map(|catalog| {
                catalog
                    .plugins
                    .iter()
                    .map(|listing| listing.category.clone())
                    .collect::<std::collections::BTreeSet<_>>()
            })
            .unwrap_or_default();
        let tags = state
            .market
            .catalog
            .as_ref()
            .map(|catalog| {
                catalog
                    .plugins
                    .iter()
                    .flat_map(|listing| listing.tags.iter().cloned())
                    .collect::<std::collections::BTreeSet<_>>()
            })
            .unwrap_or_default();
        drop(state);
        let mut categories_row = h_flex()
            .flex_wrap()
            .gap_1()
            .child(self.market_filter_choice(
                "market-category-all".into(),
                t!("plugins.market_all_categories").to_string(),
                Filter::Category(String::new()),
                self.market_view.category.is_empty(),
                cx,
            ));
        for category in categories {
            categories_row = categories_row.child(self.market_filter_choice(
                format!("market-category-{category}"),
                category.clone(),
                Filter::Category(category.clone()),
                self.market_view.category == category,
                cx,
            ));
        }
        let mut tags_row = h_flex()
            .flex_wrap()
            .gap_1()
            .child(self.market_filter_choice(
                "market-tag-all".into(),
                t!("plugins.market_all_tags").to_string(),
                Filter::Tag(String::new()),
                self.market_view.tag.is_empty(),
                cx,
            ));
        for tag in tags {
            tags_row = tags_row.child(self.market_filter_choice(
                format!("market-tag-{tag}"),
                tag.clone(),
                Filter::Tag(tag.clone()),
                self.market_view.tag == tag,
                cx,
            ));
        }
        let mut sorts = h_flex().flex_wrap().gap_1();
        for (id, label, sort) in [
            (
                "market-sort-relevance",
                t!("plugins.market_sort_relevance"),
                Sort::Relevance,
            ),
            (
                "market-sort-downloads",
                t!("plugins.market_sort_downloads"),
                Sort::Downloads,
            ),
            (
                "market-sort-updated",
                t!("plugins.market_sort_updated"),
                Sort::Updated,
            ),
            (
                "market-sort-name",
                t!("plugins.market_sort_name"),
                Sort::Name,
            ),
        ] {
            sorts = sorts.child(self.market_filter_choice(
                id.into(),
                label.to_string(),
                Filter::Sort(sort),
                self.market_view.sort == sort,
                cx,
            ));
        }
        let body = v_flex()
            .gap_2()
            .child(
                div()
                    .font_semibold()
                    .child(t!("plugins.market_categories").to_string()),
            )
            .child(categories_row)
            .child(
                div()
                    .font_semibold()
                    .child(t!("plugins.market_tags").to_string()),
            )
            .child(tags_row)
            .child(
                div()
                    .font_semibold()
                    .child(t!("plugins.market_sort").to_string()),
            )
            .child(sorts)
            .child(self.market_filter_choice(
                "market-clear-filters".into(),
                t!("plugins.market_clear_filters").to_string(),
                Filter::Clear,
                false,
                cx,
            ));
        popover("market-filter-panel", trigger, body.into_any_element()).into_any_element()
    }

    /// One callback keeps filtering, detail reset and withdrawal-history lifetime consistent.
    fn market_filter_choice(
        &self,
        id: String,
        label: String,
        filter: Filter,
        selected: bool,
        cx: &Context<Self>,
    ) -> Button {
        let selector = id.clone();
        Button::new(SharedString::from(id))
            .debug_selector(move || selector.clone())
            .small()
            .label(label)
            .when(selected, |button| button.primary())
            .on_click(cx.listener(move |this, _, _, cx| {
                match &filter {
                    Filter::Category(category) => this.market_view.category = category.clone(),
                    Filter::Tag(tag) => this.market_view.tag = tag.clone(),
                    Filter::Sort(sort) => this.market_view.sort = *sort,
                    Filter::Clear => {
                        this.market_view.category.clear();
                        this.market_view.tag.clear();
                        this.market_view.sort = Sort::Relevance;
                    }
                }
                this.market_view.leave_history();
                this.manager_selected = None;
                this.manager_detail_tab = management::DetailTab::Overview;
                this.manager_detail_scroll.set_offset(Default::default());
                cx.notify();
            }))
    }
}
