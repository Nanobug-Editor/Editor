//! Native confirmation input delivers each dialog's unchecked or explicit global choice to installation.

use super::*;
use gpui_kit::{TestAppContext, gpui};

/// Mouse, keyboard, locale, theme and scaling retain the left footer choice until confirmation.
#[gpui::test]
fn install_confirmation_global_option_is_unchecked_and_accepts_opt_in(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        typography::init(cx);
        cx.set_reduce_motion(true);
    });
    let previous_locale = rust_i18n::locale().to_string();
    for (dark, locale) in [(false, "zh-CN"), (true, "en")] {
        rust_i18n::set_locale(locale);
        cx.update(|cx| apply_theme(builtin_theme(dark), cx));
        for global_enabled in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let workspace = Workspace::open(directory.path()).unwrap();
            let slot = Rc::new(RefCell::new(None));
            let capture = slot.clone();
            let (_, view) = cx.add_window_view(move |window, cx| {
                let app = cx.new(|cx| EditorApp::new(workspace, None, window, cx));
                *capture.borrow_mut() = Some(app.clone());
                Root::new(app, window, cx)
            });
            let app = slot.borrow_mut().take().unwrap();
            let package = Package {
                manifest: test_manifest(&crate::test_packages::text("example", "manifest.json")),
                files: Default::default(),
                digest: "scope-consent".into(),
                source: Some("scope-consent.zip".into()),
            };
            view.simulate_resize(size(px(600.), px(440.)));
            view.simulate_scale_factor_change(1.5);
            view.update(|window, cx| {
                let panel = app.read(cx).extensions.clone();
                panel.update(cx, |panel, cx| {
                    panel.open_install_dialog(package, window, cx)
                });
            });
            view.run_until_parked();
            view.update(|window, cx| window.draw(cx).clear(cx));
            view.update(|window, cx| window.draw(cx).clear(cx));
            let option = view.debug_bounds("plugin-install-global-option").unwrap();
            let confirm = view.debug_bounds("plugin-install-confirm").unwrap();
            assert!(option.right() <= confirm.left());
            let viewport = view.update(|window, _| window.viewport_size());
            assert!(option.bottom() <= viewport.height);
            if global_enabled {
                // Click the mark, then toggle twice with Space to exercise Base's keyboard behavior.
                view.simulate_click(
                    point(option.left() + px(7.), option.center().y),
                    Default::default(),
                );
                view.simulate_keystrokes("space space");
            }
            view.simulate_click(confirm.center(), Default::default());
            assert!(view.update(|_, cx| {
                app.read(cx)
                    .extensions
                    .read(cx)
                    .worker
                    .recorded
                    .lock()
                    .unwrap()
                    .try_iter()
                    .any(|work| {
                        matches!(work, Work::Install(package, enabled)
                            if package.digest == "scope-consent" && enabled == global_enabled)
                    })
            }));
        }
    }
    rust_i18n::set_locale(&previous_locale);
}
