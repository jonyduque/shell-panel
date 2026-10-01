<#
.SYNOPSIS
    End-to-end test of install.ps1 and uninstall.ps1 against a release zip.
.DESCRIPTION
    Installs into a temporary directory with a temporary Windows Terminal fragment directory,
    checks the result, reinstalls, tries a tampered checksum and a foreign directory, then
    uninstalls twice. The user's PATH registry value is saved and restored, so this can run on a
    developer machine. Without -ZipPath it packs target\release\shell-panel.exe; run
    `cargo build --release` first.
.PARAMETER Shell
    The PowerShell that runs install.ps1 and uninstall.ps1: pwsh (7) or powershell (5.1).
#>
[CmdletBinding()]
param(
    [string]$ZipPath,
    [ValidateSet('pwsh', 'powershell')][string]$Shell = 'pwsh'
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $PSScriptRoot
$version = (Select-String -LiteralPath (Join-Path $root 'Cargo.toml') -Pattern '^version\s*=\s*"([^"]+)"' |
    Select-Object -First 1).Matches[0].Groups[1].Value

$work = Join-Path ([IO.Path]::GetTempPath()) ('sp-installer-test-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $work | Out-Null
$installDir = Join-Path $work 'Programs\shell-panel'
$fragmentDir = Join-Path $work 'Fragments\shell-panel'

$envKey = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Environment', $true)
$hadPath = $envKey.GetValueNames() -contains 'Path'
$savedPath = $envKey.GetValue('Path', $null, [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
$savedKind = if ($hadPath) { $envKey.GetValueKind('Path') } else { $null }
$script:passed = 0

function Assert([bool]$condition, [string]$message) {
    if (-not $condition) { throw "FAIL: $message" }
    $script:passed++
    Write-Host "[OK] $message"
}

function Invoke-Script([string]$name, [string[]]$arguments) {
    & $Shell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $root $name) @arguments 2>&1 |
        ForEach-Object { Write-Host "    $_" }
    $LASTEXITCODE
}

function Get-RawUserPath {
    $key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Environment')
    try { [string]$key.GetValue('Path', '', [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames) }
    finally { $key.Close() }
}

function Get-PathCount {
    @((Get-RawUserPath) -split ';' | Where-Object { $_.TrimEnd('\') -ieq $installDir.TrimEnd('\') }).Count
}

try {
    # Start from a PATH that holds an unexpanded variable, the case a careless edit destroys.
    $marker = '%SP_INSTALLER_TEST%\bin'
    $seed = if ($hadPath) { [string]$savedPath + ';' + $marker } else { $marker }
    $envKey.SetValue('Path', $seed, [Microsoft.Win32.RegistryValueKind]::ExpandString)

    if (-not $ZipPath) {
        $built = Join-Path $root 'target\release\shell-panel.exe'
        if (-not (Test-Path -LiteralPath $built)) { throw 'Build first: cargo build --release' }
        $stage = Join-Path $work 'stage'
        New-Item -ItemType Directory -Path $stage | Out-Null
        Copy-Item -LiteralPath $built, (Join-Path $root 'LICENSE'), (Join-Path $root 'README.md') -Destination $stage
        $ZipPath = Join-Path $work "shell-panel-$version-x64.zip"
        Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $ZipPath
    }
    $zipName = Split-Path -Leaf $ZipPath
    $sums = Join-Path $work 'SHA256SUMS.txt'
    '{0}  {1}' -f (Get-FileHash -LiteralPath $ZipPath -Algorithm SHA256).Hash.ToLower(), $zipName |
        Set-Content -LiteralPath $sums -Encoding ascii
    $install = @('-InstallDir', $installDir, '-TerminalFragmentDir', $fragmentDir, '-ZipPath', $ZipPath, '-ChecksumsPath', $sums)
    $installedExe = Join-Path $installDir 'shell-panel.exe'

    Write-Host "== install ($Shell)"
    Assert ((Invoke-Script 'install.ps1' $install) -eq 0) 'install exits 0'
    $reported = & $installedExe --version
    Assert ($reported -match [regex]::Escape($version)) "installed binary reports $version ($reported)"
    Assert ((Get-PathCount) -eq 1) 'install dir is on the user PATH once'
    Assert ((Get-RawUserPath) -like "*$marker*") 'unexpanded PATH entries are kept as written'
    Assert ($envKey.GetValueKind('Path') -eq [Microsoft.Win32.RegistryValueKind]::ExpandString) 'PATH stays REG_EXPAND_SZ'
    $fragment = Get-Content -LiteralPath (Join-Path $fragmentDir 'shell-panel.json') -Raw | ConvertFrom-Json
    Assert (@($fragment.profiles).Count -eq 1) 'Terminal fragment holds one profile'
    Assert ($fragment.profiles[0].commandline -eq ('"' + $installedExe + '"')) 'Terminal profile runs the installed binary'
    Assert ($fragment.profiles[0].guid -eq '{b6f3a6a8-5d0e-4c55-9a5a-3e7c1f2d9b41}') 'Terminal profile has the fixed GUID'

    Write-Host '== reinstall'
    Assert ((Invoke-Script 'install.ps1' $install) -eq 0) 'reinstall exits 0'
    Assert ((Get-PathCount) -eq 1) 'reinstall does not duplicate the PATH entry'

    Write-Host '== tampered checksum'
    $hashBefore = (Get-FileHash -LiteralPath $installedExe).Hash
    $bad = Join-Path $work 'BAD_SHA256SUMS.txt'
    (('0' * 64) + "  $zipName") | Set-Content -LiteralPath $bad -Encoding ascii
    $tampered = @('-InstallDir', $installDir, '-TerminalFragmentDir', $fragmentDir, '-ZipPath', $ZipPath, '-ChecksumsPath', $bad)
    Assert ((Invoke-Script 'install.ps1' $tampered) -ne 0) 'a checksum mismatch fails the install'
    Assert ((Get-FileHash -LiteralPath $installedExe).Hash -eq $hashBefore) 'a checksum mismatch leaves the installed binary alone'

    Write-Host '== foreign directory'
    $foreign = Join-Path $work 'not-shell-panel'
    New-Item -ItemType Directory -Path $foreign | Out-Null
    Set-Content -LiteralPath (Join-Path $foreign 'keep.txt') -Value 'x'
    Assert ((Invoke-Script 'uninstall.ps1' @('-InstallDir', $foreign, '-TerminalFragmentDir', $fragmentDir)) -ne 0) 'uninstall refuses a directory without shell-panel.exe'
    Assert (Test-Path -LiteralPath (Join-Path $foreign 'keep.txt')) 'the foreign directory is untouched'

    Write-Host '== uninstall'
    $uninstall = @('-InstallDir', $installDir, '-TerminalFragmentDir', $fragmentDir)
    Assert ((Invoke-Script 'uninstall.ps1' $uninstall) -eq 0) 'uninstall exits 0'
    Assert (-not (Test-Path -LiteralPath $installDir)) 'install dir removed'
    Assert ((Get-PathCount) -eq 0) 'PATH entry removed'
    Assert ((Get-RawUserPath) -like "*$marker*") 'uninstall keeps the other PATH entries as written'
    Assert ($envKey.GetValueKind('Path') -eq [Microsoft.Win32.RegistryValueKind]::ExpandString) 'PATH is still REG_EXPAND_SZ after uninstall'
    Assert (-not (Test-Path -LiteralPath $fragmentDir)) 'Terminal fragment removed'
    Assert ((Invoke-Script 'uninstall.ps1' $uninstall) -eq 0) 'a second uninstall succeeds'

    Write-Host "test result: ok. $script:passed passed; 0 failed"
} catch {
    Write-Host "[!] $($_.Exception.Message)"
    Write-Host "test result: FAILED. $script:passed passed; 1 failed"
    throw
} finally {
    if ($hadPath) { $envKey.SetValue('Path', $savedPath, $savedKind) } else { $envKey.DeleteValue('Path', $false) }
    $envKey.Close()
    Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue
}
