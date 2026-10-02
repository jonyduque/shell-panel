#Requires -Version 7.0
<#
.SYNOPSIS
    End-to-end test of install.ps1 and uninstall.ps1 against a release zip.
.DESCRIPTION
    Installs into a temporary directory with a temporary Windows Terminal fragment directory,
    checks the result, reinstalls, tries a tampered checksum and a foreign directory, then
    uninstalls twice. The user's PATH registry value is saved and restored, so this can run on a
    developer machine. Without -ZipPath it packs target\release\shell-panel.exe; run
    `cargo build --release` first.
    Only one run at a time per machine: the run holds the named mutex
    Global\shell-panel-installer-test from before it saves PATH until after it has restored and
    checked it, and a second run waits for it (see -LockTimeoutSeconds). Before it changes PATH the
    run writes the saved value to %TEMP%\shell-panel-installer-test.recovery.json and deletes the
    file after a successful restore; a run that finds that file was preceded by one that was
    killed, so it refuses to start and prints how to restore the saved value.
.PARAMETER Shell
    The PowerShell that runs install.ps1 and uninstall.ps1: pwsh (7) or powershell (5.1).
.PARAMETER LockTimeoutSeconds
    How long to wait for another run to release the machine-wide lock. Default: 600.
.PARAMETER RecoveryFile
    Testing only: the recovery file, instead of %TEMP%\shell-panel-installer-test.recovery.json.
.PARAMETER LockName
    Testing only: the name of the machine-wide lock, instead of Global\shell-panel-installer-test.
#>
[CmdletBinding()]
param(
    [string]$ZipPath,
    [ValidateSet('pwsh', 'powershell')][string]$Shell = 'pwsh',
    [int]$LockTimeoutSeconds = 600,
    [string]$RecoveryFile = (Join-Path $env:TEMP 'shell-panel-installer-test.recovery.json'),
    [string]$LockName = 'Global\shell-panel-installer-test'
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# The PATH is saved before anything else can fail, and restored on every exit path (see the
# finally at the end). The harness itself needs PowerShell 7; -Shell picks the one under test.
function Get-RawUserPath {
    $key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Environment')
    try { [string]$key.GetValue('Path', '', [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames) }
    finally { $key.Close() }
}
function Get-TextHash([string]$text) {
    [BitConverter]::ToString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($text))).Replace('-', '')
}
# Existence, kind and raw value, like user_state.ps1: a restore that recreated a missing value as
# empty, or wrote the right text as REG_SZ, must not pass the check.
function Get-PathFingerprint {
    $key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Environment')
    try {
        $exists = $key.GetValueNames() -contains 'Path'
        $kind = if ($exists) { "$($key.GetValueKind('Path'))" } else { '' }
        $raw = if ($exists) { [string]$key.GetValue('Path', '', [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames) } else { '' }
    } finally { $key.Close() }
    Get-TextHash "$exists|$kind|$raw"
}

# One run per machine: two runs would each save the other's test PATH as "the user's" and the
# last restore would leave a test entry behind. Nothing below touches PATH before the lock is held.
$mutex = [Threading.Mutex]::new($false, $LockName)
$acquired = $false
try {
    $acquired = $mutex.WaitOne(0)
    if (-not $acquired) {
        Write-Host "[i] another installer test holds $LockName; waiting up to $LockTimeoutSeconds s"
        $acquired = $mutex.WaitOne([TimeSpan]::FromSeconds($LockTimeoutSeconds))
    }
} catch {
    $inner = $_.Exception
    while ($inner -and $inner -isnot [Threading.AbandonedMutexException]) { $inner = $inner.InnerException }
    if (-not $inner) { throw }
    # The lock is ours now. Its previous holder died without releasing it; if it had changed PATH,
    # its recovery file is still there and the check below refuses to run.
    $acquired = $true
    Write-Host '[!] the previous installer test died while holding the lock; checking for its recovery file'
}
if (-not $acquired) {
    $mutex.Dispose()
    throw "Another shell-panel installer test is running on this machine (lock $LockName not acquired within $LockTimeoutSeconds s). PATH was not touched."
}

try {
if (Test-Path -LiteralPath $RecoveryFile) {
    $left = Get-Content -LiteralPath $RecoveryFile -Raw | ConvertFrom-Json
    $restore = if ($left.exists) {
        "`$k = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Environment', `$true); `$k.SetValue('Path', (Get-Content -LiteralPath '$($RecoveryFile -replace "'", "''")' -Raw | ConvertFrom-Json).raw, '$($left.kind)'); `$k.Close()"
    } else {
        "`$k = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Environment', `$true); `$k.DeleteValue('Path', `$false); `$k.Close()"
    }
    $what = if ($left.exists) { "a value of kind $($left.kind)" } else { 'no value (Path did not exist)' }
    Write-Host "[!] A previous installer test was killed before it restored your user PATH."
    Write-Host "    Its recovery file $RecoveryFile holds the PATH it saved: $what."
    Write-Host '    Check it, then restore it from PowerShell 7 with:'
    Write-Host "    $restore"
    Write-Host "    and delete $RecoveryFile. This run changed nothing."
    throw "Refusing to run: a previous installer test left $RecoveryFile (its PATH was not restored)."
}

$envKey = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Environment', $true)
$hadPath = $envKey.GetValueNames() -contains 'Path'
$savedPath = $envKey.GetValue('Path', $null, [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
$savedKind = if ($hadPath) { $envKey.GetValueKind('Path') } else { $null }
$pathHashBefore = Get-PathFingerprint
Write-Host "PATH fingerprint before: $pathHashBefore"
$work = $null
$script:passed = 0

function Assert([bool]$condition, [string]$message) {
    if (-not $condition) { throw "FAIL: $message" }
    $script:passed++
    Write-Host "[OK] $message"
}

function Invoke-Script([string]$name, [string[]]$arguments) {
    $script:lastOutput = @(& $Shell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $root $name) @arguments 2>&1 |
        ForEach-Object { "$_" })
    $script:lastOutput | ForEach-Object { Write-Host "    $_" }
    $LASTEXITCODE
}

# Styled output carries emoji: a child writing to a pipe uses its console code page (OEM on 5.1),
# which cannot hold them. This runs the script with UTF-8 output on both sides of the pipe.
# An element that starts with '-' is passed as a parameter name; every other element is a value
# and is single-quoted, so no value may start with '-' (none of this harness's paths do).
function Invoke-Utf8([string]$name, [string[]]$arguments) {
    $quoted = $arguments | ForEach-Object { if ($_ -like '-*') { $_ } else { "'" + ($_ -replace "'", "''") + "'" } }
    $scriptPath = (Join-Path $root $name) -replace "'", "''"
    $command = "[Console]::OutputEncoding = [Text.UTF8Encoding]::new(); & '$scriptPath' $($quoted -join ' ')"
    $saved = [Console]::OutputEncoding
    [Console]::OutputEncoding = [Text.UTF8Encoding]::new()
    try { Invoke-CommandLine $command } finally { [Console]::OutputEncoding = $saved }
}

# For argument forms -File cannot express, such as an explicit -Purge:$false.
function Invoke-CommandLine([string]$command) {
    $script:lastOutput = @(& $Shell -NoProfile -ExecutionPolicy Bypass -Command $command 2>&1 | ForEach-Object { "$_" })
    $script:lastOutput | ForEach-Object { Write-Host "    $_" }
    $LASTEXITCODE
}

function Get-PathCount {
    @((Get-RawUserPath) -split ';' | Where-Object { $_.TrimEnd('\') -ieq $installDir.TrimEnd('\') }).Count
}

try {
    $root = Split-Path -Parent $PSScriptRoot
    $version = (Select-String -LiteralPath (Join-Path $root 'Cargo.toml') -Pattern '^version\s*=\s*"([^"]+)"' |
        Select-Object -First 1).Matches[0].Groups[1].Value
    $work = Join-Path ([IO.Path]::GetTempPath()) ('sp-installer-test-' + [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $work | Out-Null
    $installDir = Join-Path $work 'Programs\shell-panel'
    $fragmentDir = Join-Path $work 'Fragments\shell-panel'

    # Start from a PATH that holds an unexpanded variable, the case a careless edit destroys.
    $marker = '%SP_INSTALLER_TEST%\bin'
    $seed = if ($hadPath) { [string]$savedPath + ';' + $marker } else { $marker }
    # Written before the first PATH change: if this process is killed, the next run finds it.
    [ordered]@{ exists = $hadPath; kind = "$savedKind"; raw = [string]$savedPath; pid = $PID; started = (Get-Date).ToString('o') } |
        ConvertTo-Json | Set-Content -LiteralPath $RecoveryFile -Encoding utf8
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

    Write-Host '== one run per machine'
    # This run holds the lock, so a second run must give up without touching PATH. The child gets
    # its own recovery file and a zip that does not exist, so that without the lock it would fail
    # fast (and restore what it changed) instead of running the whole test.
    $pwsh = (Get-Process -Id $PID).Path
    $missingZip = Join-Path $work 'missing.zip'
    $childRecovery = Join-Path $work 'child.recovery.json'
    $fingerprint = Get-PathFingerprint
    $out = @(& $pwsh -NoProfile -File $PSCommandPath -LockTimeoutSeconds 1 -RecoveryFile $childRecovery -ZipPath $missingZip 2>&1 | ForEach-Object { "$_" })
    $code = $LASTEXITCODE
    $out | ForEach-Object { Write-Host "    $_" }
    Assert ($code -ne 0) 'a second run while this one holds the lock fails'
    Assert (($out -join "`n") -match 'Another shell-panel installer test is running') 'the second run says another test holds the lock'
    Assert ((Get-PathFingerprint) -eq $fingerprint) 'the second run leaves PATH alone'

    Write-Host '== leftover recovery file'
    # A run that was killed leaves its recovery file. The child takes a lock of its own (this run
    # holds the real one) and points at a fake leftover, so the user's real file is never read.
    $fake = Join-Path $work 'leftover.recovery.json'
    [ordered]@{ exists = $true; kind = 'ExpandString'; raw = 'C:\sp-installer-test-leftover'; pid = 0; started = '' } |
        ConvertTo-Json | Set-Content -LiteralPath $fake -Encoding utf8
    $fakeBytes = [IO.File]::ReadAllBytes($fake)
    $out = @(& $pwsh -NoProfile -File $PSCommandPath -LockName ('Local\sp-installer-test-' + [guid]::NewGuid().ToString('N')) -RecoveryFile $fake -ZipPath $missingZip 2>&1 | ForEach-Object { "$_" })
    $code = $LASTEXITCODE
    $out | ForEach-Object { Write-Host "    $_" }
    Assert ($code -ne 0) 'a run that finds a recovery file fails'
    Assert (($out -join "`n") -match 'was killed before it restored your user PATH') 'it says a previous run was killed'
    Assert (($out -join "`n") -match "SetValue\('Path'.+'ExpandString'\)") 'it prints the command that restores the saved value and kind'
    Assert ((Get-PathFingerprint) -eq $fingerprint) 'it leaves PATH alone'
    Assert ((Test-Path -LiteralPath $fake) -and ([Convert]::ToHexString([IO.File]::ReadAllBytes($fake)) -eq [Convert]::ToHexString($fakeBytes))) 'it keeps the recovery file as it was'

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

    Write-Host '== release lookup failure'
    # -ApiUri points the latest-release lookup at a closed local port: the failure must say what
    # to do, since the usual real cause (the GitHub API rate limit) cannot be reproduced here.
    $lookup = "& '$(Join-Path $root 'install.ps1')' -InstallDir '$installDir' -TerminalFragmentDir '$fragmentDir' -ApiUri 'http://127.0.0.1:9/'"
    Assert ((Invoke-CommandLine $lookup) -ne 0) 'a failed release lookup fails the install'
    Assert (($script:lastOutput -join "`n") -match '-Version') 'the lookup failure suggests -Version'
    Assert (($script:lastOutput -join "`n") -match '(?s)release:\s*\(.+\)\s*Pass\s*-Version') 'the lookup failure keeps the original connection error (its text is localized and wrapped, so only its shape is checked)'
    Assert ((Get-PathCount) -eq 0 -and -not (Test-Path -LiteralPath $fragmentDir)) 'a failed release lookup changes nothing (PATH, Terminal fragment)'

    Write-Host '== zip missing from SHA256SUMS.txt'
    $other = Join-Path $work 'OTHER_SHA256SUMS.txt'
    (('1' * 64) + '  some-other-file.zip') | Set-Content -LiteralPath $other -Encoding ascii
    $noEntry = @('-InstallDir', $installDir, '-TerminalFragmentDir', $fragmentDir, '-ZipPath', $ZipPath, '-ChecksumsPath', $other)
    Assert ((Invoke-Script 'install.ps1' $noEntry) -ne 0) 'a SHA256SUMS.txt without the zip line fails the install'
    Assert (-not (Test-Path -LiteralPath $installedExe)) 'a missing checksum line installs nothing'
    Assert ((Get-PathCount) -eq 0) 'a missing checksum line leaves PATH alone'

    Write-Host '== PATH stored as REG_SZ'
    # The uninstall test left a user file here, which would make install refuse the folder.
    Remove-Item -LiteralPath (Join-Path $installDir 'notes.txt') -Force
    $envKey.SetValue('Path', 'C:\sp-installer-test-plain', [Microsoft.Win32.RegistryValueKind]::String)
    Assert ((Invoke-Script 'install.ps1' $install) -eq 0) 'install exits 0 with a REG_SZ PATH'
    Assert ($envKey.GetValueKind('Path') -eq [Microsoft.Win32.RegistryValueKind]::String) 'a REG_SZ PATH stays REG_SZ'
    Assert (((Get-RawUserPath) -split ';') -contains 'C:\sp-installer-test-plain') 'the REG_SZ PATH keeps its entry'
    Assert ((Get-PathCount) -eq 1) 'the install dir is added to the REG_SZ PATH'
    Assert ((Invoke-Script 'uninstall.ps1' $uninstall) -eq 0) 'uninstall exits 0 with a REG_SZ PATH'
    Assert ($envKey.GetValueKind('Path') -eq [Microsoft.Win32.RegistryValueKind]::String) 'a REG_SZ PATH is still REG_SZ after uninstall'
    $rawNow = Get-RawUserPath
    Assert ($rawNow -eq 'C:\sp-installer-test-plain') "uninstall restores the REG_SZ PATH value (got [$rawNow])"

    Write-Host '== PATH value missing'
    $envKey.DeleteValue('Path', $false)
    Assert ((Invoke-Script 'install.ps1' $install) -eq 0) 'install exits 0 without a PATH value'
    Assert (($envKey.GetValueNames() -contains 'Path') -and $envKey.GetValueKind('Path') -eq [Microsoft.Win32.RegistryValueKind]::ExpandString) 'a missing PATH is created as REG_EXPAND_SZ'
    Assert ((Get-RawUserPath) -eq $installDir) 'the created PATH holds only the install dir'
    Assert ((Invoke-Script 'uninstall.ps1' $uninstall) -eq 0) 'uninstall exits 0 after creating the PATH'
    # Documented behaviour: the value stays, empty, instead of being deleted.
    Assert (($envKey.GetValueNames() -contains 'Path') -and (Get-RawUserPath) -eq '') 'uninstall leaves the created PATH empty'

    Write-Host '== styled output'
    # NO_COLOR from the caller's environment would turn every styled case below into a plain one.
    $savedNoColor = $env:NO_COLOR
    $env:NO_COLOR = $null
    try {
    $esc = [string][char]0x1b
    $check = [char]::ConvertFromUtf32(0x2705)
    Assert ((Invoke-Utf8 'install.ps1' ($install + '-ForceStyle')) -eq 0) 'install -ForceStyle exits 0'
    $styled = $script:lastOutput -join "`n"
    Assert ($styled.Contains("$esc[1m")) '-ForceStyle output has bold'
    Assert ($styled.Contains("$esc[3m")) '-ForceStyle output has italic'
    Assert ($styled.Contains("$esc[32m")) '-ForceStyle output has green'
    Assert ($styled.Contains($check)) '-ForceStyle output has the done icon'
    Assert ((Get-PathCount) -eq 1) '-ForceStyle install still puts the install dir on PATH'

    # A command span inside a coloured line ends with the line's colour again, not the default.
    Assert ($styled.Contains("$esc[22;39m$esc[32m written")) 'text after a command span keeps the line colour (install)'

    $failIcon = [char]::ConvertFromUtf32(0x274C)
    Assert ((Invoke-Utf8 'install.ps1' ($tampered + '-ForceStyle')) -ne 0) 'a styled install failure still fails'
    $failed = $script:lastOutput -join "`n"
    # PowerShell's own error report carries the message (and may quote the throw line, so the text
    # is not counted); the styled line is a headline and must not repeat it.
    $failLines = @($script:lastOutput | Where-Object { $_.Contains($failIcon) })
    Assert ($failLines.Count -eq 1 -and $failLines[0].Contains("$failIcon Install failed")) 'a styled install failure shows one failure headline'
    Assert (-not $failLines[0].Contains('Checksum mismatch')) 'the styled headline does not repeat the error message'
    Assert ($failed.Contains("Checksum mismatch for $zipName")) 'the error message is still reported'

    $env:NO_COLOR = '1'
    try { $code = Invoke-Utf8 'install.ps1' ($install + '-ForceStyle') } finally { $env:NO_COLOR = $null }
    $plain = $script:lastOutput -join "`n"
    Assert ($code -eq 0) 'install -ForceStyle with NO_COLOR exits 0'
    Assert (-not $plain.Contains($esc)) 'NO_COLOR wins over -ForceStyle (no ESC)'
    Assert ($plain.Contains('[OK]')) 'NO_COLOR keeps the plain [OK] markers'

    Assert ((Invoke-Script 'install.ps1' $install) -eq 0) 'default install exits 0'
    Assert (-not ($script:lastOutput -join "`n").Contains($esc)) 'redirected output without -ForceStyle has no ESC'

    foreach ($f in 'install.ps1', 'uninstall.ps1', 'scripts\test-installer.ps1', '.claude\skills\shell-panel-review\scripts\user_state.ps1') {
        $bytes = [IO.File]::ReadAllBytes((Join-Path $root $f))
        Assert (@($bytes | Where-Object { $_ -ge 0x80 }).Count -eq 0) "$f is pure ASCII"
    }

    Assert ((Invoke-Utf8 'uninstall.ps1' ($uninstall + '-ForceStyle')) -eq 0) 'uninstall -ForceStyle exits 0'
    $ustyled = $script:lastOutput -join "`n"
    Assert ($ustyled.Contains("$esc[1m" + 'shell-panel uninstaller')) 'uninstall -ForceStyle shows the styled header'
    Assert ($ustyled.Contains($check)) 'uninstall -ForceStyle has the done icon'
    Assert ($ustyled.Contains("-Purge$esc[22;39m$esc[33m to remove it.")) 'text after a command span keeps the line colour (uninstall note)'

    $sharedUninstall = @('-InstallDir', $shared, '-TerminalFragmentDir', $fragmentDir, '-ForceStyle')
    Assert ((Invoke-Utf8 'uninstall.ps1' $sharedUninstall) -ne 0) 'a styled uninstall failure still fails'
    $failed = $script:lastOutput -join "`n"
    $failLines = @($script:lastOutput | Where-Object { $_.Contains($failIcon) })
    Assert ($failLines.Count -eq 1 -and $failLines[0].Contains("$failIcon Uninstall failed")) 'a styled uninstall failure shows one failure headline'
    Assert (-not $failLines[0].Contains('was not created')) 'the styled uninstall headline does not repeat the error message'
    Assert ($failed -match 'was not created by the shell-panel') 'the uninstall error message is still reported'
    } finally {
        # Back to exactly what the caller had: absent stays absent.
        $env:NO_COLOR = $savedNoColor
    }

    Write-Host "test result: ok. $script:passed passed; 0 failed"
    # The last child exited 1 on purpose (a styled failure test). A host that ends with
    # `exit $LASTEXITCODE`, like GitHub's pwsh steps, would report that as the run's result.
    $global:LASTEXITCODE = 0
} catch {
    Write-Host "[!] $($_.Exception.Message)"
    Write-Host "test result: FAILED. $script:passed passed; 1 failed"
    throw
} finally {
    # Every step on its own: one failure must not skip the restore.
    $ErrorActionPreference = 'Continue'
    try {
        if ($hadPath) { $envKey.SetValue('Path', $savedPath, $savedKind) } else { $envKey.DeleteValue('Path', $false) }
    } catch { Write-Host "[!] could not restore PATH: $($_.Exception.Message)" }
    try { $envKey.Close() } catch { }
    # The last broadcast came from uninstall.ps1 while PATH still held the test entry: broadcast
    # again so Explorer hands new processes the restored PATH (same trick as install.ps1).
    try {
        [Environment]::SetEnvironmentVariable('SHELL_PANEL_INSTALLER', '1', 'User')
        [Environment]::SetEnvironmentVariable('SHELL_PANEL_INSTALLER', $null, 'User')
    } catch { Write-Host "[!] could not broadcast: $($_.Exception.Message)" }
    if ($work) { try { Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue } catch { } }
    $pathHashAfter = try { Get-PathFingerprint } catch { "unreadable: $($_.Exception.Message)" }
    Write-Host "PATH fingerprint after:  $pathHashAfter"
    if ($pathHashAfter -ne $pathHashBefore) {
        # The recovery file stays: it holds the value to restore, and the next run refuses to start.
        Write-Host "[!] PATH LEAK: before $pathHashBefore, after $pathHashAfter. The saved value is in $RecoveryFile."
        throw 'The user PATH differs from what it was before the test.'
    }
    Remove-Item -LiteralPath $RecoveryFile -Force -ErrorAction SilentlyContinue
    Write-Host '[OK] PATH is identical to before the test (existence, kind and raw value)'
}
} finally {
    # Held from before PATH was saved until after it was restored and checked.
    try { $mutex.ReleaseMutex() } catch { Write-Host "[!] could not release ${LockName}: $($_.Exception.Message)" }
    $mutex.Dispose()
}
