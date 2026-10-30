param(
    [ValidateSet('Setup', 'Format', 'Check', 'Run', 'Launch')]
    [string]$Action = 'Check',
    [string]$CodexPath,
    [string]$CodexHome
)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $projectRoot
$env:RUSTUP_HOME = Join-Path $projectRoot '.tools/rustup'
$env:CARGO_HOME = Join-Path $projectRoot '.tools/cargo'
$cargoBin = Join-Path $env:CARGO_HOME 'bin'
$env:PATH = "$cargoBin;$env:PATH"
# Match the validated local build; callers can explicitly enable incremental builds.
if (-not $env:CARGO_INCREMENTAL) { $env:CARGO_INCREMENTAL = '0' }
# Reuse the user's Windows proxy for tools that do not read Windows proxy settings.
# These settings stay in this PowerShell process and its children; never write global config.
try {
    $downloadUri = [uri]'https://github.com/'
    $downloadProxy = [System.Net.WebRequest]::GetSystemWebProxy().GetProxy($downloadUri)
    if ($downloadProxy -and $downloadProxy.AbsoluteUri -ne $downloadUri.AbsoluteUri) {
        if ($Action -eq 'Setup' -and -not $env:HTTPS_PROXY) { $env:HTTPS_PROXY = $downloadProxy.AbsoluteUri }
        $env:CARGO_NET_GIT_FETCH_WITH_CLI = 'true'
        $configuredGitProxy = & git config --get http.proxy 2>$null
        if (-not $configuredGitProxy) {
            $gitConfigSlots = 0
            if ($env:GIT_CONFIG_COUNT -and -not [int]::TryParse($env:GIT_CONFIG_COUNT, [ref]$gitConfigSlots)) {
                throw 'Invalid process Git configuration count'
            }
            if ($gitConfigSlots -lt 0 -or $gitConfigSlots -gt 64) { throw 'Unexpected process Git configuration count' }
            [Environment]::SetEnvironmentVariable("GIT_CONFIG_KEY_$gitConfigSlots", 'http.proxy', 'Process')
            [Environment]::SetEnvironmentVariable("GIT_CONFIG_VALUE_$gitConfigSlots", $downloadProxy.AbsoluteUri, 'Process')
            $gitConfigSlots++
            $configuredGitVersion = & git config --get http.version 2>$null
            if (-not $configuredGitVersion) {
                [Environment]::SetEnvironmentVariable("GIT_CONFIG_KEY_$gitConfigSlots", 'http.version', 'Process')
                [Environment]::SetEnvironmentVariable("GIT_CONFIG_VALUE_$gitConfigSlots", 'HTTP/1.1', 'Process')
                $gitConfigSlots++
            }
            $env:GIT_CONFIG_COUNT = [string]$gitConfigSlots
        }
        Write-Output 'Using the existing Windows proxy for this process.'
    }
} catch {
    Write-Warning 'Could not reuse Windows proxy settings; existing tool network settings will be used.'
}
if ($CodexPath) {
    $env:PHOTOCRAFT_CODEX_PATH = (Resolve-Path -LiteralPath $CodexPath).Path
}
if ($CodexHome) {
    New-Item -ItemType Directory -Force $CodexHome | Out-Null
    $env:PHOTOCRAFT_CODEX_HOME = (Resolve-Path -LiteralPath $CodexHome).Path
}
if ($Action -eq 'Setup') {
    New-Item -ItemType Directory -Force (Join-Path $projectRoot '.tools') | Out-Null
    $installerPath = Join-Path $projectRoot '.tools/rustup-init.exe'
    Invoke-WebRequest 'https://win.rustup.rs/x86_64' -OutFile $installerPath -TimeoutSec 60
    & $installerPath -y --no-modify-path --default-toolchain 1.95.0 --profile minimal
    if ($LASTEXITCODE -ne 0) { throw 'Rust installation failed' }
    & rustup component add rustfmt clippy
    if ($LASTEXITCODE -ne 0) { throw 'Rust components installation failed' }
    & rustup target add wasm32-unknown-unknown
    if ($LASTEXITCODE -ne 0) { throw 'WebAssembly target installation failed' }
    return
}
if ($Action -ne 'Launch' -and -not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw 'Rust is unavailable. Run ./scripts/ai-local.ps1 Setup first. Windows builds also require Visual Studio C++ Build Tools and the Windows SDK.'
}
if ($Action -in @('Run', 'Launch')) {
    if (-not $env:PHOTOCRAFT_CODEX_PATH) {
        $codexCommand = Get-Command codex.exe -CommandType Application -ErrorAction SilentlyContinue
        if ($codexCommand) {
            $env:PHOTOCRAFT_CODEX_PATH = $codexCommand.Source
        } else {
            # The desktop app does not necessarily add its complete runtime to PATH.
            # UserProfile also works when LocalApplicationData is redirected by the host.
            $codexRoots = @(
                (Join-Path $env:LOCALAPPDATA 'OpenAI/Codex/bin'),
                (Join-Path ([Environment]::GetFolderPath('UserProfile')) 'AppData/Local/OpenAI/Codex/bin')
            ) | Select-Object -Unique
            $codexCandidates = @(foreach ($codexRoot in $codexRoots) {
                if (Test-Path -LiteralPath $codexRoot -PathType Container) {
                    Get-ChildItem -LiteralPath $codexRoot -Directory | ForEach-Object {
                        $candidate = Join-Path $_.FullName 'codex.exe'
                        if (Test-Path -LiteralPath $candidate -PathType Leaf) {
                            Get-Item -LiteralPath $candidate
                        }
                    }
                }
            })
            $latestCodex = $codexCandidates | Sort-Object LastWriteTimeUtc -Descending | Select-Object -First 1
            if ($latestCodex) { $env:PHOTOCRAFT_CODEX_PATH = $latestCodex.FullName }
        }
    }
    if (-not $env:PHOTOCRAFT_CODEX_PATH -or -not (Test-Path -LiteralPath $env:PHOTOCRAFT_CODEX_PATH -PathType Leaf)) {
        throw 'Codex runtime not found. Install the Codex desktop app or pass -CodexPath with its original full codex.exe path.'
    }
    Write-Output "Using Codex runtime: $env:PHOTOCRAFT_CODEX_PATH"
    if ($env:PHOTOCRAFT_CODEX_HOME) { Write-Output "Using Codex profile: $env:PHOTOCRAFT_CODEX_HOME" }
    if (-not $env:PHOTOCRAFT_CONFIG_DIR) {
        $env:PHOTOCRAFT_CONFIG_DIR = Join-Path $projectRoot '.tools/photocraft-profile'
        New-Item -ItemType Directory -Force $env:PHOTOCRAFT_CONFIG_DIR | Out-Null
    }
    # Launch can use the reviewed binary while independent Cargo checks are still running.
    if ($Action -eq 'Launch') {
        $desktopPath = Join-Path $projectRoot 'target/debug/photocraft.exe'
        if (-not (Test-Path -LiteralPath $desktopPath -PathType Leaf)) {
            throw 'PhotoCraft has not been built yet. Use Run first.'
        }
        & $desktopPath --new-instance
    } else {
        # Reuse the validation build and keep this prototype separate from another open instance.
        & cargo run -p photocraft --profile test --locked -- --new-instance
    }
    if ($LASTEXITCODE -ne 0) { throw 'PhotoCraft build/run failed' }
    return
}
if ($Action -eq 'Format') {
    & cargo fmt --all
    if ($LASTEXITCODE -ne 0) { throw 'Formatting failed' }
    return
}
function Invoke-CargoCheck([string[]]$CargoArguments) {
    & cargo @CargoArguments
    if ($LASTEXITCODE -ne 0) { throw "Check failed: cargo $($CargoArguments -join ' ')" }
}
Invoke-CargoCheck -CargoArguments @('fmt', '--all', '--', '--check')
Invoke-CargoCheck -CargoArguments @('test', '-p', 'photocraft-codecs', '-p', 'photocraft-text', '-p', 'photocraft-io', '-p', 'photocraft-gen', '-p', 'photocraft-engine', '-p', 'photocraft-ui-egui', '--lib', '--tests', '--no-fail-fast', '--locked')
Invoke-CargoCheck -CargoArguments @('test', '-p', 'photocraft-engine', '--test', 'panic_hunt', '--', '--ignored')
Invoke-CargoCheck -CargoArguments @('xtask', 'layers')
Invoke-CargoCheck -CargoArguments @('xtask', 'wasm')
Invoke-CargoCheck -CargoArguments @('xtask', 'parity')
Invoke-CargoCheck -CargoArguments @('clippy', '-p', 'photocraft-codecs', '-p', 'photocraft-text', '-p', 'photocraft-io', '-p', 'photocraft-gen', '-p', 'photocraft-engine', '-p', 'photocraft-ui-egui', '--all-targets', '--locked', '--', '-D', 'warnings')
