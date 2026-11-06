<# Build the fork's independent, per-user Windows x64 beta. Does not publish or sign. #>
param(
    [switch]$SkipBuild,
    [string]$WixDll,
    [string]$DotnetPath = 'dotnet',
    [string]$WixUiExtension,
    [string]$OutputDirectory
)
$ErrorActionPreference = 'Stop'
$root = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '../..')).Path
$version = '0.20.1-ai-beta.3'
# Increment the third MSI field for every beta; prerelease suffixes are not MSI versions.
$msiVersion = '0.20.3'
$target = 'x86_64-pc-windows-msvc'
$targetDir = if ($env:CARGO_TARGET_DIR) { [IO.Path]::GetFullPath($env:CARGO_TARGET_DIR) } else { Join-Path $root 'target' }
$out = if ($OutputDirectory) { [IO.Path]::GetFullPath($OutputDirectory) } else { Join-Path $root 'dist/ai-beta' }
$bin = Join-Path $targetDir "$target/release"
$stage = Join-Path $targetDir ('ai-package-' + [guid]::NewGuid().ToString('N'))
[IO.Directory]::CreateDirectory($out) | Out-Null
[IO.Directory]::CreateDirectory($stage) | Out-Null
$docs = Join-Path $stage 'docs'
[IO.Directory]::CreateDirectory($docs) | Out-Null
$utf8 = [Text.UTF8Encoding]::new($false)
Push-Location -LiteralPath $root
try {
    if (-not $SkipBuild) {
        $env:CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS = '-C target-feature=+crt-static'
        $env:PHOTOCRAFT_REQUIRE_WINRES = '1'
        $env:PHOTOCRAFT_BUILD_SHA = (& git rev-parse HEAD).Trim()
        $env:PHOTOCRAFT_BUILD_DATE = (Get-Date).ToUniversalTime().ToString('yyyy-MM-dd')
        & cargo build --release --locked -p photocraft -p photocraft-cli --features heif --target $target
        if ($LASTEXITCODE -ne 0) { throw 'Release compilation failed' }
    }
    foreach ($name in @('photocraft.exe', 'photocraft-cli.exe')) {
        $path = Join-Path $bin $name
        if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "Missing release binary: $name" }
        $bytes = [IO.File]::ReadAllBytes($path)
        if ($bytes.Length -lt 256) { throw "Invalid executable: $name" }
        $pe = [BitConverter]::ToInt32($bytes, 0x3c)
        if ($pe -lt 0 -or $pe -gt $bytes.Length - 128 -or [BitConverter]::ToUInt32($bytes, $pe) -ne 0x4550) { throw "Invalid PE header: $name" }
        $subsystem = if ($name -eq 'photocraft.exe') { 2 } else { 3 }
        if ([BitConverter]::ToUInt16($bytes, $pe + 4) -ne 0x8664 -or [BitConverter]::ToUInt16($bytes, $pe + 0x5c) -ne $subsystem) { throw "Wrong architecture/subsystem: $name" }
        Copy-Item -LiteralPath $path -Destination $stage
    }
    $binaryVersion = (& (Join-Path $bin 'photocraft-cli.exe') --version | Out-String).Trim()
    if ($LASTEXITCODE -ne 0 -or -not $binaryVersion.Contains($version)) { throw 'The executable does not match this beta version' }
    $resource = (Get-Item -LiteralPath (Join-Path $bin 'photocraft.exe')).VersionInfo
    if ($resource.ProductName -ne 'PhotoCraft AI Beta' -or $resource.CompanyName -ne 'BLEZZ0102') { throw 'The executable is not the independent AI distribution' }
    foreach ($name in @('LICENSE-MIT', 'LICENSE-APACHE', 'NOTICE', 'ATTRIBUTION.md')) {
        Copy-Item -LiteralPath (Join-Path $root $name) -Destination $docs
    }
    [IO.File]::WriteAllText((Join-Path $docs 'LICENSE'), "PhotoCraft AI Beta and the PhotoCraft core are licensed under MIT OR Apache-2.0, at your option. See LICENSE-MIT, LICENSE-APACHE, NOTICE and ATTRIBUTION.md. Bundled third-party components retain their own licenses; see THIRD-PARTY-NOTICES.txt and LICENSE-WiX.txt.`n", $utf8)
    Copy-Item -LiteralPath (Join-Path $root 'docs/ai-beta.md') -Destination (Join-Path $docs 'README.md')
    Copy-Item -LiteralPath (Join-Path $root 'assets/app-icon/LICENSE.txt') -Destination (Join-Path $docs 'LICENSE-app-icon.txt')
    # Include license texts from the resolved dependency sources, never their local paths.
    $metadataText = & cargo metadata --locked --format-version 1 --filter-platform $target
    if ($LASTEXITCODE -ne 0) { throw 'Dependency notice inventory failed' }
    $metadata = $metadataText | ConvertFrom-Json
    $notices = [Text.StringBuilder]::new()
    foreach ($package in ($metadata.packages | Sort-Object name, version)) {
        [void]$notices.AppendLine("`n=== $($package.name) $($package.version) | $($package.license) ===")
        $dir = Split-Path -Parent $package.manifest_path
        foreach ($license in (Get-ChildItem -LiteralPath $dir -File | Where-Object { $_.Name -match '^(LICENSE|LICENCE|COPYING|NOTICE|UNLICENSE)([._-]|$)' })) {
            [void]$notices.AppendLine("--- $($license.Name) ---")
            [void]$notices.AppendLine([IO.File]::ReadAllText($license.FullName))
        }
    }
    # Embedded repository assets also need their complete notices, alongside crate licences.
    foreach ($license in (Get-ChildItem -LiteralPath (Join-Path $root 'assets') -Recurse -File | Where-Object { $_.Name -match '^(LICENSE|OFL-)' } | Sort-Object FullName)) {
        $relative = [IO.Path]::GetRelativePath($root, $license.FullName).Replace('\', '/')
        [void]$notices.AppendLine("`n=== $relative ===")
        [void]$notices.AppendLine([IO.File]::ReadAllText($license.FullName))
    }
    [IO.File]::WriteAllText((Join-Path $docs 'THIRD-PARTY-NOTICES.txt'), $notices.ToString(), $utf8)
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'LICENSE-WiX.txt') -Destination $docs
    $stamp = "PhotoCraft AI Beta $version`nSource snapshot commit: $((& git rev-parse HEAD).Trim())`nEmbedded base build: $binaryVersion`nArchitecture: Windows x64`nUnsigned community build. Codex runtime and credentials are not bundled.`n"
    [IO.File]::WriteAllText((Join-Path $docs 'BUILD.txt'), $stamp, $utf8)
    $msi = Join-Path $out "photocraft-ai-$version-windows-x64.msi"
    $wixArguments = @('build', (Join-Path $PSScriptRoot 'ai-beta.wxs'), '-arch', 'x64', '-ext', $WixUiExtension,
        '-d', "Version=$msiVersion", '-d', "DisplayVersion=$version", '-d', "Payload=$stage",
        '-d', "IconPath=$(Join-Path $root 'assets/app-icon/photocraft.ico')", '-pdbtype', 'none', '-o', $msi)
    if (-not $WixUiExtension) { throw 'Pass the path to the WiX 5.0.2 UI extension DLL' }
    if ($WixDll) { & $DotnetPath $WixDll @wixArguments } else { & wix @wixArguments }
    if ($LASTEXITCODE -ne 0) { throw 'MSI build failed' }
    # The portable archive owns its own data folder, separate from both installed versions.
    [IO.File]::WriteAllText((Join-Path $stage 'portable.txt'), "PhotoCraft AI Beta portable mode.`n", $utf8)
    $zip = Join-Path $out "photocraft-ai-$version-windows-x64-portable.zip"
    Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $zip -Force
    Copy-Item -LiteralPath (Join-Path $docs 'README.md') -Destination (Join-Path $out 'README.md')
    $hashes = @($msi, $zip) | ForEach-Object { $hash = Get-FileHash -LiteralPath $_ -Algorithm SHA256; "$($hash.Hash.ToLowerInvariant())  $([IO.Path]::GetFileName($_))" }
    [IO.File]::WriteAllLines((Join-Path $out 'SHA256SUMS.txt'), $hashes, $utf8)
    Get-Item -LiteralPath $msi, $zip | Select-Object Name, Length
} finally { Pop-Location }
