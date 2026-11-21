//! Modal dialog for `edit.ai.codex` (Codex ChatGPT + configurable image API providers).
//!
//! Drawn outside `craft::ui::dialog::DialogHost` (like Keyboard Shortcuts and PhotoCraft AI
//! Updates) so multi-provider account, credential and layer-context controls stay independent of
//! upstream's `HostedParamDialog` migrations while remaining fully inspectable and drivable
//! through `ui.inspect` and `ui.dialog.{set,confirm,cancel}`.

use serde_json::{Map, Value, json};

use crate::PhotocraftApp;

/// Control-channel identifier reported in `ui.inspect().dialogs` and accepted by `ui.dialog.*`.
pub const CONTROL_ID: &str = "aiEdit";

/// Maximum prompt length in UTF-8 bytes accepted by the dialog and engine.
const MAX_PROMPT_BYTES: usize = 16_000;

/// Active AI Edit dialog state held on [`crate::state::UiState`].
#[derive(Clone, Debug, PartialEq)]
pub struct AiEditDialog {
    pub title: String,
    pub fields: Map<String, Value>,
}

/// Whether the AI Edit dialog is currently open.
pub fn is_open(app: &PhotocraftApp) -> bool {
    app.ui.ai_edit.is_some()
}

/// Open (or focus) the AI Edit dialog, returning its control identifier.
pub fn open(app: &mut PhotocraftApp) -> &'static str {
    if app.ui.ai_edit.is_some() {
        return CONTROL_ID;
    }
    crate::ai_account_ui::opened(app);
    let title = photocraft_engine::commands::find("edit.ai.codex").map_or("AI Edit…", |c| c.label).trim_end_matches('…').to_owned();
    let mut fields = Map::new();
    fields.insert("__command".into(), json!("edit.ai.codex"));
    fields.insert("__label".into(), json!("AI Edit…"));
    fields.insert("prompt".into(), json!(""));
    fields.insert("sendLayerContext".into(), json!(true));
    fields.insert("includeHiddenLayers".into(), json!(false));
    app.ui.ai_edit = Some(AiEditDialog { title, fields });
    CONTROL_ID
}

/// Set one field on the open AI Edit dialog (used by `ui.dialog.set`).
pub fn set_field(app: &mut PhotocraftApp, field: &str, value: Value) -> Result<(), String> {
    let Some(dialog) = app.ui.ai_edit.as_mut() else {
        return Err("AI Edit dialog is not open".into());
    };
    if field.starts_with("__") {
        return Err(format!("cannot edit internal field `{field}`"));
    }
    dialog.fields.insert(field.to_owned(), value);
    if field == "provider" {
        let provider = crate::ai_api_ui::selected(&dialog.fields).to_owned();
        crate::ai_account_ui::closed(app);
        if provider == "codex" {
            crate::ai_account_ui::opened(app);
        } else {
            crate::ai_api_ui::load(app, &provider);
        }
    }
    Ok(())
}

/// Confirm the open AI Edit dialog, validating account/provider/prompt state and running the
/// selected engine command.
pub fn confirm(app: &mut PhotocraftApp) -> Result<Value, String> {
    let Some(dialog) = app.ui.ai_edit.as_ref() else {
        return Err("AI Edit dialog is not open".into());
    };
    if dialog.fields.get("prompt").and_then(Value::as_str).is_some_and(|p| p.len() > MAX_PROMPT_BYTES) {
        return Err("prompt must contain 1–16000 bytes of text".into());
    }
    if !crate::ai_account_ui::can_confirm(app, &dialog.fields) {
        return Err("sign in with ChatGPT and enter a prompt first".into());
    }
    let dialog = app.ui.ai_edit.take().ok_or_else(|| "AI Edit dialog is not open".to_string())?;
    let mut params = Map::new();
    for (k, v) in &dialog.fields {
        if !k.starts_with("__") {
            params.insert(k.clone(), v.clone());
        }
    }
    let (cmd, params) = crate::ai_api_ui::edit_command(&dialog.fields, "edit.ai.codex".into(), Value::Object(params));
    let result = app.run(&cmd, params);
    crate::ai_api_ui::closed(app);
    result
}

/// Cancel the open AI Edit dialog without applying edits.
pub fn cancel(app: &mut PhotocraftApp) -> bool {
    if app.ui.ai_edit.take().is_some() {
        crate::ai_account_ui::closed(app);
        crate::ai_api_ui::closed(app);
        true
    } else {
        false
    }
}

/// Snapshot of the open AI Edit dialog for `ui.inspect().dialogs`.
pub fn inspect(app: &PhotocraftApp) -> Option<Value> {
    let dialog = app.ui.ai_edit.as_ref()?;
    Some(json!({
        "id": CONTROL_ID,
        "kind": "command",
        "title": dialog.title,
        "command": "edit.ai.codex",
        "fields": dialog.fields,
    }))
}

/// Prompt and layer-context controls shared by the Codex and API provider views.
pub fn prompt_body(ui: &mut egui::Ui, fields: &mut Map<String, Value>) {
    if crate::ai_api_ui::selected(fields) == "codex" {
        ui.label(crate::i18n::t(
            "The visible canvas and instruction will be sent to Codex using your ChatGPT sign-in. The result is fitted to the canvas and added as a new layer.",
        ));
        let mut layers = fields.get("sendLayerContext").and_then(Value::as_bool).unwrap_or(true);
        let mut hidden = fields.get("includeHiddenLayers").and_then(Value::as_bool).unwrap_or(false);
        ui.checkbox(&mut layers, tl!("Include individual layer images, names and structure"));
        ui.add_enabled_ui(layers, |ui| {
            ui.checkbox(&mut hidden, tl!("Also send hidden layers"));
        });
        if !layers {
            hidden = false;
        }
        if layers {
            ui.weak(tl!("Up to 15 layer images."));
            ui.add(egui::Label::new(egui::RichText::new(tl!("Isolated previews may reveal content covered by other layers or group masks.")).weak()).wrap());
        }
        fields.insert("sendLayerContext".into(), json!(layers));
        fields.insert("includeHiddenLayers".into(), json!(hidden));
    } else {
        ui.label(tl!("The result is fitted to the canvas and added as a new layer. Your existing layers are preserved."));
    }
    let mut prompt = fields.get("prompt").and_then(Value::as_str).unwrap_or_default().to_string();
    let response = ui.add(
        egui::TextEdit::multiline(&mut prompt).id_salt("ai-edit-prompt").desired_rows(4).desired_width(400.0).hint_text(crate::i18n::t("Describe your edit")),
    );
    if response.has_focus() {
        ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
    }
    fields.insert("prompt".into(), json!(prompt));
}

/// Draw the AI Edit modal window while open.
pub fn show(app: &mut PhotocraftApp, ctx: &egui::Context) {
    let Some(mut dialog) = app.ui.ai_edit.take() else {
        return;
    };
    let mut open = true;
    let mut do_confirm = false;
    let mut do_cancel = false;
    let id = egui::Id::unique("photocraft_ai_edit");
    let room = (ctx.content_rect().height() - 140.0).max(160.0);
    let title = tl!("AI Edit");

    egui::Window::new(title)
        .id(id)
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(460.0)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().id_salt(id.with("ai-body")).max_height(room).show(ui, |ui| {
                crate::ai_account_ui::body(app, ui, &mut dialog.fields);
            });
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(6.0);
            let can_ok = crate::ai_account_ui::can_confirm(app, &dialog.fields);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add_enabled(can_ok, egui::Button::new(tl!("OK")).min_size(egui::vec2(84.0, 26.0))).clicked() {
                    do_confirm = true;
                }
                if ui.add(egui::Button::new(tl!("Cancel")).min_size(egui::vec2(84.0, 26.0))).clicked() {
                    do_cancel = true;
                }
            });
            if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
                do_cancel = true;
            }
        });

    app.ui.ai_edit = Some(dialog);
    if do_confirm {
        let _ = confirm(app);
    } else if do_cancel || !open {
        cancel(app);
    }
}
