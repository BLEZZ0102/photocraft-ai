// AI view vocabulary, including dynamic form, job and notice templates.
const AI_KEYS: &[(&str, &str)] = &[
    ("", "API settings"),
    (r#""#, r#"AI Edit…"#),
    (r#"@id"#, r#"edit.ai.codex"#),
    (r#""#, r#"Describe your edit"#),
    (
        r#""#,
        r#"The visible canvas and instruction will be sent to Codex using your ChatGPT sign-in. The result is fitted to the canvas and added as a new layer."#,
    ),
    (r#""#, r#"Codex account has not been checked"#),
    (r#""#, r#"Checking Codex connection…"#),
    (r#""#, r#"Complete ChatGPT sign-in in your browser"#),
    (r#""#, r#"Connected to ChatGPT"#),
    (r#""#, r#"ChatGPT plan"#),
    (r#""#, r#"ChatGPT sign-in is required"#),
    (r#""#, r#"Codex is using another login method. Sign in with ChatGPT to use your subscription."#),
    (r#""#, r#"Cancelling sign-in…"#),
    (r#""#, r#"Sign-in cancelled"#),
    (r#""#, r#"Could not verify the account. Check your connection or sign in again."#),
    (r#""#, r#"Connection details"#),
    (r#""#, r#"Sign in with ChatGPT"#),
    (r#""#, r#"Open sign-in page"#),
    (r#""#, r#"Check connection"#),
    (r#""#, r#"Cancel sign-in"#),
    (r#""#, r#"Copy sign-in link"#),
    (r#""#, r#"Could not open your browser. Try again or copy the sign-in link into your browser."#),
    (r#""#, r#"Sign-in is managed by the installed Codex runtime and shared with Codex. PhotoCraft does not store tokens."#),
    (r#""#, r#"Image service"#),
    (r#""#, r#"Codex (ChatGPT subscription)"#),
    (r#""#, r#"Doubao / Seedream"#),
    (r#""#, r#"This API sends the visible canvas and instruction to the configured service. Usage may incur charges billed by that service."#),
    (r#""#, r#"Model ID"#),
    (r#""#, r#"API address and protocol"#),
    (r#""#, r#"API address"#),
    (r#""#, r#"API protocol"#),
    (r#""#, r#"Custom services must support the selected image-editing protocol. Changing the address requires a key for that address."#),
    (r#""#, r#"API key"#),
    (r#""#, r#"API key configured — leave empty to keep it"#),
    (r#""#, r#"Remember API key in system credential store"#),
    (r#""#, r#"The API key is available for this session only."#),
    (r#""#, r#"Save API settings"#),
    (r#""#, r#"Remove API key"#),
    (r#""#, r#"API configured. Saving settings does not generate an image."#),
    (r#""#, r#"Save your API settings before editing."#),
    (r#""#, r#"API settings saved"#),
    (r#""#, r#"Enter a valid API key without whitespace"#),
    (r#""#, r#"API key removed"#),
    (r#""#, r#"Could not load API settings"#),
    (r#""#, r#"The result is fitted to the canvas and added as a new layer. Your existing layers are preserved."#),
    (r#""#, r#"Codex was not found. Install and open the Codex desktop app, then check the connection again."#),
    (r#""#, r#"Use the complete Codex installation; copying codex.exe alone may omit image-editing components."#),
    (r#""#, r#"PhotoCraft AI Updates"#),
    (r#""#, r#"Updates are offered only after a new AI package is published."#),
    (r#""#, r#"PhotoCraft AI {version} is available."#),
    (r#""#, r#"No newer AI package is available."#),
    (r#""#, r#"Update and Restart"#),
    (r#""#, r#"Downloading and verifying update…"#),
    (r#""#, r#"Could not install the update."#),
    (r#""#, r#"Save your documents and wait for background tasks before updating."#),
    (r#""#, r#"AI Project Page"#),
    (r#""#, r#"Release Notes"#),
    (r#""#, r#"Installed version: {version}"#),
    (r#""#, r#"Checking for updates…"#),
    (r#""#, r#"Could not check for updates."#),
    (r#""#, r#"Updates have not been checked yet."#),
    (r#""#, r#"Check Again"#),
    (r#""#, r#"AI edit did not pass the service's content review"#),
    (r#""#, r#"The image service's content review is unavailable"#),
    (r#""#, r#"Your editing instruction did not pass the service's content review."#),
    (r#""#, r#"Your reference image did not pass the service's content review."#),
    (r#""#, r#"The generated image did not pass the service's content review."#),
    (r#""#, r#"Your instruction triggered the service's content rules, which may include copyright restrictions."#),
    (r#""#, r#"Your reference image triggered the service's content rules, which may include copyright restrictions."#),
    (r#""#, r#"Your reference image did not pass the service's privacy review."#),
    (r#""#, r#"The generated image triggered the service's content rules, which may include copyright restrictions."#),
    (r#""#, r#"The generated image triggered the service's impersonation restrictions."#),
    (r#""#, r#"The service rejected this edit without identifying the content-review stage."#),
    (r#""#, r#"The service's content review failed, so this edit could not be completed."#),
    (r#""#, r#"Revise the instruction and try again manually."#),
    (r#""#, r#"Choose a reference image that meets the service's requirements."#),
    (r#""#, r#"Adjust the editing request or contact the service provider for details."#),
    (r#""#, r#"Check that your editing request meets the service's content requirements."#),
    (r#""#, r#"Check your editing content or ask the service provider for details."#),
    (r#""#, r#"Try again manually later. Contact the service provider if the problem continues."#),
    (r#""#, r#"This edit did not change existing layers or add a new layer."#),
    (r#""#, r#"Error code: {code}"#),
    (r#""#, r#"AI Edit"#),
    (r#""#, r#"AI Edit via API"#),
    (r#""#, r#"Codex Account"#),
    (r#""#, r#"AI Provider Settings"#),
];

#[test]
fn every_registered_catalog_covers_ai_controls_and_dynamic_notices() {
    use super::{LANGUAGES, Lang, catalog::parse_entries, tr, tr_id};
    for info in LANGUAGES.iter().filter(|info| info.code != "en") {
        let lang = Lang::from_code(info.code).unwrap();
        let (entries, errors) = parse_entries(&info.text());
        assert!(errors.is_empty(), "{}: {errors:?}", info.code);
        let entries: std::collections::HashMap<(&str, &str), &str> = entries.iter().map(|(ctx, src, tr)| ((ctx.as_str(), src.as_str()), tr.as_str())).collect();
        for &(context, source) in AI_KEYS {
            let expected = entries.get(&(context, source)).unwrap_or_else(|| panic!("{} lacks {context:?} {source:?}", info.code));
            if source != "Doubao / Seedream" {
                assert_ne!(*expected, source, "{}: untranslated AI text {source:?}", info.code);
            }
            let rendered = if context == "@id" { tr_id(lang, source, "AI Edit…") } else { tr(lang, source) };
            assert_eq!(rendered, *expected, "{}: {source}", info.code);
        }
    }
}
