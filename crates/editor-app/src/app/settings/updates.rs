//! About and update controls render the shared application model using native local components.

use super::*;
use app::updates::{Phase, Updater, release::Edition};
use std::sync::atomic::Ordering;

/// Project menus defer this call until their editor's entity update lease has ended.
pub(in crate::app) fn open_project(view: Entity<EditorApp>, cx: &mut App) {
    open(SettingsOwner::Project(view), cx);
}

/// Empty-window menus enter the same settings category without constructing a workspace.
pub(in crate::app) fn open_empty(view: Entity<app::projects::ProjectWindow>, cx: &mut App) {
    open(SettingsOwner::Empty(view), cx);
}

/// Activate an existing settings window or retain a normal native settings dialog for this owner.
fn open(owner: SettingsOwner, cx: &mut App) {
    owner.select_section(SettingsSection::Updates, cx);
    if let Ok(profile) = owner.profile(cx) {
        app::updates::ensure(&profile, cx).update(cx, |updater, cx| {
            if updater.phase == Phase::Idle && updater.available.is_none() {
                updater.check(cx);
            }
        });
    }
    if let Some(handle) = owner.dialog_window(cx)
        && handle
            .update(cx, |_, window, _| window.activate_window())
            .is_ok()
    {
        return;
    }
    let content_owner = owner.clone();
    let (dialog, handle) = app_dialog::open_dialog(
        t!("settings.title").to_string(),
        move |content, window, cx| render_settings_content(&content_owner, content, window, cx),
        cx,
    );
    owner.retain_dialog(dialog, handle, cx);
}

/// Resolve the explicit profile once; render state without initiating discovery from a redraw.
pub(super) fn render(owner: &SettingsOwner, _: &mut Window, cx: &mut App) -> AnyElement {
    let profile = match owner.profile(cx) {
        Ok(profile) => profile,
        Err(error) => return div().child(error.to_string()).into_any_element(),
    };
    let service = app::updates::ensure(&profile, cx);
    let updater = service.read(cx);
    let busy = updater.busy();
    let automatic = updater.preferences.automatic;
    let preview = updater.preferences.preview;
    let phase = updater.phase;
    let edition = updater.identity.edition;
    let version = updater.identity.version.to_string();
    let available = updater.available.clone();
    let error = updater.error.clone();
    let status = match phase {
        Phase::Loading => t!("updates.loading").to_string(),
        Phase::Checking => t!("updates.checking").to_string(),
        Phase::Downloading => {
            let total = available
                .as_ref()
                .and_then(|release| release.asset.as_ref())
                .map_or(1, |asset| asset.size.max(1));
            let percent = updater.received.load(Ordering::Relaxed).saturating_mul(100) / total;
            t!("updates.downloading", percent = percent.min(100)).to_string()
        }
        Phase::Ready => t!("updates.ready").to_string(),
        Phase::Installing => t!("updates.exiting").to_string(),
        Phase::Idle if available.is_some() => t!(
            "updates.available",
            version = available.as_ref().unwrap().version.clone()
        )
        .to_string(),
        Phase::Idle if updater.checked => t!("updates.up_to_date").to_string(),
        Phase::Idle => t!("updates.not_checked").to_string(),
    };
    let check = service.clone();
    let policy = service.clone();
    let channel = service.clone();
    let action = service.clone();
    let cancel = service.clone();
    v_flex()
        .gap_4()
        .w_full()
        .min_w_0()
        .child(div().text_xl().font_semibold().child(app::APP_NAME))
        .child(div().child(t!("updates.current_version", version = version).to_string()))
        .child(
            div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(match edition {
                    Edition::Installed => t!("updates.edition_installed").to_string(),
                    Edition::Portable => t!("updates.edition_portable").to_string(),
                    Edition::Browse => t!("updates.edition_development").to_string(),
                }),
        )
        .child(
            div()
                .id("host-update-status")
                .debug_selector(|| "host-update-status".into())
                .child(status),
        )
        .when_some(error, |page, error| {
            page.child(
                div()
                    .id("host-update-error")
                    .text_color(cx.theme().danger)
                    .child(t!("updates.error", detail = error).to_string()),
            )
        })
        .child(
            h_flex()
                .gap_2()
                .flex_wrap()
                .child(
                    Button::new("host-update-check")
                        .debug_selector(|| "host-update-check".into())
                        .label(t!("updates.check").to_string())
                        .disabled(busy)
                        .on_click(move |_, _, cx| {
                            check.update(cx, |updater, cx| updater.check(cx))
                        }),
                )
                .when(phase == Phase::Downloading, |row| {
                    row.child(
                        Button::new("host-update-cancel")
                            .debug_selector(|| "host-update-cancel".into())
                            .label(t!("updates.cancel").to_string())
                            .on_click(move |_, _, cx| {
                                cancel.update(cx, |updater, _| updater.cancel_download())
                            }),
                    )
                })
                .when(available.is_some() && phase != Phase::Downloading, |row| {
                    row.child(primary_action(
                        action,
                        phase,
                        edition,
                        busy,
                        available
                            .as_ref()
                            .is_some_and(|release| release.asset.is_some()),
                    ))
                }),
        )
        .child(
            Checkbox::new("host-update-automatic")
                .label(t!("updates.automatic").to_string())
                .checked(automatic)
                .disabled(busy)
                .on_change(move |enabled, _, cx| {
                    policy.update(cx, |updater, cx| updater.configure(*enabled, preview, cx))
                }),
        )
        .child(
            Checkbox::new("host-update-preview")
                .label(t!("updates.preview").to_string())
                .checked(preview)
                .disabled(busy)
                .on_change(move |enabled, _, cx| {
                    channel.update(cx, |updater, cx| updater.configure(automatic, *enabled, cx))
                }),
        )
        .child(
            div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(t!("updates.policy_hint").to_string()),
        )
        .when_some(available, |page, release| {
            let address = release.url.clone();
            page.child(
                div()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .pt_4()
                    .child(
                        v_flex()
                            .gap_3()
                            .min_w_0()
                            .child(div().font_semibold().child(release.title))
                            .when(!release.published_at.is_empty(), |column| {
                                column.child(div().text_sm().child(release.published_at))
                            })
                            .child(
                                Button::new("host-update-release-page")
                                    .label(t!("updates.release_page").to_string())
                                    .on_click(move |_, _, cx| cx.open_url(&address)),
                            )
                            .child(
                                div()
                                    .id("host-update-notes")
                                    .debug_selector(|| "host-update-notes".into())
                                    .text_sm()
                                    .child(if release.notes.is_empty() {
                                        t!("updates.no_notes").to_string()
                                    } else {
                                        release.notes
                                    }),
                            ),
                    ),
            )
        })
        .into_any_element()
}

/// The ready action closes its modal before all-root close consent is requested.
fn primary_action(
    service: Entity<Updater>,
    phase: Phase,
    edition: Edition,
    busy: bool,
    has_asset: bool,
) -> Button {
    let label = if phase == Phase::Ready {
        t!("updates.restart").to_string()
    } else if edition == Edition::Browse || !has_asset {
        t!("updates.release_page").to_string()
    } else {
        t!("updates.download").to_string()
    };
    Button::new("host-update-primary")
        .debug_selector(|| "host-update-primary".into())
        .label(label)
        .primary()
        .disabled(busy)
        .on_click(move |_, window, cx| {
            if phase == Phase::Ready {
                service.update(cx, |updater, cx| updater.restart(cx));
                window.remove_window();
            } else if !has_asset {
                if let Some(release) = &service.read(cx).available {
                    cx.open_url(&release.url);
                }
            } else {
                service.update(cx, |updater, cx| updater.download(cx));
            }
        })
}
