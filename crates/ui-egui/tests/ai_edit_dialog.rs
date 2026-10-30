//! The menu opens a real command form without starting a provider request.
use photocraft_ui_egui::{PhotocraftApp, Services};
use serde_json::json;

#[test]
fn ai_edit_opens_prompt_form() {
    let mut session = photocraft_engine::Session::new();
    session.execute("file.new", json!({"width":16,"height":16})).unwrap();
    let mut app = PhotocraftApp::new(session, Services::default());
    let items = photocraft_ui_egui::menus::menu_items(&app);
    let fill = items.iter().position(|item| item.id == "edit.contentAwareFill").unwrap();
    let ai = &items[fill + 1];
    assert_eq!(ai.id, "edit.ai.codex");
    assert_eq!(ai.path, ["Edit"]);
    assert!(ai.enabled);
    let ctx = egui::Context::default();
    let result = photocraft_ui_egui::menus::invoke(&mut app, &ctx, "edit.ai.codex", json!({})).unwrap();
    assert!(result["dialog"].is_u64());
    assert_eq!(app.session.active().unwrap().doc.layers.len(), 1);
}

#[test]
fn ai_edit_requires_an_open_document() {
    let app = PhotocraftApp::new(photocraft_engine::Session::new(), Services::default());
    assert!(!photocraft_ui_egui::menus::is_enabled(&app, "edit.ai.codex"));
}
