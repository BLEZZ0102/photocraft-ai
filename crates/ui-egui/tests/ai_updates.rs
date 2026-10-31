//! This fork may report upstream releases, but cannot install an official binary over AI work.
use egui_kittest::{Harness, kittest::Queryable};
use photocraft_engine::{
    Session,
    updates_cmds::{Release, Status},
};
use photocraft_ui_egui::{PhotocraftApp, Services};
use serde_json::json;
use std::sync::{Arc, Mutex};

#[test]
fn upstream_notice_offers_information_and_ai_project_without_installing_or_closing() {
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
    assert!(h.query_by_label("Update and Restart").is_none());
    assert!(h.query_by_label("Open Download Page").is_none());
    assert!(h.query_by_label("Release Notes").is_some());
    assert!(h.query_by_label("AI Project Page").is_some());
    assert!(
        h.query_by_label(
            "This AI build reports upstream releases. Official installers do not include its AI additions. Get AI build updates from the AI project."
        )
        .is_some()
    );
    assert_eq!(h.state().session.active().unwrap().doc.layers.len(), 2);
    assert!(h.state().has_unsaved_work());
    h.get_by_label("AI Project Page").click();
    h.run_steps(2);
    assert_eq!(h.state().session.active().unwrap().doc.layers.len(), 2);
    assert!(h.state().has_unsaved_work());
    assert!(h.state().ui.updates_open);
    assert_eq!(*opened.lock().unwrap(), ["https://github.com/BLEZZ0102/photocraft-ai"]);
}
