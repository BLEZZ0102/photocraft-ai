//! AI updates preserve unsaved work and expose an explicit, localized installation action.
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use photocraft_engine::{
    Session,
    updates_cmds::{Release, Status},
};
use photocraft_ui_egui::{PhotocraftApp, Services};
use serde_json::json;
use std::sync::{Arc, Mutex};

#[test]
fn commands_queued_after_restart_cannot_create_unsaved_work() {
    let mut app = PhotocraftApp::new(Session::new(), Services::default());
    app.session.updates.install_status = photocraft_engine::updates_cmds::InstallStatus::Restarting;
    assert!(app.run("file.new", json!({"width": 8, "height": 8})).is_err());
    assert!(app.session.documents().is_empty());
    assert_eq!(app.run("updates.status", json!({})).unwrap()["installation"]["status"], "restarting");
}

#[test]
fn explicit_update_click_reports_preparation_failure_without_closing() {
    let calls = Arc::new(Mutex::new(0));
    let recorded = calls.clone();
    let mut h = Harness::builder().with_size(egui::vec2(800.0, 600.0)).build_eframe(move |cc| {
        PhotocraftApp::setup_context(&cc.egui_ctx, Default::default());
        let mut session = Session::new();
        session.edit_prefs(|p| p.interface.language = "en".into());
        let mut app = PhotocraftApp::new(
            session,
            Services {
                prepare_update: Some(Box::new(move |_| {
                    *recorded.lock().unwrap() += 1;
                    Err("Checksum mismatch".into())
                })),
                ..Default::default()
            },
        );
        app.session.updates.status = Status::Available { release: Release { version: "99.0.0".into(), url: photocraft_engine::updates_cmds::RELEASES.into() } };
        app
    });
    h.run_steps(4);
    assert_eq!(*calls.lock().unwrap(), 0);
    h.get_by_label("Update and Restart").click();
    h.run_steps(4);
    assert_eq!(*calls.lock().unwrap(), 1);
    assert!(h.query_by_label("Could not install the update.").is_some());
    assert!(h.query_all_by_label("Checksum mismatch").next().is_some());
    assert!(h.state().ui.updates_open);
}

#[test]
fn update_notice_preserves_unsaved_documents_and_localizes_every_language() {
    let opened = Arc::new(Mutex::new(Vec::new()));
    let recorded = opened.clone();
    let mut h = Harness::builder().with_size(egui::vec2(800.0, 600.0)).build_eframe(move |cc| {
        PhotocraftApp::setup_context(&cc.egui_ctx, Default::default());
        let mut session = Session::new();
        session.edit_prefs(|prefs| prefs.interface.language = "en".into());
        session.execute("file.new", json!({"width":16,"height":16})).unwrap();
        session.execute("layer.new.layer", json!({"name":"Unsaved test edit"})).unwrap();
        let links = recorded.clone();
        let services = Services {
            prepare_update: Some(Box::new(|_| Err("An unsaved document must prevent this call".into()))),
            open_url: Some(Box::new(move |url| {
                links.lock().unwrap().push(url.to_owned());
                Ok(())
            })),
            ..Default::default()
        };
        let mut app = PhotocraftApp::new(session, services);
        app.session.updates.status =
            Status::Available { release: Release { version: "99.0.0".into(), url: "https://github.com/storytold/photocraft/releases/tag/v99.0.0".into() } };
        app
    });
    h.run_steps(4);
    assert!(h.query_by_label("PhotoCraft AI Updates").is_some());
    assert!(h.get_by_label("Update and Restart").accesskit_node().is_disabled());
    assert!(h.query_by_label("Open Download Page").is_none());
    assert!(h.query_by_label("Release Notes").is_some());
    assert!(h.query_by_label("AI Project Page").is_some());
    assert!(h.query_by_label("Updates are offered only after a new AI package is published.").is_some());
    assert_eq!(h.state().session.active().unwrap().doc.layers.len(), 2);
    assert!(h.state().has_unsaved_work());
    for lang in photocraft_ui_egui::i18n::Lang::all() {
        use photocraft_ui_egui::i18n::{fmt, tr};
        h.state_mut().session.execute("prefs.set", json!({"path":"interface.language","value":lang.code()})).unwrap();
        h.run_steps(3);
        for key in [
            "PhotoCraft AI Updates",
            "AI Project Page",
            "Release Notes",
            "Update and Restart",
            "Save your documents and wait for background tasks before updating.",
        ] {
            assert!(h.query_by_label(tr(lang, key)).is_some(), "{}: {key}", lang.code());
        }
        let version = fmt(tr(lang, "PhotoCraft AI {version} is available."), &[("version", "99.0.0")]);
        assert!(h.query_by_label(&version).is_some(), "{}: {version}", lang.code());
    }
    h.state_mut().session.execute("prefs.set", json!({"path":"interface.language","value":"en"})).unwrap();
    h.run_steps(3);
    h.get_by_label("AI Project Page").click();
    h.run_steps(2);
    assert_eq!(h.state().session.active().unwrap().doc.layers.len(), 2);
    assert!(h.state().has_unsaved_work());
    assert!(h.state().ui.updates_open);
    assert_eq!(*opened.lock().unwrap(), ["https://github.com/BLEZZ0102/photocraft-ai"]);
}
