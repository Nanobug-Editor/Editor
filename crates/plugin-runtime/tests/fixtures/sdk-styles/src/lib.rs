//! Minimal public-SDK consumer isolating native presentation from example configuration and services.

use plugin_protocol::{
    FontStyle, api,
    bindings::{Guest, export},
    ui,
};
use std::cell::Cell;

thread_local! { static REVISION: Cell<u64> = const { Cell::new(0) }; }
struct StylesFixture;

impl Guest for StylesFixture {
    /// Publish only on ordinary lifecycle callbacks; no assets, documents or external effects are used.
    fn dispatch(payload: String) -> Result<String, String> {
        api::guest::dispatch(&payload, |input| {
            match input {
                api::Input::Prepare { .. } => REVISION.set(0),
                api::Input::Activate => {}
                api::Input::Snapshot => {
                    return Ok(api::Output {
                        snapshot: Some(Default::default()),
                        ..Default::default()
                    });
                }
                _ => return Ok(Default::default()),
            }
            let revision = REVISION.get() + 1;
            REVISION.set(revision);
            let mut node = ui::Node::text("prose", "Native content").role("prose");
            node.layout.align = Some(ui::Alignment::Center);
            let mut document = ui::Document::new(node).revision(revision);
            document.content_styles.insert(
                "prose".into(),
                ui::ContentStyle {
                    font: FontStyle {
                        size_px: Some(16.),
                        ..Default::default()
                    },
                    line_height: Some(1.5),
                    ..Default::default()
                },
            );
            Ok(api::Output {
                views: vec![api::View {
                    panel: "welcome".into(),
                    document,
                }],
                ..Default::default()
            })
        })
    }
}
export!(StylesFixture);
