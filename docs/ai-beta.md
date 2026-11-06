# PhotoCraft AI Beta 0.20.1-ai-beta.3

An unofficial community build of PhotoCraft, distributed by BLEZZ0102 under MIT OR Apache-2.0. Original PhotoCraft contributors retain their credits.

This version integrates upstream PhotoCraft through a25999311 (November 6, 2026), adds layer-aware Codex input, and retains the independent AI update channel. Keep backups of important documents while using this beta.

## Installation

Windows 10/11 x64. Run the MSI or extract the portable ZIP and run photocraft.exe. Rust, Visual Studio and .NET are not required.

- MSI: per-user, normally in %LOCALAPPDATA%\Programs\PhotoCraftAI.
- Settings: %APPDATA%\PhotoCraftAI. Recovery: %LOCALAPPDATA%\PhotoCraftAI\Recovery.
- Portable settings: the adjacent PhotoCraftData folder.
- The AI distribution has a separate installer identity and does not replace official PhotoCraft or claim default file associations.
- Community packages are unsigned. Verify the repository and SHA-256 checksums and follow your device's application-control policy.

## AI editing

Open a document and choose Edit > AI Edit, or the sparkle toolbar button.

For Codex, install the complete Codex desktop runtime separately and sign in with your own ChatGPT account. Image editing availability depends on your account and runtime. No runtime, subscription, model or credentials are bundled.

Codex receives individual visible layer previews, names and structure by default. Hidden layers and children of hidden groups are excluded unless Also send hidden layers is explicitly enabled for that edit. Isolated previews may reveal content covered by other layers or group masks. Up to 15 leaf images plus the canvas can be sent; oversized contexts fail explicitly. Turn off individual references to send only the visible canvas.

Groups and adjustments provide metadata; leaf layers provide rendered previews, not raw editable PSD data. Interpretation is model-dependent. The result is one full-canvas image added as a new undoable layer. Original layers remain intact; this does not return an editable AI layer stack.

Gemini, Doubao/Seedream and FLUX use explicitly configured APIs and continue to receive flattened canvas input. Configure your own supported endpoint, model and key, save, then submit a prompt. Saving does not generate an image. API usage may incur provider charges. There is no automatic fallback to a paid API or automatic generation retry.

## Updates

Download AI builds from https://github.com/BLEZZ0102/photocraft-ai/releases.
Beta 2 and later support in-app updates. Beta 1 users must manually install a newer AI package once. Updates are offered only after a newer AI release is published with a complete package and verified GitHub digest. Save your work and wait for background tasks before updating.

## Troubleshooting and removal

For a graphics driver problem, launch photocraft.exe --safe-gpu for CPU canvas rendering. Use --new-instance to run a second copy for testing.

Uninstall through Windows Settings. User settings, recovery files, documents and the separately installed Codex runtime are retained. Uninstalling PhotoCraft AI does not sign out Codex.

Source: https://github.com/BLEZZ0102/photocraft-ai
Upstream: https://github.com/storytold/photocraft
See LICENSE-MIT, LICENSE-APACHE, NOTICE, ATTRIBUTION.md and THIRD-PARTY-NOTICES.txt. The app icon has its own LICENSE-app-icon.txt. Restricted ArtCraft brand artwork is not included in the installer.
