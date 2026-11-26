# Windows packaging

See [installation policy checks](INSTALLATION.md) for the installer scope and path gates.

## Independent AI Beta distribution

`package-ai-beta.ps1` creates the Windows x64, per-user **PhotoCraft AI Beta** MSI and portable ZIP. It uses `ai-beta.wxs`, a distinct upgrade identity, separate application data, no file associations, and an explicit current-user-only guard. It does not publish, sign or bundle Codex. Pass the WiX 5.0.2 CLI DLL, matching UI extension DLL and a local .NET runtime, or use an installed WiX CLI. The script checks binary identity, architecture and version before staging. Use `test-ai-beta.ps1 -MsiPath <msi> -PortablePath <zip>` to inspect the real payload without installing it. This metadata check does not replace an actual install/uninstall test. See [user instructions](../../docs/ai-beta.md).

## Nightly build and signing boundary

`package.ps1 -Arch x64 -Nightly -BuildOnly` is for the secret-free compilation job.
It refuses signing credential variables and builds the GUI only, without CLI,
portable marker, installer or associations. Its `*-nightly.unsigned.zip` is an
internal intermediate, not the final feed artifact. There is no unsigned
publication switch.

The separate trusted producer signing job safely unpacks that new intermediate,
checks its exact identity/inventory, then invokes the existing backend:

```powershell
pwsh packaging/windows/sign.ps1 -Required -CheckOnly
pwsh packaging/windows/sign.ps1 -Required -Files <stage>/photocraft.exe
```

Missing or partial configuration fails. `-Required` verifies Authenticode and
a signer certificate after the existing signtool verification. Stable local
packaging keeps its existing optional-signing policy. The producer independently
verifies the final signature, PE identity and inventory before archiving under
the normal feed filename, and refuses to overwrite prepared outputs. It never
executes the payload in the signing step. Real keys belong only to that step,
not compilation. These interfaces do not configure or grant access to keys.

The signing-policy fixtures use generated dummy values and a mocked compiler
boundary; they do not prove real certificate signing or release readiness.
