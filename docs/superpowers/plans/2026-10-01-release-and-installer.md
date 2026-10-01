# Release Workflows and PowerShell Installer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** One-line install of shell-panel from GitHub releases, and a tag-driven release pipeline that tests the installer before publishing.

**Architecture:** `install.ps1` / `uninstall.ps1` at the repository root are standalone scripts (each is downloaded on its own) whose whole body is a script block invoked with `@args`, so they work both piped through `iex` and run as files without leaking anything into the caller's session. `scripts/test-installer.ps1` drives both against a real zip in temporary directories. Two GitHub Actions workflows: `ci.yml` (gate + installer test) and `release.yml` (version check, gate, matrix build, installer test, publish with `gh`).

**Tech Stack:** Windows PowerShell 5.1 and PowerShell 7, GitHub Actions on `windows-latest`, `gh` CLI, Cargo targets `x86_64-pc-windows-msvc` and `aarch64-pc-windows-msvc`.

**Spec:** `docs/superpowers/specs/2026-10-01-release-and-installer-design.md`

## Global Constraints

- Scripts run on Windows PowerShell 5.1 **and** PowerShell 7; every script step is exercised on both.
- `install.ps1` and `uninstall.ps1` never call `exit` and never leave variables, functions, preferences or strict mode in the caller's session (they are piped into `iex`). Errors are `throw`n; run with `-File`, a throw gives exit code 1.
- Console output is plain ASCII with markers `[*]`, `[OK]`, `[!]`, `[i]` — a CP-850 console mangles other symbols.
- Repository: `jonyduque/shell-panel`. Asset names: `shell-panel-<version>-<arch>.zip` with arch `x64` / `arm64`, plus `SHA256SUMS.txt` (`<lowercase sha256>  <file name>` per line).
- Default install dir `%LOCALAPPDATA%\Programs\shell-panel`; fragment dir `%LOCALAPPDATA%\Microsoft\Windows Terminal\Fragments\shell-panel`; profile name `PowerShell (shell-panel)`; profile GUID `{b6f3a6a8-5d0e-4c55-9a5a-3e7c1f2d9b41}`.
- The user `PATH` is edited in `HKCU\Environment` with `DoNotExpandEnvironmentNames`, preserving the value kind; never through `[Environment]::SetEnvironmentVariable('Path', ...)` (it turns `REG_EXPAND_SZ` into `REG_SZ`).
- Workflows use only `actions/checkout`, `actions/upload-artifact`, `actions/download-artifact` and the preinstalled `gh`. `permissions: contents: read` everywhere except the publish job (`contents: write`).
- Tests of the installer never touch the developer's real install dir, real fragment dir, or (after the run) their `PATH`.

## Review Focus

1. A `PATH` that holds `%VAR%` entries (REG_EXPAND_SZ) — must stay REG_EXPAND_SZ with the variables unexpanded after install and uninstall. Pinned in Task 1 (`test-installer.ps1` asserts the value kind is unchanged).
2. Reinstalling over an existing install — must not duplicate the PATH entry nor the Terminal profile. Pinned in Task 1 (reinstall step).
3. A corrupted or tampered download — must abort before touching the installed binary. Pinned in Task 1 (tampered checksum step).
4. `-InstallDir` pointing at a folder that is not a shell-panel install — uninstall must refuse to delete it. Pinned in Task 1 (foreign directory step).
5. Pushing a tag that does not match `Cargo.toml` — the release must fail before building. Pinned in Task 2 (version step, checked with a local dry run of the same PowerShell).

---

### Task 1: Installer, uninstaller and their end-to-end test

**Files:**
- Create: `scripts/test-installer.ps1`
- Create: `install.ps1`
- Create: `uninstall.ps1`

**Interfaces:**
- Produces: `install.ps1 [-Version <x.y.z>] [-InstallDir <dir>] [-ZipPath <zip> -ChecksumsPath <sums>] [-TerminalFragmentDir <dir>]`; `uninstall.ps1 [-InstallDir <dir>] [-Purge] [-TerminalFragmentDir <dir>]`; `scripts/test-installer.ps1 [-ZipPath <zip>] [-Shell pwsh|powershell]`, which prints a final cargo-style line `test result: ok. N passed; 0 failed` or `test result: FAILED. N passed; 1 failed` (so `mutate.ps1` can drive it) and exits non-zero on failure.

- [ ] **Step 1: Write the end-to-end test**

Create `scripts/test-installer.ps1`:

```powershell
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
```

- [ ] **Step 2: Run it to see it fail**

Run: `cargo build --release; pwsh -NoProfile -File scripts/test-installer.ps1 -Shell pwsh`
Expected: `[!] FAIL: install exits 0` (install.ps1 does not exist yet), `test result: FAILED. 0 passed; 1 failed`, exit 1. Afterwards `(Get-ItemProperty HKCU:\Environment).Path` equals what it was before the run.

- [ ] **Step 3: Write `install.ps1`**

```powershell
<#
.SYNOPSIS
    Installs shell-panel from its GitHub releases.
.DESCRIPTION
    irm https://github.com/jonyduque/shell-panel/releases/latest/download/install.ps1 | iex

    Downloads the release zip for this machine's architecture, checks it against the release's
    SHA256SUMS.txt, installs shell-panel.exe into -InstallDir, adds that directory to the user
    PATH and writes a Windows Terminal profile. Windows PowerShell 5.1 and PowerShell 7.
.PARAMETER Version
    Version to install, e.g. 0.2.0. Default: the latest release.
.PARAMETER InstallDir
    Default: %LOCALAPPDATA%\Programs\shell-panel.
.PARAMETER ZipPath
    Testing only: install this local zip (requires -ChecksumsPath) instead of downloading.
.PARAMETER TerminalFragmentDir
    Testing only: where the Windows Terminal profile fragment is written.
#>
# The body is a script block invoked with the script's arguments: piped through `iex` a script
# runs in the caller's scope, and its variables, functions and preferences would stay behind in
# the user's session. A child scope keeps them out, and `throw` (never `exit`) ends it.
& {
    [CmdletBinding()]
    param(
        [string]$Version,
        [string]$InstallDir = (Join-Path $env:LOCALAPPDATA 'Programs\shell-panel'),
        [string]$ZipPath,
        [string]$ChecksumsPath,
        [string]$TerminalFragmentDir = (Join-Path $env:LOCALAPPDATA 'Microsoft\Windows Terminal\Fragments\shell-panel')
    )
    Set-StrictMode -Version Latest
    $ErrorActionPreference = 'Stop'
    # The progress bar makes Invoke-WebRequest many times slower on Windows PowerShell 5.1.
    $ProgressPreference = 'SilentlyContinue'

    $repo = 'jonyduque/shell-panel'
    # Fixed, so that reinstalling updates the same Windows Terminal profile.
    $profileGuid = '{b6f3a6a8-5d0e-4c55-9a5a-3e7c1f2d9b41}'

    function Get-Arch {
        # A 32-bit PowerShell on a 64-bit Windows reports x86 here and the real one in W6432.
        $arch = if ($env:PROCESSOR_ARCHITEW6432) { $env:PROCESSOR_ARCHITEW6432 } else { $env:PROCESSOR_ARCHITECTURE }
        switch ($arch) {
            'AMD64' { return 'x64' }
            'ARM64' { return 'arm64' }
            default { throw "shell-panel has no build for processor architecture '$arch' (only x64 and ARM64)." }
        }
    }

    function Get-ExpectedHash([string]$sumsFile, [string]$fileName) {
        foreach ($line in Get-Content -LiteralPath $sumsFile) {
            $parts = $line.Trim() -split '\s+', 2
            if ($parts.Count -eq 2 -and $parts[1].TrimStart('*') -eq $fileName) { return $parts[0] }
        }
        throw "SHA256SUMS.txt has no entry for $fileName. Nothing was installed."
    }

    function Send-EnvironmentChange {
        # SetEnvironmentVariable broadcasts WM_SETTINGCHANGE, so new terminals see the new PATH.
        # It is used on a throwaway variable: writing Path through it would turn a REG_EXPAND_SZ
        # value into REG_SZ and freeze every %VARIABLE% in it.
        [Environment]::SetEnvironmentVariable('SHELL_PANEL_INSTALLER', '1', 'User')
        [Environment]::SetEnvironmentVariable('SHELL_PANEL_INSTALLER', $null, 'User')
    }

    function Add-UserPath([string]$dir) {
        $want = $dir.TrimEnd('\')
        $key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Environment', $true)
        try {
            $raw = [string]$key.GetValue('Path', '', [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
            $kind = if ($key.GetValueNames() -contains 'Path') { $key.GetValueKind('Path') } else { [Microsoft.Win32.RegistryValueKind]::ExpandString }
            $present = @($raw -split ';' | Where-Object { [Environment]::ExpandEnvironmentVariables($_).TrimEnd('\') -ieq $want })
            if ($present.Count -eq 0) {
                $new = if ($raw.Trim(';')) { $raw.TrimEnd(';') + ';' + $dir } else { $dir }
                $key.SetValue('Path', $new, $kind)
                Send-EnvironmentChange
            }
        } finally { $key.Close() }
        if (@($env:Path -split ';' | Where-Object { $_.TrimEnd('\') -ieq $want }).Count -eq 0) {
            $env:Path = $env:Path.TrimEnd(';') + ';' + $dir
        }
    }

    function Write-TerminalFragment([string]$exePath) {
        New-Item -ItemType Directory -Force -Path $TerminalFragmentDir | Out-Null
        $fragment = @{
            profiles = @(
                @{
                    name        = 'PowerShell (shell-panel)'
                    commandline = '"' + $exePath + '"'
                    icon        = 'ms-appx:///ProfileIcons/pwsh.png'
                    guid        = $profileGuid
                }
            )
        }
        $json = ConvertTo-Json -InputObject $fragment -Depth 4
        # Windows Terminal reads fragments as UTF-8; Set-Content on 5.1 would add a BOM or use ANSI.
        [IO.File]::WriteAllText((Join-Path $TerminalFragmentDir 'shell-panel.json'), $json, (New-Object System.Text.UTF8Encoding $false))
    }

    $arch = Get-Arch
    $work = Join-Path ([IO.Path]::GetTempPath()) ('shell-panel-install-' + [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $work | Out-Null
    try {
        if ($ZipPath) {
            if (-not $ChecksumsPath) { throw '-ChecksumsPath is required with -ZipPath.' }
            $zip = (Resolve-Path -LiteralPath $ZipPath).Path
            $sums = (Resolve-Path -LiteralPath $ChecksumsPath).Path
        } else {
            [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
            if (-not $Version) {
                Write-Host '[*] Looking up the latest release'
                $Version = (Invoke-RestMethod -UseBasicParsing -Uri "https://api.github.com/repos/$repo/releases/latest").tag_name
            }
            $Version = $Version -replace '^v', ''
            $name = "shell-panel-$Version-$arch.zip"
            $base = "https://github.com/$repo/releases/download/v$Version"
            $zip = Join-Path $work $name
            $sums = Join-Path $work 'SHA256SUMS.txt'
            Write-Host "[*] Downloading $name"
            Invoke-WebRequest -UseBasicParsing -Uri "$base/$name" -OutFile $zip
            Invoke-WebRequest -UseBasicParsing -Uri "$base/SHA256SUMS.txt" -OutFile $sums
        }

        $zipName = Split-Path -Leaf $zip
        $expected = Get-ExpectedHash $sums $zipName
        $actual = (Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash
        if ($actual -ine $expected) {
            throw "Checksum mismatch for ${zipName}: expected $expected, got $actual. Nothing was installed."
        }
        Write-Host '[OK] Checksum verified'

        $extract = Join-Path $work 'files'
        Expand-Archive -LiteralPath $zip -DestinationPath $extract
        $exeSource = Join-Path $extract 'shell-panel.exe'
        if (-not (Test-Path -LiteralPath $exeSource)) { throw "$zipName does not contain shell-panel.exe. Nothing was installed." }

        New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
        $exe = Join-Path $InstallDir 'shell-panel.exe'
        try {
            Copy-Item -LiteralPath $exeSource -Destination $exe -Force
        } catch {
            throw "Could not replace $exe ($($_.Exception.Message)). Close every shell-panel session and run the installer again."
        }
        foreach ($doc in 'LICENSE', 'README.md') {
            $docSource = Join-Path $extract $doc
            if (Test-Path -LiteralPath $docSource) { Copy-Item -LiteralPath $docSource -Destination $InstallDir -Force }
        }
        Write-Host "[OK] Installed $exe"

        Add-UserPath $InstallDir
        Write-Host "[OK] $InstallDir is on your user PATH"

        Write-TerminalFragment $exe
        Write-Host '[OK] Windows Terminal profile "PowerShell (shell-panel)" written'

        $installed = & $exe --version
        Write-Host "[OK] $installed"
        Write-Host ''
        Write-Host '[i] Open the "PowerShell (shell-panel)" profile in Windows Terminal, or run shell-panel in a new terminal window.'
    } finally {
        Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue
    }
} @args
```

- [ ] **Step 4: Write `uninstall.ps1`**

```powershell
<#
.SYNOPSIS
    Removes shell-panel installed by install.ps1.
.DESCRIPTION
    Removes the install directory, its entry in the user PATH and the Windows Terminal profile.
    Your configuration (%USERPROFILE%\.config\shell-panel.toml and .config\shell-panel\) is kept
    unless -Purge is given. Windows PowerShell 5.1 and PowerShell 7.
.PARAMETER InstallDir
    Default: %LOCALAPPDATA%\Programs\shell-panel.
.PARAMETER Purge
    Also delete the configuration file and the custom specs directory.
.PARAMETER TerminalFragmentDir
    Testing only: where the Windows Terminal profile fragment was written.
#>
# Same structure as install.ps1: a script block in a child scope, so `iex` leaves nothing behind.
& {
    [CmdletBinding()]
    param(
        [string]$InstallDir = (Join-Path $env:LOCALAPPDATA 'Programs\shell-panel'),
        [switch]$Purge,
        [string]$TerminalFragmentDir = (Join-Path $env:LOCALAPPDATA 'Microsoft\Windows Terminal\Fragments\shell-panel')
    )
    Set-StrictMode -Version Latest
    $ErrorActionPreference = 'Stop'

    function Send-EnvironmentChange {
        # See install.ps1: broadcast WM_SETTINGCHANGE without rewriting Path as REG_SZ.
        [Environment]::SetEnvironmentVariable('SHELL_PANEL_INSTALLER', '1', 'User')
        [Environment]::SetEnvironmentVariable('SHELL_PANEL_INSTALLER', $null, 'User')
    }

    function Remove-UserPath([string]$dir) {
        $unwanted = $dir.TrimEnd('\')
        $key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Environment', $true)
        try {
            if ($key.GetValueNames() -contains 'Path') {
                $raw = [string]$key.GetValue('Path', '', [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
                $kind = $key.GetValueKind('Path')
                $entries = @($raw -split ';')
                $kept = @($entries | Where-Object { [Environment]::ExpandEnvironmentVariables($_).TrimEnd('\') -ine $unwanted })
                if ($kept.Count -ne $entries.Count) {
                    $key.SetValue('Path', ($kept -join ';'), $kind)
                    Send-EnvironmentChange
                }
            }
        } finally { $key.Close() }
        $env:Path = (@($env:Path -split ';' | Where-Object { $_.TrimEnd('\') -ine $unwanted }) -join ';')
    }

    if (Test-Path -LiteralPath $InstallDir) {
        # Never delete a directory that is not a shell-panel install: -InstallDir is user input.
        if (-not (Test-Path -LiteralPath (Join-Path $InstallDir 'shell-panel.exe'))) {
            throw "$InstallDir does not contain shell-panel.exe; it does not look like a shell-panel install. Nothing was removed."
        }
        try {
            Remove-Item -LiteralPath $InstallDir -Recurse -Force
        } catch {
            throw "Could not remove $InstallDir ($($_.Exception.Message)). Close every shell-panel session and run the uninstaller again."
        }
        Write-Host "[OK] Removed $InstallDir"
    } else {
        Write-Host "[i] $InstallDir does not exist"
    }

    Remove-UserPath $InstallDir
    Write-Host '[OK] Removed from the user PATH'

    if (Test-Path -LiteralPath $TerminalFragmentDir) {
        Remove-Item -LiteralPath $TerminalFragmentDir -Recurse -Force
        Write-Host '[OK] Removed the Windows Terminal profile'
    }

    $config = Join-Path $env:USERPROFILE '.config\shell-panel.toml'
    $specs = Join-Path $env:USERPROFILE '.config\shell-panel'
    if ($Purge) {
        foreach ($item in $config, $specs) {
            if (Test-Path -LiteralPath $item) {
                Remove-Item -LiteralPath $item -Recurse -Force
                Write-Host "[OK] Removed $item"
            }
        }
    } else {
        Write-Host "[i] Kept your configuration ($config and $specs); run with -Purge to remove it."
    }
} @args
```

- [ ] **Step 5: Run the test on both shells**

Run:
```
pwsh -NoProfile -File scripts/test-installer.ps1 -Shell pwsh
pwsh -NoProfile -File scripts/test-installer.ps1 -Shell powershell
```
Expected for both: every `[OK]` line, `test result: ok. 21 passed; 0 failed`, exit 0. Then `(Get-ItemProperty HKCU:\Environment).Path` is byte-identical to before the runs.

- [ ] **Step 6: Prove the test can fail (mutation proofs)**

Each must print `[OK] MORTA` and exit 0:
```
pwsh -NoProfile -File .claude/skills/shell-panel-review/scripts/mutate.ps1 -Path install.ps1 -Anchor '            if ($present.Count -eq 0) {' -Replacement '            if ($true) {' -TestCommand 'pwsh -NoProfile -File scripts/test-installer.ps1 -Shell pwsh'
pwsh -NoProfile -File .claude/skills/shell-panel-review/scripts/mutate.ps1 -Path install.ps1 -Anchor '        if ($actual -ine $expected) {' -Replacement '        if ($false) {' -TestCommand 'pwsh -NoProfile -File scripts/test-installer.ps1 -Shell pwsh'
pwsh -NoProfile -File .claude/skills/shell-panel-review/scripts/mutate.ps1 -Path install.ps1 -Anchor '                $key.SetValue(''Path'', $new, $kind)' -Replacement '                $key.SetValue(''Path'', $new)' -TestCommand 'pwsh -NoProfile -File scripts/test-installer.ps1 -Shell pwsh'
pwsh -NoProfile -File .claude/skills/shell-panel-review/scripts/mutate.ps1 -Path uninstall.ps1 -Anchor '        if (-not (Test-Path -LiteralPath (Join-Path $InstallDir ''shell-panel.exe''))) {' -Replacement '        if ($false) {' -TestCommand 'pwsh -NoProfile -File scripts/test-installer.ps1 -Shell pwsh'
```
(`mutate.ps1` recognises the run through the `test result:` line; a FAILED line with a non-zero exit is MORTA.)

- [ ] **Step 7: Check that `iex` leaves nothing behind**

Run under both shells:
```
pwsh -NoProfile -Command "iex (Get-Content uninstall.ps1 -Raw) *> `$null; 'leak=' + [bool](Get-Variable InstallDir -ErrorAction Ignore) + ' fn=' + [bool](Get-Command Remove-UserPath -ErrorAction Ignore) + ' eap=' + `$ErrorActionPreference"
```
and the same with `powershell`. Expected: `leak=False fn=False eap=Continue`. (`uninstall.ps1` with defaults on a machine without an install only prints `[i]` lines and removes nothing.)

- [ ] **Step 8: Commit**

```bash
git add install.ps1 uninstall.ps1 scripts/test-installer.ps1
git commit -m "feat: add the PowerShell installer and uninstaller with an end-to-end test"
```

---

### Task 2: CI and release workflows

**Files:**
- Create: `.github/workflows/ci.yml`
- Create: `.github/workflows/release.yml`

**Interfaces:**
- Consumes: `.claude/skills/shell-panel-review/scripts/verify.ps1` (gate, exit 0/1), `scripts/test-installer.ps1 -ZipPath <zip> -Shell pwsh|powershell` (Task 1), `install.ps1`, `uninstall.ps1`.

- [ ] **Step 1: Write the version check and dry-run it locally**

The check used by `release.yml` (job `verify`, step `version`):

```powershell
$version = (Select-String -Path Cargo.toml -Pattern '^version\s*=\s*"([^"]+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
if ("v$version" -ne $env:GITHUB_REF_NAME) {
    throw "Tag $env:GITHUB_REF_NAME does not match version $version in Cargo.toml"
}
"version=$version" >> $env:GITHUB_OUTPUT
```

Dry run (pwsh), expecting a throw for the wrong tag and `version=0.1.0` for the right one:
```
$env:GITHUB_OUTPUT = "$env:TEMP\gh_out.txt"; Remove-Item $env:GITHUB_OUTPUT -ErrorAction Ignore
$env:GITHUB_REF_NAME = 'v9.9.9'; try { <the script above> } catch { "threw: $_" }
$env:GITHUB_REF_NAME = 'v0.1.0'; <the script above>; Get-Content $env:GITHUB_OUTPUT
```

- [ ] **Step 2: Write `.github/workflows/ci.yml`**

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:

permissions:
  contents: read

jobs:
  gate:
    name: Gate (fmt, clippy, tests)
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - name: Toolchain components
        run: rustup component add rustfmt clippy
      - name: Gate
        shell: pwsh
        run: ./.claude/skills/shell-panel-review/scripts/verify.ps1

  installer:
    name: Installer
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - name: Build release binary
        run: cargo build --release --locked
      - name: Installer under PowerShell 7
        shell: pwsh
        run: ./scripts/test-installer.ps1 -Shell pwsh
      - name: Installer under Windows PowerShell 5.1
        shell: pwsh
        run: ./scripts/test-installer.ps1 -Shell powershell
```

- [ ] **Step 3: Write `.github/workflows/release.yml`**

```yaml
name: Release

on:
  push:
    tags: ['v*']

permissions:
  contents: read

jobs:
  verify:
    name: Verify tag and gate
    runs-on: windows-latest
    outputs:
      version: ${{ steps.version.outputs.version }}
    steps:
      - uses: actions/checkout@v4
      - name: Tag matches Cargo.toml
        id: version
        shell: pwsh
        run: |
          $version = (Select-String -Path Cargo.toml -Pattern '^version\s*=\s*"([^"]+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
          if ("v$version" -ne $env:GITHUB_REF_NAME) {
              throw "Tag $env:GITHUB_REF_NAME does not match version $version in Cargo.toml"
          }
          "version=$version" >> $env:GITHUB_OUTPUT
      - name: Toolchain components
        run: rustup component add rustfmt clippy
      - name: Gate
        shell: pwsh
        run: ./.claude/skills/shell-panel-review/scripts/verify.ps1

  build:
    name: Build ${{ matrix.arch }}
    needs: verify
    runs-on: windows-latest
    strategy:
      matrix:
        include:
          - target: x86_64-pc-windows-msvc
            arch: x64
          - target: aarch64-pc-windows-msvc
            arch: arm64
    steps:
      - uses: actions/checkout@v4
      - name: Add target
        run: rustup target add ${{ matrix.target }}
      - name: Build
        run: cargo build --release --locked --target ${{ matrix.target }}
      - name: Package
        shell: pwsh
        run: |
          $name = "shell-panel-${{ needs.verify.outputs.version }}-${{ matrix.arch }}"
          $stage = Join-Path $env:RUNNER_TEMP $name
          New-Item -ItemType Directory -Force -Path $stage | Out-Null
          Copy-Item -LiteralPath "target/${{ matrix.target }}/release/shell-panel.exe", LICENSE, README.md -Destination $stage
          Compress-Archive -Path (Join-Path $stage '*') -DestinationPath "$name.zip"
      - uses: actions/upload-artifact@v4
        with:
          name: zip-${{ matrix.arch }}
          path: shell-panel-*.zip
          if-no-files-found: error

  installer:
    name: Installer test
    needs: [verify, build]
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/download-artifact@v4
        with:
          name: zip-x64
          path: dist
      - name: Installer under PowerShell 7
        shell: pwsh
        run: ./scripts/test-installer.ps1 -Shell pwsh -ZipPath dist/shell-panel-${{ needs.verify.outputs.version }}-x64.zip
      - name: Installer under Windows PowerShell 5.1
        shell: pwsh
        run: ./scripts/test-installer.ps1 -Shell powershell -ZipPath dist/shell-panel-${{ needs.verify.outputs.version }}-x64.zip

  publish:
    name: Publish release
    needs: [verify, build, installer]
    runs-on: windows-latest
    permissions:
      contents: write
    steps:
      - uses: actions/checkout@v4
      - uses: actions/download-artifact@v4
        with:
          pattern: zip-*
          path: dist
          merge-multiple: true
      - name: Create release
        shell: pwsh
        env:
          GH_TOKEN: ${{ github.token }}
        run: |
          $zips = Get-ChildItem dist -Filter *.zip | Sort-Object Name
          if ($zips.Count -ne 2) { throw "expected 2 zips, found $($zips.Count)" }
          $zips | ForEach-Object { '{0}  {1}' -f (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLower(), $_.Name } |
              Set-Content -LiteralPath dist/SHA256SUMS.txt -Encoding ascii
          $arguments = @($env:GITHUB_REF_NAME) + $zips.FullName + @('dist/SHA256SUMS.txt', 'install.ps1', 'uninstall.ps1',
              '--repo', $env:GITHUB_REPOSITORY, '--title', $env:GITHUB_REF_NAME, '--generate-notes')
          if ($env:GITHUB_REF_NAME -like '*-*') { $arguments += '--prerelease' }
          gh release create @arguments
          if ($LASTEXITCODE -ne 0) { throw "gh release create failed ($LASTEXITCODE)" }
```

- [ ] **Step 4: Validate both workflows**

Run:
```
python -c "import yaml,sys; [yaml.safe_load(open(f, encoding='utf-8')) for f in sys.argv[1:]]; print('yaml ok')" .github/workflows/ci.yml .github/workflows/release.yml
go run github.com/rhysd/actionlint/cmd/actionlint@latest .github/workflows/ci.yml .github/workflows/release.yml
```
Expected: `yaml ok`; actionlint prints nothing and exits 0 (its shellcheck pass is skipped on Windows; the pwsh `run:` blocks are checked by Step 1's dry run). If `go run` cannot download actionlint, record that and rely on the YAML parse plus a CI run after push.

- [ ] **Step 5: Commit**

```bash
git add .github/workflows/ci.yml .github/workflows/release.yml
git commit -m "ci: add the CI gate and the tag-driven release workflow"
```

---

### Task 3: Install and release documentation

**Files:**
- Modify: `README.md` (new "Install" section before "Build and run"; release procedure at the end of "Testing")

- [ ] **Step 1: Add the Install section**

Insert before `## Build and run`:

```markdown
## Install

```powershell
irm https://github.com/jonyduque/shell-panel/releases/latest/download/install.ps1 | iex
```

The installer picks the x64 or ARM64 build, checks it against the release's `SHA256SUMS.txt`, installs `shell-panel.exe` into `%LOCALAPPDATA%\Programs\shell-panel`, adds that folder to your user `PATH` and adds a **PowerShell (shell-panel)** profile to Windows Terminal. Run it again to update. A specific version: download `install.ps1` from the release and run `.\install.ps1 -Version 0.2.0`.

To uninstall (your `~\.config\shell-panel.toml` and custom specs are kept unless you add `-Purge`):

```powershell
irm https://github.com/jonyduque/shell-panel/releases/latest/download/uninstall.ps1 | iex
```
```

- [ ] **Step 2: Add the release procedure**

Append to the "Testing" section:

```markdown
`scripts/test-installer.ps1` tests `install.ps1` and `uninstall.ps1` end to end against `target\release` (run `cargo build --release` first), in temporary folders; add `-Shell powershell` to run them under Windows PowerShell 5.1. It restores your `PATH` afterwards.

### Releasing

1. Set `version` in `Cargo.toml` (and run `cargo build` so `Cargo.lock` follows), commit.
2. `git tag v<version>` and `git push origin v<version>`.

The Release workflow checks that the tag matches `Cargo.toml`, runs the full gate, builds x64 and ARM64, tests the installer on the x64 build and publishes the release with both zips, `SHA256SUMS.txt`, `install.ps1` and `uninstall.ps1`. A tag with a `-` (e.g. `v0.2.0-rc.1`) is published as a pre-release.
```

- [ ] **Step 3: Commit**

```bash
git add README.md
git commit -m "docs: document installing, uninstalling and releasing"
```

---

## After the tasks (gated on the user)

1. Push the branch so CI runs on GitHub; fix anything the hosted runner reveals.
2. Secret scan of the history and the tracked files; report to the user.
3. Only with the user's explicit confirmation: `gh repo edit jonyduque/shell-panel --visibility public --accept-visibility-change-consequences`.
4. First release tag only after the repository is public.
