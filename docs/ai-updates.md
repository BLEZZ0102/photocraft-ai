# PhotoCraft AI update publishing

Updated 2026-11-02. This fork checks only public releases in
`BLEZZ0102/photocraft-ai`, never the upstream PhotoCraft feed or the ArtCraft suite feed.
The current distribution supports Windows x64, independent per-user AI MSI installations
and portable ZIPs. Development builds and other platforms do not install updates.

## When users see an update

- The tag is `v<VERSION>`, with a strictly newer semantic version than the running binary.
- AI Beta users receive newer `ai-beta.N` releases and stable releases. Stable users receive
  only stable releases. Development version suffixes are not treated as the Beta channel.
- The release is published, not a draft. A source tag alone never triggers a notification.
- The asset for the user's installation is present, in GitHub's `uploaded` state, nonempty,
  at most 1 GiB, with a valid GitHub `sha256:` digest and an exact URL in this repository:
  - `photocraft-ai-<VERSION>-windows-x64.msi`
  - `photocraft-ai-<VERSION>-windows-x64-portable.zip`

For example, `v0.18.0-ai-beta.2` must carry
`photocraft-ai-0.18.0-ai-beta.2-windows-x64.msi`. The updater checks the release list,
including prereleases, rather than `/releases/latest`, which excludes the published Beta.
An incomplete newest release does not hide a slightly older, complete update.

## Release procedure

1. Integrate and test changes in the independent AI distribution (the packaging branch is
   `codex/package-ai-beta`). Keep its per-user identity, profile directories and MSI UpgradeCode
   `8E0C7BE0-745A-4D87-B77C-4A4315D4EC62`. Do not substitute the upstream MSI.
2. Increase the AI version and the three-field MSI product version. The binary's reported
   version must exactly match the release tag and asset names. Reusing an existing version
   does not notify users; replacing a published asset in place is not an update mechanism.
3. Build and test both real packages, including an upgrade from the previous AI MSI in an
   isolated test installation. Include this updater in the binaries. The ZIP root contains
   `photocraft.exe`, `photocraft-cli.exe`, `portable.txt` and `docs/`; never include user data.
4. Create a **draft** GitHub release and upload both packages, checksums, notices and source.
   Verify remote sizes and GitHub SHA-256 digests against the local files.
5. Publish the draft only when all required packages have passed acceptance. Publishing is
   the final notification switch; do not publish an unfinished installer. Unpublishing or
   deleting the package makes it unavailable to subsequent checks/download preparations.

No workflow change or separate update server is required. Startup checks run once per launch;
Help > Check for Updates checks manually. Automatic checks can be disabled in preferences.
No periodic polling, automatic installation or document upload is performed.

Agents use `help.checkForUpdates` and `updates.status`, then explicitly call `updates.install`
to prepare a verified package and `updates.restart` when `installation.status` is `ready`.
Each command takes `{}`. `updates.status` and `ui.inspect` expose `oneClickSupported` and
`installation` (`idle`, `downloading`, `ready`, `restarting`, `failed`). The UI button performs
the two installation steps automatically, with the same save guards. A check cannot change
the offered version while an installation is in progress.

## Installation and recovery

The update action re-fetches release metadata, downloads over certificate-validated HTTPS
and verifies GitHub's SHA-256 before scheduling a helper. This permits the current unsigned
community Beta; it relies on control of this GitHub repository, not an independent signing key.
It does not disable OS application-control policy. Keep the release account protected.

Unsaved documents, active text/transforms, saves and background jobs prevent restart. A second
guard runs after download. The helper waits for the original process, checks the package hash
again, then updates the original folder and restarts it. MSI installation is silent and never
requests a system restart. Portable replacement uses bounded extraction, rejects user-data
entries and restores previous program files on a replacement failure. Documents and profile
directories are outside the update payload. Failures are reported on the next start; MSI
logs remain in the updater's staging folder under the per-user PhotoCraftAI cache.

Old `0.18.0-ai-beta.1` binaries do not gain this code remotely. Their users must manually install
the first updated AI package, after which subsequent releases can use the one-click action.
