//! One background check at startup and a non-modal window for release information.

use photocraft_engine::updates_cmds::{InstallStatus, Status};
use serde_json::Value;

use crate::PhotocraftApp;

// Only complete packages published by the AI fork reach this window.
#[derive(Default)]
pub(crate) struct Runtime {
    startup_checked: bool,
    notified_version: Option<String>,
    auto_restart: bool,
}

pub(crate) fn check(app: &mut PhotocraftApp, _ctx: &egui::Context, params: Value) -> Result<Value, String> {
    let before = app.session.updates.status.clone();
    let result = app.run("help.checkForUpdates", params);
    // A check that fails to start still shows its failure and Check Again; the command error
    // stands. Only this call's failure: a Failed left from an earlier check doesn't open the
    // window for a call refused for another reason (bad params).
    let failed_to_start = match (&result, &app.session.updates.status) {
        (Err(error), status @ Status::Failed { message }) => *status != before || (!message.is_empty() && error.contains(message.as_str())),
        _ => false,
    };
    if result.is_ok() || failed_to_start {
        app.ui.updates_open = true;
        app.updates_rt.startup_checked = true;
    }
    result
}

pub(crate) fn tick(app: &mut PhotocraftApp, ctx: &egui::Context) {
    app.session.updates.poll_install();
    if matches!(app.session.updates.install_status, InstallStatus::Ready)
        && app.updates_rt.auto_restart
        && safe_to_restart(app)
        && !ctx.input(|i| i.pointer.any_down())
    {
        app.updates_rt.auto_restart = false;
        let _ = app.run("updates.restart", serde_json::json!({}));
    }
    if matches!(app.session.updates.install_status, InstallStatus::Restarting) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        return;
    }
    if matches!(app.session.updates.install_status, InstallStatus::Downloading) {
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }
    // `updates.status` also polls (automation may consume the result before this frame).
    app.session.updates.poll();
    if let Status::Available { release } = &app.session.updates.status
        && app.updates_rt.notified_version.as_ref() != Some(&release.version)
    {
        app.updates_rt.notified_version = Some(release.version.clone());
        app.ui.updates_open = true;
    }
    if app.session.updates.check.is_none() {
        return;
    }
    if !app.updates_rt.startup_checked {
        app.updates_rt.startup_checked = true;
        if app.session.prefs().general.automatically_check_updates && matches!(app.session.updates.status, Status::Idle) {
            let _ = app.run("help.checkForUpdates", serde_json::json!({}));
        }
    }
    if matches!(app.session.updates.status, Status::Checking) {
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }
}

pub(crate) fn show(app: &mut PhotocraftApp, ctx: &egui::Context) {
    let mut download: Option<String> = None;

    if !app.ui.updates_open {
        return;
    }
    let mut open = true;
    let status = app.session.updates.status.clone();
    let mut retry = false;
    let mut install = false;
    egui::Window::new(tl!("PhotoCraft AI Updates"))
        .id(egui::Id::unique("photocraft_updates"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(380.0)
        .show(ctx, |ui| {
            let t = crate::theme::Tokens::get(ctx);
            ui.label(crate::i18n::fmt(crate::i18n::t("Installed version: {version}"), &[("version", photocraft_engine::build_info::VERSION)]));
            ui.label(tl!("Updates are offered only after a new AI package is published."));
            ui.add_space(8.0);
            match &status {
                Status::Checking => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(tl!("Checking for updates…"));
                    });
                }
                Status::Available { release } => {
                    ui.label(crate::i18n::fmt(crate::i18n::t("PhotoCraft AI {version} is available."), &[("version", &release.version)]));
                    if ui.button(tl!("Release Notes")).clicked() {
                        download = Some(release.url.clone());
                    }
                }
                Status::UpToDate => {
                    ui.label(tl!("No newer AI package is available."));
                }
                Status::Failed { message } => {
                    ui.colored_label(t.warning, tl!("Could not check for updates."));
                    ui.label(message);
                }
                Status::Idle => {
                    ui.label(tl!("Updates have not been checked yet."));
                }
            }
            match &app.session.updates.install_status {
                InstallStatus::Downloading => {
                    ui.spinner();
                    ui.label(tl!("Downloading and verifying update…"));
                }
                InstallStatus::Failed { message } => {
                    ui.colored_label(t.warning, tl!("Could not install the update."));
                    ui.label(message);
                }
                _ => {}
            }
            let busy = matches!(app.session.updates.install_status, InstallStatus::Downloading | InstallStatus::Restarting);
            if app.session.updates.prepare.is_some() && matches!(status, Status::Available { .. }) && !busy {
                let saved = safe_to_restart(app);
                if !saved {
                    ui.label(tl!("Save your documents and wait for background tasks before updating."));
                }
                if ui.add_enabled(saved, egui::Button::new(tl!("Update and Restart"))).clicked() {
                    install = true;
                }
            }
            if ui.button(tl!("AI Project Page")).clicked() {
                download = Some("https://github.com/BLEZZ0102/photocraft-ai".into());
            }
            if !busy
                && !matches!(app.session.updates.install_status, InstallStatus::Ready)
                && !matches!(status, Status::Checking)
                && ui.button(tl!("Check Again")).clicked()
            {
                retry = true;
            }
        });
    app.ui.updates_open = open;
    if let Some(url) = download {
        crate::links::open(app, ctx, &url);
    }
    if retry {
        let _ = check(app, ctx, serde_json::json!({}));
    }
    if install {
        let id = if matches!(app.session.updates.install_status, InstallStatus::Ready) { "updates.restart" } else { "updates.install" };
        app.updates_rt.auto_restart = app.run(id, serde_json::json!({})).is_ok();
    }
}

pub(crate) fn safe_to_restart(app: &PhotocraftApp) -> bool {
    !app.has_unsaved_work()
        && !app.saving()
        && !app.printing()
        && app.session.jobs().is_empty()
        && app.ui.text_edit.is_none()
        && app.ui.transform.is_none()
        && app.ui.type_transform.is_none()
        && app.ui.pen.is_none()
        && app.ui.crop_rect.is_none()
        && app.ui.perspective_crop.is_none()
        && app.discard.is_none()
        && app.drag.is_none()
        && app.live_stroke.is_none()
        && !app.press_stroke
        && app.remove_pending.is_empty()
        && app.remove_trail.is_none()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Services, menus};
    use photocraft_engine::{Session, updates_cmds::Release};
    use std::sync::{Arc, Mutex, mpsc};

    type Sender = Arc<Mutex<Option<mpsc::Sender<photocraft_engine::updates_cmds::CheckResult>>>>;

    #[test]
    fn uncommitted_pointer_work_defers_restart() {
        let mut app = PhotocraftApp::new(Session::new(), Services::default());
        assert!(safe_to_restart(&app));
        app.press_stroke = true;
        assert!(!safe_to_restart(&app));
        app.press_stroke = false;
        app.remove_pending.push(serde_json::json!({"points": [[1.0, 1.0]]}));
        assert!(!safe_to_restart(&app));
        app.remove_pending.clear();
        assert!(safe_to_restart(&app));
    }

    #[test]
    fn newly_unsaved_work_pauses_restart_after_a_verified_download() {
        let count = Arc::new(Mutex::new(0));
        let scheduled = count.clone();
        let mut app = PhotocraftApp::new(
            Session::new(),
            Services {
                prepare_update: Some(Box::new(move |_| {
                    let (tx, rx) = mpsc::channel();
                    let count = scheduled.clone();
                    tx.send(Ok(Box::new(move || {
                        *count.lock().unwrap() += 1;
                        Ok(())
                    }) as photocraft_engine::updates_cmds::ReadyFn))
                        .unwrap();
                    Ok(rx)
                })),
                ..Default::default()
            },
        );
        app.session.updates.status = Status::Available { release: Release { version: "99.0.0".into(), url: photocraft_engine::updates_cmds::RELEASES.into() } };
        app.run("updates.install", serde_json::json!({})).unwrap();
        app.updates_rt.auto_restart = true;
        let ctx = egui::Context::default();
        let mut output = ctx.run_ui(
            egui::RawInput {
                time: Some(0.0),
                events: vec![egui::Event::PointerButton {
                    pos: egui::pos2(10.0, 10.0),
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                }],
                ..Default::default()
            },
            |ui| tick(&mut app, ui.ctx()),
        );
        output.textures_delta.clear();
        assert_eq!(*count.lock().unwrap(), 0, "a new pointer gesture in this frame prevents restart");
        app.session.execute("file.new", serde_json::json!({"width": 8, "height": 8})).unwrap();
        app.session.execute("layer.new.layer", serde_json::json!({})).unwrap();
        let mut output = ctx.run_ui(
            egui::RawInput {
                time: Some(0.5),
                events: vec![egui::Event::PointerButton {
                    pos: egui::pos2(10.0, 10.0),
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: Default::default(),
                }],
                ..Default::default()
            },
            |ui| tick(&mut app, ui.ctx()),
        );
        output.textures_delta.clear();
        assert_eq!(*count.lock().unwrap(), 0);
        assert!(matches!(app.session.updates.install_status, InstallStatus::Ready));
        app.session.execute("file.close", serde_json::json!({})).unwrap();
        at(&mut app, &ctx, 1.0);
        assert_eq!(*count.lock().unwrap(), 1);
        at(&mut app, &ctx, 2.0);
        assert_eq!(*count.lock().unwrap(), 1, "the helper is scheduled only once");
    }

    fn app() -> (PhotocraftApp, Arc<Mutex<u32>>, Sender) {
        let calls = Arc::new(Mutex::new(0));
        let count = calls.clone();
        let sender = Arc::new(Mutex::new(None));
        let tx = sender.clone();
        let app = PhotocraftApp::new(
            Session::new(),
            Services {
                check_updates: Some(Box::new(move || {
                    *count.lock().unwrap() += 1;
                    let (sender, receiver) = mpsc::channel();
                    *tx.lock().unwrap() = Some(sender);
                    Ok(receiver)
                })),
                ..Default::default()
            },
        );
        (app, calls, sender)
    }

    fn at(app: &mut PhotocraftApp, ctx: &egui::Context, time: f64) {
        let mut output = ctx.run_ui(egui::RawInput { time: Some(time), ..Default::default() }, |ui| tick(app, ui.ctx()));
        output.textures_delta.clear();
    }

    #[test]
    fn automatic_check_runs_once_per_launch_even_after_days_or_preference_changes() {
        let (mut app, calls, sender) = app();
        let ctx = egui::Context::default();
        at(&mut app, &ctx, 0.0);
        at(&mut app, &ctx, 1.0);
        at(&mut app, &ctx, 2.0);
        assert_eq!(*calls.lock().unwrap(), 1);
        sender.lock().unwrap().as_ref().unwrap().send(Ok(None)).unwrap();
        at(&mut app, &ctx, 3.0);
        assert!(!app.ui.updates_open, "automatic up-to-date checks stay quiet");
        at(&mut app, &ctx, 86400.0);
        app.session.edit_prefs(|p| p.general.automatically_check_updates = false);
        at(&mut app, &ctx, 86401.0);
        app.session.edit_prefs(|p| p.general.automatically_check_updates = true);
        at(&mut app, &ctx, 172800.0);
        assert_eq!(*calls.lock().unwrap(), 1, "no periodic checks or preference-triggered rechecks");
        let (mut restarted, restarted_calls, _) = self::app();
        at(&mut restarted, &ctx, 0.0);
        assert_eq!(*restarted_calls.lock().unwrap(), 1, "the next launch checks again");
    }

    #[test]
    fn startup_opt_out_and_failed_checks_do_not_retry_automatically() {
        let (mut disabled, calls, _) = app();
        let ctx = egui::Context::default();
        disabled.session.edit_prefs(|p| p.general.automatically_check_updates = false);
        at(&mut disabled, &ctx, 0.0);
        disabled.session.edit_prefs(|p| p.general.automatically_check_updates = true);
        at(&mut disabled, &ctx, 86400.0);
        assert_eq!(*calls.lock().unwrap(), 0, "opt-out is evaluated at startup");

        let (mut failed, calls, sender) = app();
        at(&mut failed, &ctx, 0.0);
        sender.lock().unwrap().as_ref().unwrap().send(Err("offline".into())).unwrap();
        at(&mut failed, &ctx, 1.0);
        at(&mut failed, &ctx, 172800.0);
        assert_eq!(*calls.lock().unwrap(), 1);
        assert!(!failed.ui.updates_open, "an automatic failure stays quiet");
    }

    #[test]
    fn manual_check_works_when_automatic_checks_are_off_and_opens_status() {
        let (mut app, calls, sender) = app();
        let ctx = egui::Context::default();
        app.session.edit_prefs(|p| p.general.automatically_check_updates = false);
        assert!(menus::is_enabled(&app, "help.checkForUpdates"));
        menus::invoke(&mut app, &ctx, "help.checkForUpdates", serde_json::json!({})).unwrap();
        assert!(app.ui.updates_open);
        assert_eq!(*calls.lock().unwrap(), 1);
        sender.lock().unwrap().as_ref().unwrap().send(Err("offline".into())).unwrap();
        at(&mut app, &ctx, 1.0);
        assert!(matches!(app.session.updates.status, Status::Failed { .. }));
        let mut web = PhotocraftApp::new(Session::new(), Services::default());
        assert!(!menus::is_enabled(&web, "help.checkForUpdates"));
        at(&mut web, &ctx, 2.0);
        assert_eq!(web.session.updates.status, Status::Idle);
    }

    #[test]
    fn available_release_notifies_once_even_when_automation_polls_first() {
        let (mut app, _, sender) = app();
        let ctx = egui::Context::default();
        at(&mut app, &ctx, 0.0);
        sender
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .send(Ok(Some(Release { version: "99.0.0".into(), url: "https://github.com/storytold/photocraft/releases/tag/v99.0.0".into() })))
            .unwrap();
        app.run("updates.status", serde_json::json!({})).unwrap();
        at(&mut app, &ctx, 1.0);
        assert!(app.ui.updates_open);
        app.ui.updates_open = false;
        at(&mut app, &ctx, 2.0);
        assert!(!app.ui.updates_open);
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn startup_update_notice_is_information_only() {
        let (mut app, _, sender) = app();
        let ctx = egui::Context::default();
        at(&mut app, &ctx, 0.0);
        sender
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .send(Ok(Some(Release { version: "99.0.0".into(), url: "https://github.com/storytold/photocraft/releases/tag/v99.0.0".into() })))
            .unwrap();
        at(&mut app, &ctx, 1.0);
        assert!(app.ui.updates_open, "upstream release opens the information window");

        // Rendering release information never starts an installer or closes the document.
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            show(&mut app, ui.ctx());
        });
        output.textures_delta.clear();
        assert!(app.ui.updates_open);
    }

    #[test]
    fn a_manual_check_that_fails_to_start_opens_the_status_window() {
        let mut app =
            PhotocraftApp::new(Session::new(), Services { check_updates: Some(Box::new(|| Err("Could not start worker".into()))), ..Default::default() });
        let ctx = egui::Context::default();
        let error = menus::invoke(&mut app, &ctx, "help.checkForUpdates", serde_json::json!({})).unwrap_err();
        assert!(error.contains("Could not start worker"), "{error}");
        assert!(matches!(app.session.updates.status, Status::Failed { .. }));
        assert!(app.ui.updates_open, "the failure and Check Again are shown");
        // Bad params are a caller error, not a failed check: nothing opens, also while the earlier
        // failure is still the status.
        app.ui.updates_open = false;
        assert!(menus::invoke(&mut app, &ctx, "help.checkForUpdates", serde_json::json!({"x": 1})).is_err());
        assert!(matches!(app.session.updates.status, Status::Failed { .. }), "the earlier failure stays");
        assert!(!app.ui.updates_open, "bad params after a failed check");
        app.session.updates.status = Status::Idle;
        assert!(menus::invoke(&mut app, &ctx, "help.checkForUpdates", serde_json::json!({"x": 1})).is_err());
        assert!(!app.ui.updates_open);
        // The same start failure again opens it again.
        app.session.updates.status = Status::Failed { message: "Could not start worker".into() };
        assert!(menus::invoke(&mut app, &ctx, "help.checkForUpdates", serde_json::json!({})).is_err());
        assert!(app.ui.updates_open, "a repeated start failure");
    }

    #[test]
    fn update_window_is_drivable_and_bad_state_changes_are_atomic() {
        let (mut app, _, _) = app();
        let ctx = egui::Context::default();
        let call = |app: &mut PhotocraftApp, method: &str, params: Value| {
            let (req, _) = crate::control::ControlRequest::new(method, params);
            match crate::control::handle(app, &ctx, &req) {
                crate::control::Outcome::Done(value) => value,
                _ => panic!("expected an immediate reply"),
            }
        };
        assert_eq!(call(&mut app, "ui.set", serde_json::json!({"updatesOpen": true}))["ok"], true);
        let inspected = call(&mut app, "ui.inspect", serde_json::json!({}));
        assert_eq!(inspected["result"]["updatesOpen"], true);
        assert_eq!(inspected["result"]["updates"]["supported"], true);
        assert_eq!(call(&mut app, "ui.set", serde_json::json!({"updatesOpen": false, "maskTarget": "bad"}))["ok"], false);
        assert!(app.ui.updates_open);
        assert_eq!(call(&mut app, "ui.set", serde_json::json!({"updatesOpen": false}))["ok"], true);
        assert!(!app.ui.updates_open);
    }
}
