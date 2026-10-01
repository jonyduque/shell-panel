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

# For argument forms -File cannot express, such as an explicit -Purge:$false.
function Invoke-CommandLine([string]$command) {
    $script:lastOutput = @(& $Shell -NoProfile -ExecutionPolicy Bypass -Command $command 2>&1 | ForEach-Object { "$_" })
    $script:lastOutput | ForEach-Object { Write-Host "    $_" }
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
    $fragment = Get-Content -LiteralPath (Join-Path $fragmentDir 'shell-panel.json') -Raw | ConvertFrom-Json
    Assert (@($fragment.profiles).Count -eq 1) 'reinstall keeps a single Terminal profile'

    Write-Host '== update while shell-panel runs'
    # A running image is mapped with read and delete sharing: it cannot be overwritten, but it
    # can be renamed. This handle reproduces that lock without starting a session.
    $lock = [IO.File]::Open($installedExe, 'Open', 'Read', [IO.FileShare]'Read, Delete')
    try {
        Assert ((Invoke-Script 'install.ps1' $install) -eq 0) 'install succeeds while the old binary is in use'
    } finally { $lock.Dispose() }
    Assert (Test-Path -LiteralPath "$installedExe.old") 'the binary in use was moved aside'
    Assert ((Invoke-Script 'install.ps1' $install) -eq 0) 'the next install exits 0'
    Assert (-not (Test-Path -LiteralPath "$installedExe.old")) 'the next install deletes the moved-aside binary'

    Write-Host '== tampered checksum'
    $hashBefore = (Get-FileHash -LiteralPath $installedExe).Hash
    $bad = Join-Path $work 'BAD_SHA256SUMS.txt'
    (('0' * 64) + "  $zipName") | Set-Content -LiteralPath $bad -Encoding ascii
    $tampered = @('-InstallDir', $installDir, '-TerminalFragmentDir', $fragmentDir, '-ZipPath', $ZipPath, '-ChecksumsPath', $bad)
    Assert ((Invoke-Script 'install.ps1' $tampered) -ne 0) 'a checksum mismatch fails the install'
    Assert ((Get-FileHash -LiteralPath $installedExe).Hash -eq $hashBefore) 'a checksum mismatch leaves the installed binary alone'

    Write-Host '== shared directory'
    # A folder of other tools, one of them happening to be a shell-panel.exe copied by hand.
    $shared = Join-Path $work 'tools'
    New-Item -ItemType Directory -Path $shared | Out-Null
    Set-Content -LiteralPath (Join-Path $shared 'other-tool.txt') -Value 'x'
    Copy-Item -LiteralPath $installedExe -Destination $shared
    $sharedInstall = @('-InstallDir', $shared, '-TerminalFragmentDir', $fragmentDir, '-ZipPath', $ZipPath, '-ChecksumsPath', $sums)
    Assert ((Invoke-Script 'install.ps1' $sharedInstall) -ne 0) 'install refuses a folder that already holds other files'
    Assert ((Invoke-Script 'uninstall.ps1' @('-InstallDir', $shared, '-TerminalFragmentDir', $fragmentDir)) -ne 0) 'uninstall refuses a folder the installer did not create'
    Assert ((Test-Path -LiteralPath (Join-Path $shared 'other-tool.txt')) -and (Test-Path -LiteralPath (Join-Path $shared 'shell-panel.exe'))) 'the shared folder is untouched'

    Write-Host '== constrained language'
    $clm = "`$ExecutionContext.SessionState.LanguageMode = 'ConstrainedLanguage'; & '$(Join-Path $root 'uninstall.ps1')' -InstallDir '$installDir' -TerminalFragmentDir '$fragmentDir'"
    Assert ((Invoke-CommandLine $clm) -ne 0) 'uninstall refuses to run in ConstrainedLanguage mode'
    Assert (($script:lastOutput -join "`n") -match 'needs FullLanguage mode') 'the refusal names FullLanguage mode'
    Assert ((Test-Path -LiteralPath $installedExe) -and (Get-PathCount) -eq 1) 'ConstrainedLanguage leaves the install intact'

    Write-Host '== uninstall'
    Set-Content -LiteralPath (Join-Path $installDir 'notes.txt') -Value 'mine'
    $uninstall = @('-InstallDir', $installDir, '-TerminalFragmentDir', $fragmentDir)
    Assert ((Invoke-Script 'uninstall.ps1' $uninstall) -eq 0) 'uninstall exits 0'
    Assert (-not (Test-Path -LiteralPath $installedExe)) 'binary removed'
    Assert (Test-Path -LiteralPath (Join-Path $installDir 'notes.txt')) 'files the installer did not write are kept'
    Assert ((Get-PathCount) -eq 0) 'PATH entry removed'
    Assert ((Get-RawUserPath) -like "*$marker*") 'uninstall keeps the other PATH entries as written'
    Assert ($envKey.GetValueKind('Path') -eq [Microsoft.Win32.RegistryValueKind]::ExpandString) 'PATH is still REG_EXPAND_SZ after uninstall'
    Assert (-not (Test-Path -LiteralPath $fragmentDir)) 'Terminal fragment removed'
    Assert ((Invoke-Script 'uninstall.ps1' $uninstall) -eq 0) 'a second uninstall succeeds'
    Assert (Test-Path -LiteralPath (Join-Path $installDir 'notes.txt')) 'a second uninstall still keeps the user file'

    Write-Host '== purge'
    $profileDir = Join-Path $work 'profile'
    $configFile = Join-Path $profileDir '.config\shell-panel.toml'
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $configFile) | Out-Null
    Set-Content -LiteralPath $configFile -Value 'max_suggestions = 5'
    $savedProfile = $env:USERPROFILE
    $env:USERPROFILE = $profileDir
    try {
        $keep = "& '$(Join-Path $root 'uninstall.ps1')' -Purge:`$false -InstallDir '$installDir' -TerminalFragmentDir '$fragmentDir'"
        Assert ((Invoke-CommandLine $keep) -eq 0) 'uninstall -Purge:$false exits 0'
        Assert (Test-Path -LiteralPath $configFile) 'uninstall -Purge:$false keeps the configuration'
        Assert ((Invoke-Script 'uninstall.ps1' @('-Purge', '-InstallDir', $installDir, '-TerminalFragmentDir', $fragmentDir)) -eq 0) 'uninstall -Purge exits 0'
        Assert (-not (Test-Path -LiteralPath $configFile)) 'uninstall -Purge removes the configuration'
    } finally { $env:USERPROFILE = $savedProfile }

    Write-Host "test result: ok. $script:passed passed; 0 failed"
} catch {
    Write-Host "[!] $($_.Exception.Message)"
    Write-Host "test result: FAILED. $script:passed passed; 1 failed"
    throw
} finally {
    if ($hadPath) { $envKey.SetValue('Path', $savedPath, $savedKind) } else { $envKey.DeleteValue('Path', $false) }
    $envKey.Close()
    # The last broadcast came from uninstall.ps1 while PATH still held the test entry: broadcast
    # again so Explorer hands new processes the restored PATH (same trick as install.ps1).
    [Environment]::SetEnvironmentVariable('SHELL_PANEL_INSTALLER', '1', 'User')
    [Environment]::SetEnvironmentVariable('SHELL_PANEL_INSTALLER', $null, 'User')
    Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue
}
