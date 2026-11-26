//! The menu opens a real command form without starting a provider request.
use photocraft_ui_egui::{PhotocraftApp, Services};
use serde_json::json;

#[test]
fn ai_edit_opens_prompt_form() {
    let mut session = photocraft_engine::Session::new();
    session.execute("file.new", json!({"width":16,"height":16})).unwrap();
    let mut app = PhotocraftApp::new(session, Services::default());
    let items = photocraft_ui_egui::menus::menu_rows(&app);
    let fill = items.iter().position(|item| item.command.id == "edit.contentAwareFill").unwrap();
    let ai = &items[fill + 1];
    assert_eq!(ai.command.id, "edit.ai.codex");
    assert_eq!(ai.path, ["Edit"]);
    assert!(ai.enabled);
    let ctx = app.ctx().clone();
    let result = photocraft_ui_egui::menus::invoke(&mut app, &ctx, "edit.ai.codex", json!({})).unwrap();
    assert_eq!(result["dialog"], photocraft_ui_egui::ai_edit_ui::CONTROL_ID);
    assert!(app.ui.ai_edit.is_some());
    assert_eq!(app.session.active().unwrap().doc.layers.len(), 1);
}

#[test]
fn ai_edit_requires_an_open_document() {
    let app = PhotocraftApp::new(photocraft_engine::Session::new(), Services::default());
    assert!(!photocraft_ui_egui::menus::is_enabled(&app, "edit.ai.codex"));
}

#[cfg(not(target_arch = "wasm32"))]
mod account_flow {
    use super::*;
    use egui_kittest::{
        Harness,
        kittest::{NodeT, Queryable},
    };
    use photocraft_engine::cmd::ai_account_cmds::{AccountError, AccountInfo, AccountService, State};
    use std::sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    };
    use std::time::{Duration, Instant};

    struct Fake {
        info: Mutex<Option<AccountInfo>>,
        finish: AtomicBool,
        opened: Mutex<Vec<String>>,
        fail_browser: AtomicBool,
        runtime_missing: AtomicBool,
        hold_cancel: AtomicBool,
        checks: AtomicUsize,
    }
    impl AccountService for Fake {
        fn check(&self, _: &dyn Fn() -> bool) -> Result<AccountInfo, AccountError> {
            self.checks.fetch_add(1, Ordering::AcqRel);
            if self.runtime_missing.load(Ordering::Acquire) {
                return Err(AccountError::RuntimeUnavailable("Synthetic missing runtime".into()));
            }
            self.info.lock().unwrap_or_else(PoisonError::into_inner).clone().ok_or_else(|| AccountError::Provider("Expired session".into()))
        }
        fn login(&self, url: &mut dyn FnMut(String) -> Result<(), AccountError>, cancel: &dyn Fn() -> bool) -> Result<AccountInfo, AccountError> {
            url("https://auth.openai.com/authorize?state=test".into())?;
            while !cancel() && !self.finish.load(Ordering::Acquire) {
                std::thread::sleep(Duration::from_millis(1));
            }
            // Model a bounded runtime that needs time to stop after cancellation.
            let deadline = Instant::now() + Duration::from_secs(3);
            while cancel() && self.hold_cancel.load(Ordering::Acquire) && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(1));
            }
            if cancel() { Err(AccountError::Cancelled) } else { Ok(AccountInfo::ChatGpt { plan: Some("plus".into()) }) }
        }
    }

    fn harness(info: AccountInfo) -> (Harness<'static, Box<PhotocraftApp>>, Arc<Fake>) {
        let fake = Arc::new(Fake {
            info: Mutex::new(Some(info)),
            finish: AtomicBool::new(false),
            opened: Mutex::new(Vec::new()),
            fail_browser: AtomicBool::new(false),
            runtime_missing: AtomicBool::new(false),
            hold_cancel: AtomicBool::new(false),
            checks: AtomicUsize::new(0),
        });
        let service = fake.clone();
        let mut h = Harness::builder().with_size(egui::vec2(1280.0, 900.0)).with_max_steps(64).build_eframe(move |cc| {
            PhotocraftApp::setup_context(&cc.egui_ctx, Default::default());
            let mut session = photocraft_engine::Session::new();
            session.edit_prefs(|p| p.interface.language = "en".into());
            assert!(session.execute("file.new", json!({"width":16,"height":16})).is_ok());
            // Even a broken confirmation path must never reach a real provider in this harness.
            session.authorize = Some(|command, _| {
                if command == "edit.ai.codex" {
                    Err(photocraft_engine::EngineError::Other("Offline UI test: image requests are forbidden".into()))
                } else {
                    Ok(())
                }
            });
            let browser = service.clone();
            assert!(session.ai_account.set_service(service).is_ok());
            let ctx = cc.egui_ctx.clone();
            let services = Services {
                open_url: Some(Box::new(move |url| {
                    browser.opened.lock().unwrap_or_else(PoisonError::into_inner).push(url.to_owned());
                    ctx.data_mut(|data| data.insert_temp(egui::Id::unique("native-auth-browser-called"), true));
                    if browser.fail_browser.load(Ordering::Acquire) { Err("Synthetic browser failure".into()) } else { Ok(()) }
                })),
                ..Default::default()
            };
            PhotocraftApp::with_context(cc.egui_ctx.clone(), session, services)
        });
        h.run_steps(3);
        let ctx = h.ctx.clone();
        assert!(photocraft_ui_egui::menus::invoke(h.state_mut(), &ctx, "edit.ai.codex", json!({})).is_ok());
        (h, fake)
    }

    fn wait(h: &mut Harness<'_, Box<PhotocraftApp>>, expected: State) -> bool {
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut browser = false;
        while Instant::now() < deadline {
            h.run_steps(1);
            browser |= h.ctx.data(|data| data.get_temp::<bool>(egui::Id::unique("native-auth-browser-called"))).unwrap_or(false);
            if h.state().ui.ai_account.status.state == expected {
                // Egui measures a newly opened modal before exposing its widgets.
                h.run_steps(2);
                return browser;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(h.state().ui.ai_account.status.state, expected);
        browser
    }

    #[test]
    fn signed_out_login_opens_browser_only_after_click_and_cancel_preserves_canvas() {
        let (mut h, fake) = harness(AccountInfo::SignedOut);
        assert!(!wait(&mut h, State::SignedOut), "checking must not open a browser");
        assert!(h.get_by_label("OK").accesskit_node().is_disabled());
        h.get_by_label("Sign in with ChatGPT").click();
        assert!(wait(&mut h, State::WaitingForBrowser));
        assert_eq!(fake.opened.lock().unwrap_or_else(PoisonError::into_inner).len(), 1, "use the native browser service exactly once");
        assert!(!h.state().ui.status.contains("authorize"), "auth URL must not enter status history");
        let serialized = serde_json::to_string(&h.state().ui.ai_account).unwrap();
        assert!(!serialized.contains("authorize") && !serialized.contains("state=test"));
        h.get_by_label("Cancel sign-in").click();
        wait(&mut h, State::Cancelled);
        assert_eq!(h.state().session.active().unwrap().doc.layers.len(), 1);
        assert!(h.get_by_label("OK").accesskit_node().is_disabled());
    }

    #[test]
    fn completion_enables_edit_only_with_prompt_and_expired_refresh_disables_it() {
        let (mut h, fake) = harness(AccountInfo::SignedOut);
        wait(&mut h, State::SignedOut);
        h.get_by_label("Sign in with ChatGPT").click();
        assert!(wait(&mut h, State::WaitingForBrowser));
        fake.finish.store(true, Ordering::Release);
        wait(&mut h, State::Connected);
        assert!(h.get_by_label("OK").accesskit_node().is_disabled(), "empty prompt");
        h.state_mut().ui.ai_edit.as_mut().unwrap().fields.insert("prompt".into(), json!("Make the circle yellow"));
        h.run_steps(2);
        assert!(!h.get_by_label("OK").accesskit_node().is_disabled());
        *fake.info.lock().unwrap_or_else(PoisonError::into_inner) = None;
        h.get_by_label("Check connection").click();
        wait(&mut h, State::Failed);
        assert!(h.get_by_label("OK").accesskit_node().is_disabled());
        assert!(h.query_by_label("Sign in with ChatGPT").is_some());
        assert_eq!(h.state().session.active().unwrap().doc.layers.len(), 1);
    }

    #[test]
    fn closing_the_form_cancels_the_pending_browser_login() {
        let (mut h, _) = harness(AccountInfo::SignedOut);
        wait(&mut h, State::SignedOut);
        h.get_by_label("Sign in with ChatGPT").click();
        wait(&mut h, State::WaitingForBrowser);
        h.get_by_label("Cancel").click();
        wait(&mut h, State::Cancelled);
        assert!(h.state().ui.ai_edit.is_none());
        assert!(!h.state().session.ai_account.pending());
    }

    #[test]
    fn existing_subscription_needs_no_new_authorization_and_api_login_is_refused() {
        for info in [AccountInfo::ChatGpt { plan: Some("pro".into()) }, AccountInfo::Unsupported] {
            let expected = if matches!(info, AccountInfo::Unsupported) { State::Unsupported } else { State::Connected };
            let (mut h, _) = harness(info);
            assert!(!wait(&mut h, expected.clone()));
            assert_eq!(h.query_by_label("Sign in with ChatGPT").is_some(), expected == State::Unsupported);
        }
    }

    #[test]
    fn missing_runtime_guides_install_and_recheck_without_requesting_login() {
        let (mut h, fake) = harness(AccountInfo::ChatGpt { plan: None });
        wait(&mut h, State::Connected);
        fake.runtime_missing.store(true, Ordering::Release);
        h.get_by_label("Check connection").click();
        assert!(!wait(&mut h, State::RuntimeUnavailable));
        assert!(h.query_by_label("Codex was not found. Install and open the Codex desktop app, then check the connection again.").is_some());
        assert!(h.query_by_label("Sign in with ChatGPT").is_none());
        assert!(h.get_by_label("OK").accesskit_node().is_disabled());
        fake.runtime_missing.store(false, Ordering::Release);
        h.get_by_label("Check connection").click();
        assert!(!wait(&mut h, State::Connected));
        assert!(fake.opened.lock().unwrap_or_else(PoisonError::into_inner).is_empty());
        assert_eq!(h.state().session.active().unwrap().doc.layers.len(), 1);
    }

    #[test]
    fn native_browser_failure_is_visible_and_user_can_copy_link_or_retry() {
        let (mut h, fake) = harness(AccountInfo::SignedOut);
        wait(&mut h, State::SignedOut);
        fake.fail_browser.store(true, Ordering::Release);
        h.get_by_label("Sign in with ChatGPT").click();
        assert!(wait(&mut h, State::WaitingForBrowser));
        assert!(h.query_by_label("Could not open your browser. Try again or copy the sign-in link into your browser.").is_some());
        h.get_by_label("Copy sign-in link").click();
        h.run_steps(1);
        assert!(
            h.output()
                .platform_output
                .commands
                .iter()
                .any(|command| matches!(command, egui::OutputCommand::CopyText(text) if text.starts_with("https://auth.openai.com/")))
        );
        fake.fail_browser.store(false, Ordering::Release);
        h.get_by_label("Open sign-in page").click();
        h.run_steps(2);
        assert!(h.query_by_label("Could not open your browser. Try again or copy the sign-in link into your browser.").is_none());
        assert_eq!(fake.opened.lock().unwrap_or_else(PoisonError::into_inner).len(), 2);
    }

    #[test]
    fn missing_native_browser_service_reports_failure_without_emitting_an_open_url() {
        let (mut h, _) = harness(AccountInfo::SignedOut);
        wait(&mut h, State::SignedOut);
        h.state_mut().services.open_url = None;
        h.get_by_label("Sign in with ChatGPT").click();
        wait(&mut h, State::WaitingForBrowser);
        assert!(h.query_by_label("Could not open your browser. Try again or copy the sign-in link into your browser.").is_some());
        assert!(!h.output().platform_output.commands.iter().any(|cmd| matches!(cmd, egui::OutputCommand::OpenUrl(_))));
        h.get_by_label("Copy sign-in link").click();
        h.run_steps(1);
        assert!(
            h.output()
                .platform_output
                .commands
                .iter()
                .any(|cmd| matches!(cmd, egui::OutputCommand::CopyText(text) if text.starts_with("https://auth.openai.com/")))
        );
    }

    #[test]
    fn reopening_during_uncancelled_login_keeps_its_success_without_refreshing() {
        let (mut h, fake) = harness(AccountInfo::SignedOut);
        wait(&mut h, State::SignedOut);
        h.get_by_label("Sign in with ChatGPT").click();
        wait(&mut h, State::WaitingForBrowser);
        photocraft_ui_egui::ai_account_ui::opened(h.state_mut());
        *fake.info.lock().unwrap_or_else(PoisonError::into_inner) = None;
        fake.finish.store(true, Ordering::Release);
        wait(&mut h, State::Connected);
        h.run_steps(4);
        assert_eq!(h.state().ui.ai_account.status.state, State::Connected);
        assert_eq!(fake.checks.load(Ordering::Acquire), 1, "successful pending login needs no extra check");
    }

    #[test]
    fn oversized_prompt_reports_byte_limit_and_keeps_dialog_and_document() {
        let (mut h, _) = harness(AccountInfo::ChatGpt { plan: None });
        wait(&mut h, State::Connected);
        for prompt in ["x".repeat(16_001), "字".repeat(5_334)] {
            h.state_mut().ui.ai_edit.as_mut().unwrap().fields.insert("prompt".into(), json!(prompt));
            let error = photocraft_ui_egui::ai_edit_ui::confirm(h.state_mut()).unwrap_err();
            assert_eq!(error, "prompt must contain 1–16000 bytes of text");
            assert!(h.state().ui.ai_edit.is_some());
            assert_eq!(h.state().session.active().unwrap().doc.layers.len(), 1);
        }
        h.state_mut().ui.ai_edit.as_mut().unwrap().fields.insert("prompt".into(), json!("x".repeat(16_000)));
        let error = photocraft_ui_egui::ai_edit_ui::confirm(h.state_mut()).unwrap_err();
        assert!(error.contains("Offline UI test: image requests are forbidden"), "boundary prompt reaches command authorization");
    }

    #[test]
    fn reopening_during_cancel_rechecks_once_when_worker_finishes() {
        let (mut h, fake) = harness(AccountInfo::SignedOut);
        wait(&mut h, State::SignedOut);
        fake.hold_cancel.store(true, Ordering::Release);
        h.get_by_label("Sign in with ChatGPT").click();
        wait(&mut h, State::WaitingForBrowser);
        h.get_by_label("Cancel").click();
        h.run_steps(1);
        assert!(h.state().session.ai_account.pending());
        *fake.info.lock().unwrap_or_else(PoisonError::into_inner) = Some(AccountInfo::ChatGpt { plan: None });
        let ctx = h.ctx.clone();
        photocraft_ui_egui::menus::invoke(h.state_mut(), &ctx, "edit.ai.codex", json!({})).unwrap();
        h.run_steps(2);
        assert_eq!(fake.checks.load(Ordering::Acquire), 1);
        fake.hold_cancel.store(false, Ordering::Release);
        wait(&mut h, State::Connected);
        h.run_steps(4);
        assert_eq!(fake.checks.load(Ordering::Acquire), 2);
        assert_eq!(fake.opened.lock().unwrap_or_else(PoisonError::into_inner).len(), 1);
    }

    #[test]
    fn closing_reopened_dialog_clears_deferred_refresh() {
        let (mut h, fake) = harness(AccountInfo::SignedOut);
        wait(&mut h, State::SignedOut);
        fake.hold_cancel.store(true, Ordering::Release);
        h.get_by_label("Sign in with ChatGPT").click();
        wait(&mut h, State::WaitingForBrowser);
        assert!(photocraft_ui_egui::ai_edit_ui::cancel(h.state_mut()));
        let ctx = h.ctx.clone();
        photocraft_ui_egui::menus::invoke(h.state_mut(), &ctx, "edit.ai.codex", json!({})).unwrap();
        assert!(photocraft_ui_egui::ai_edit_ui::cancel(h.state_mut()));
        fake.hold_cancel.store(false, Ordering::Release);
        wait(&mut h, State::Cancelled);
        assert_eq!(fake.checks.load(Ordering::Acquire), 1);
        assert!(h.state().ui.ai_edit.is_none());
    }

    #[test]
    fn control_confirm_during_login_keeps_dialog_and_pending_worker() {
        let (mut h, _) = harness(AccountInfo::SignedOut);
        wait(&mut h, State::SignedOut);
        h.get_by_label("Sign in with ChatGPT").click();
        wait(&mut h, State::WaitingForBrowser);
        h.state_mut().ui.ai_edit.as_mut().unwrap().fields.insert("prompt".into(), json!("Make the circle yellow"));
        assert!(photocraft_ui_egui::ai_edit_ui::confirm(h.state_mut()).is_err());
        assert!(h.state().ui.ai_edit.is_some());
        assert!(h.state().session.ai_account.pending());
        assert_eq!(h.state().session.active().unwrap().doc.layers.len(), 1);
    }

    #[test]
    fn enter_in_prompt_inserts_newline_without_confirming() {
        let (mut h, _) = harness(AccountInfo::ChatGpt { plan: None });
        wait(&mut h, State::Connected);
        h.get_by_role(egui::accesskit::Role::MultilineTextInput).click();
        h.event(egui::Event::Text("Make the circle yellow".into()));
        h.run_steps(2);
        assert!(!h.get_by_label("OK").accesskit_node().is_disabled());
        h.key_press(egui::Key::Enter);
        h.run_steps(2);
        assert!(h.state().ui.ai_edit.is_some(), "Enter must not submit the canvas");
        assert!(h.state().ui.ai_edit.as_ref().unwrap().fields["prompt"].as_str().unwrap().contains('\n'));
        assert_eq!(h.state().session.active().unwrap().doc.layers.len(), 1);
    }

    #[test]
    fn control_confirm_rejects_stale_connected_ui_while_refresh_is_pending() {
        let (mut h, _) = harness(AccountInfo::ChatGpt { plan: None });
        wait(&mut h, State::Connected);
        h.state_mut().ui.ai_edit.as_mut().unwrap().fields.insert("prompt".into(), json!("Make the circle yellow"));
        h.state_mut().run("ai.codex.account", json!({"action":"refresh"})).unwrap();
        assert_eq!(h.state().ui.ai_account.status.state, State::Connected);
        assert!(photocraft_ui_egui::ai_edit_ui::confirm(h.state_mut()).is_err());
        assert!(h.state().ui.ai_edit.is_some());
        assert_eq!(h.state().session.active().unwrap().doc.layers.len(), 1);
    }

    #[test]
    fn ready_confirmation_still_closes_dialog_and_reaches_the_command_gate() {
        let (mut h, _) = harness(AccountInfo::ChatGpt { plan: None });
        wait(&mut h, State::Connected);
        h.state_mut().ui.ai_edit.as_mut().unwrap().fields.insert("prompt".into(), json!("Make the circle yellow"));
        let error = photocraft_ui_egui::ai_edit_ui::confirm(h.state_mut()).unwrap_err();
        assert!(error.contains("Offline UI test: image requests are forbidden"));
        assert!(h.state().ui.ai_edit.is_none());
        assert_eq!(h.state().session.active().unwrap().doc.layers.len(), 1);
    }

    #[test]
    fn hidden_layer_sharing_requires_a_click_and_resets_for_a_new_edit() {
        let (mut h, _) = harness(AccountInfo::ChatGpt { plan: None });
        wait(&mut h, State::Connected);
        assert_eq!(h.state().ui.ai_edit.as_ref().unwrap().fields["sendLayerContext"], true);
        assert_eq!(h.state().ui.ai_edit.as_ref().unwrap().fields["includeHiddenLayers"], false);
        h.get_by_label("Also send hidden layers").click();
        h.run_steps(2);
        assert_eq!(h.state().ui.ai_edit.as_ref().unwrap().fields["includeHiddenLayers"], true);
        h.get_by_label("Include individual layer images, names and structure").click();
        h.run_steps(2);
        assert_eq!(h.state().ui.ai_edit.as_ref().unwrap().fields["sendLayerContext"], false);
        assert_eq!(h.state().ui.ai_edit.as_ref().unwrap().fields["includeHiddenLayers"], false);
        assert!(h.get_by_label("Also send hidden layers").accesskit_node().is_disabled());
        h.get_by_label("Cancel").click();
        h.run_steps(2);
        let ctx = h.ctx.clone();
        photocraft_ui_egui::menus::invoke(h.state_mut(), &ctx, "edit.ai.codex", json!({})).unwrap();
        h.run_steps(3);
        assert_eq!(h.state().ui.ai_edit.as_ref().unwrap().fields["sendLayerContext"], true);
        assert_eq!(h.state().ui.ai_edit.as_ref().unwrap().fields["includeHiddenLayers"], false);
    }

    #[test]
    fn clicking_ok_runs_background_job_through_blocking_overlay_and_adds_layer() {
        use base64::{Engine as _, engine::general_purpose::STANDARD};
        use std::io::{Read, Write};

        let mut source = photocraft_engine::Session::new();
        source.execute("file.new", json!({"width": 16, "height": 12, "background": "#ff0000"})).unwrap();
        let bytes = photocraft_io::export(&source.active().unwrap().doc, "png", &Default::default()).unwrap().bytes;
        let body = json!({"data": [{"b64_json": STANDARD.encode(&bytes)}]}).to_string();

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}/edit", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut request = Vec::new();
            loop {
                let mut chunk = [0; 4096];
                let n = stream.read(&mut chunk).unwrap();
                assert!(n > 0);
                request.extend_from_slice(&chunk[..n]);
                if let Some(index) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&request[..index]).to_ascii_lowercase();
                    let length =
                        headers.lines().find_map(|line| line.strip_prefix("content-length:").and_then(|n| n.trim().parse::<usize>().ok())).unwrap();
                    if request.len() >= index + 4 + length {
                        break;
                    }
                }
            }
            // Hold past DIALOG_DELAY_MS (250 ms) so BlockingOverlay renders while the job runs.
            std::thread::sleep(Duration::from_millis(350));
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len())
                .unwrap();
        });

        let mut h = Harness::builder().with_size(egui::vec2(1280.0, 900.0)).with_max_steps(64).build_eframe(move |cc| {
            PhotocraftApp::setup_context(&cc.egui_ctx, Default::default());
            cc.egui_ctx.set_missing_glyph_policy(egui::MissingGlyphPolicy::Tofu);
            let mut session = photocraft_engine::Session::new();
            session.edit_prefs(|p| p.interface.language = "en".into());
            session.execute("file.new", json!({"width": 16, "height": 12})).unwrap();
            let mut profile = photocraft_engine::cmd::ai_provider_cmds::preset("seedream").unwrap();
            profile.endpoint = endpoint;
            session.ai_api.save("seedream", profile, Some("offline-ui-key"), false).unwrap();
            let mut app = PhotocraftApp::with_context(cc.egui_ctx.clone(), session, Services::default());
            app.background_jobs = true;
            app
        });
        h.ctx.set_missing_glyph_policy(egui::MissingGlyphPolicy::Tofu);
        h.run_steps(3);
        let ctx = h.ctx.clone();
        photocraft_ui_egui::menus::invoke(h.state_mut(), &ctx, "edit.ai.codex", json!({})).unwrap();
        h.run_steps(2);
        assert!(h.state().dialog_open(), "AI Edit modal must be counted by dialog_open()");
        photocraft_ui_egui::ai_edit_ui::set_field(h.state_mut(), "provider", json!("seedream")).unwrap();
        photocraft_ui_egui::ai_edit_ui::set_field(h.state_mut(), "prompt", json!("Make it red")).unwrap();
        h.run_steps(3);
        assert!(!h.get_by_label("OK").accesskit_node().is_disabled());
        h.get_by_label("OK").click();
        h.run_steps(2);
        assert!(h.state().ui.ai_edit.is_none(), "clicking OK closes the AI Edit modal");
        assert!(h.state().session.has_jobs(), "background AI job must be running");

        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline && h.state().session.has_jobs() {
            h.run_steps(1);
            std::thread::sleep(Duration::from_millis(20));
        }
        h.run_steps(2);
        server.join().unwrap();
        assert!(!h.state().session.has_jobs(), "background AI job finished");
        assert_eq!(h.state().session.active().unwrap().doc.layers.len(), 2, "generated layer must be added");
        assert!(!h.state().ui.status_error, "unexpected status error: {}", h.state().ui.status);
    }

    #[test]
    fn synchronous_confirm_error_posts_visible_notice() {
        let (mut h, _) = harness(AccountInfo::ChatGpt { plan: None });
        wait(&mut h, State::Connected);
        h.state_mut().ui.ai_edit.as_mut().unwrap().fields.insert("prompt".into(), json!("Make it yellow"));
        h.run_steps(2);
        h.get_by_label("OK").click();
        h.run_steps(2);
        assert!(h.state().ui.ai_edit.is_none());
        assert!(h.state().ui.status_error);
        assert!(h.state().ui.status.contains("Offline UI test: image requests are forbidden"), "status: {}", h.state().ui.status);
    }
}
