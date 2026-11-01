# Windows packaging

See [installation policy checks](INSTALLATION.md) for the installer scope and path gates.

## Independent AI Beta distribution

`package-ai-beta.ps1` creates the Windows x64, per-user **PhotoCraft AI Beta** MSI and portable ZIP. It uses `ai-beta.wxs`, a distinct upgrade identity, separate application data, no file associations, and an explicit current-user-only guard. It does not publish, sign or bundle Codex. Pass the WiX 5.0.2 CLI DLL, matching UI extension DLL and a local .NET runtime, or use an installed WiX CLI. The script checks binary identity, architecture and version before staging. Use `test-ai-beta.ps1 -MsiPath <msi> -PortablePath <zip>` to inspect the real payload without installing it. This metadata check does not replace an actual install/uninstall test. See [user instructions](../../docs/ai-beta.md).
