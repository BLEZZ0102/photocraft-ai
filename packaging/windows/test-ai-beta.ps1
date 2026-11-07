param([Parameter(Mandatory)][string]$MsiPath, [string]$PortablePath)
$ErrorActionPreference = 'Stop'
$msi = (Resolve-Path -LiteralPath $MsiPath).Path
$installer = New-Object -ComObject WindowsInstaller.Installer
$database = $installer.OpenDatabase($msi, 0)
function Rows([string]$sql, [int]$columns) {
    $view = $database.OpenView($sql)
    try {
        [void]$view.Execute()
        while ($null -ne ($record = $view.Fetch())) {
            try {
                $values = @(for ($i = 1; $i -le $columns; $i++) {
                    $record.GetType().InvokeMember('StringData', 'GetProperty', $null, $record, @($i))
                })
                ,$values
            } finally { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($record) }
        }
    } finally {
        [void]$view.Close()
        [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($view)
    }
}
function Assert([bool]$condition, [string]$message) { if (-not $condition) { throw $message } }
$properties = @{}
foreach ($row in (Rows 'SELECT `Property`, `Value` FROM `Property`' 2)) { $properties[$row[0]] = $row[1] }
Assert ($properties.ProductName -eq 'PhotoCraft AI Beta') 'Product name mismatch'
Assert ($properties.Manufacturer -eq 'BLEZZ0102') 'Publisher must identify the fork distributor'
Assert ($properties.UpgradeCode -eq '{8E0C7BE0-745A-4D87-B77C-4A4315D4EC62}') 'Upgrade identity mismatch'
Assert ($properties.UpgradeCode -ne '{D9660F32-2D0A-4ABE-B73D-17BE25F1542C}') 'Must never upgrade official PhotoCraft'
Assert (-not $properties.ContainsKey('ALLUSERS')) 'The beta must be per-user'
Assert ($properties.ProductVersion -eq '0.20.4') 'MSI beta upgrade version mismatch'
Assert ($properties.ARPURLUPDATEINFO -eq 'https://github.com/BLEZZ0102/photocraft-ai/releases') 'Update URL mismatch'
$conditions = @((Rows 'SELECT `Condition` FROM `LaunchCondition`' 1) | ForEach-Object { $_[0] })
Assert ($conditions -contains 'NOT ALLUSERS') 'Must refuse a forced per-machine installation'
$directories = @{}
foreach ($row in (Rows 'SELECT `Directory`, `Directory_Parent`, `DefaultDir` FROM `Directory`' 3)) { $directories[$row[0]] = @($row[1], $row[2]) }
Assert ($directories.INSTALLFOLDER[1] -match 'PhotoCraftAI$') 'Install folder must be distinct'
Assert ($directories.ProgramsFolder[0] -eq 'LocalAppDataFolder') 'Default install must remain in the current account'
$files = @((Rows 'SELECT `FileName` FROM `File`' 1) | ForEach-Object { ($_[0] -split '\|')[-1] })
foreach ($required in @('photocraft.exe', 'photocraft-cli.exe', 'LICENSE', 'LICENSE-MIT', 'LICENSE-APACHE', 'NOTICE', 'ATTRIBUTION.md', 'README.md', 'LICENSE-app-icon.txt', 'LICENSE-WiX.txt', 'THIRD-PARTY-NOTICES.txt', 'BUILD.txt')) {
    Assert ($files -contains $required) "Missing required installed file: $required"
}
Assert ($files.Count -eq 12) 'Unexpected files in the installer; do not bundle profiles or runtime accounts'
foreach ($row in (Rows 'SELECT `Root`, `Key` FROM `Registry`' 2)) {
    Assert ($row[0] -eq '1' -and $row[1] -eq 'Software\BLEZZ0102\PhotoCraftAI') 'Registry writes must be restricted to the fork HKCU key'
}
$savedFolder = @(Rows 'SELECT `Value` FROM `Registry` WHERE `Name` = ''InstallDir''' 1)
Assert ($savedFolder.Count -eq 1 -and $savedFolder[0][0] -eq '[INSTALLFOLDER]') 'Maintenance must remember the exact installed directory'
$binaryVersion = @(Rows 'SELECT `Value` FROM `Registry` WHERE `Name` = ''BinaryVersion''' 1)
Assert ($binaryVersion.Count -eq 1 -and $binaryVersion[0][0] -eq '0.20.1-ai-beta.3.1.1') 'Public AI Beta version mismatch'
$restore = @(Rows 'SELECT `Condition` FROM `InstallExecuteSequence` WHERE `Action` = ''AIRestoreInstallFolder''' 1)
Assert ($restore.Count -eq 1) 'Missing directory restore action before maintenance costing'
$session = $installer.OpenPackage($msi, 1)
try {
    foreach ($case in @(@('E:\Existing beta\', '', 'E:\Existing beta\'), @('E:\Existing beta\', 'F:\Explicit beta\', 'F:\Explicit beta\'), @('', '', ''))) {
        [void]$session.GetType().InvokeMember('Property', 'SetProperty', $null, $session, @('AI_PREVIOUSFOLDER', $case[0]))
        [void]$session.GetType().InvokeMember('Property', 'SetProperty', $null, $session, @('INSTALLFOLDER', $case[1]))
        if ($session.EvaluateCondition($restore[0][0]) -eq 1) {
            Assert ($session.DoAction('AIRestoreInstallFolder') -eq 1) 'Directory restore property action failed'
        }
        $folder = $session.GetType().InvokeMember('Property', 'GetProperty', $null, $session, @('INSTALLFOLDER'))
        Assert ($folder -eq $case[2]) 'Remembered directory must survive uninstall/repair and preserve explicit overrides'
    }
} finally { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($session) }
$shortcut = @(Rows 'SELECT `Target` FROM `Shortcut`' 1)
Assert ($shortcut.Count -eq 1 -and $shortcut[0][0] -eq '[#AIExe]') 'Expected one direct Start menu shortcut'
if ($PortablePath) {
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $zip = [IO.Compression.ZipFile]::OpenRead((Resolve-Path -LiteralPath $PortablePath).Path)
    try {
        $names = @($zip.Entries | ForEach-Object { $_.FullName.Replace('\', '/') })
        foreach ($required in @('photocraft.exe', 'photocraft-cli.exe', 'portable.txt', 'docs/README.md', 'docs/THIRD-PARTY-NOTICES.txt')) {
            Assert ($names -contains $required) "Portable archive root/layout mismatch: $required"
        }
        Assert (@($names | Where-Object { $_ -match '(?i)(auth\.json|ai-providers\.json|\.codex/|photocraft-profile/|docs/brand/)' }).Count -eq 0) 'Private profiles or restricted brand assets in archive'
    } finally { $zip.Dispose() }
}
Write-Output 'PASS: fork identity, per-user scope, independent folders, registry ownership, payload and shortcut'
