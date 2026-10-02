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
.PARAMETER ApiUri
    Testing only: the URI of the latest-release lookup, instead of the GitHub API's.
.PARAMETER TerminalFragmentDir
    Testing only: where the Windows Terminal profile fragment is written.
.PARAMETER ForceStyle
    Testing only: use colours and icons even when the output is redirected (NO_COLOR still wins).
#>
[CmdletBinding()]
param(
    [string]$Version,
    [string]$InstallDir,
    [string]$ZipPath,
    [string]$ChecksumsPath,
    [string]$TerminalFragmentDir,
    [string]$ApiUri,
    [switch]$ForceStyle
)
# The parameters are bound here and handed to a script block that runs in a child scope: piped
# through `iex` a script runs in the caller's scope, and the block keeps its variables, functions
# and preferences out of the user's session; `throw` (never `exit`) ends it. @PSBoundParameters,
# not @args: @args splits `-Switch:$false` into two arguments. Defaults live in the block.
& {
    param(
        [string]$Version,
        [string]$InstallDir = (Join-Path $env:LOCALAPPDATA 'Programs\shell-panel'),
        [string]$ZipPath,
        [string]$ChecksumsPath,
        [string]$ApiUri,
        [string]$TerminalFragmentDir = (Join-Path $env:LOCALAPPDATA 'Microsoft\Windows Terminal\Fragments\shell-panel'),
        [switch]$ForceStyle
    )
    Set-StrictMode -Version Latest
    $ErrorActionPreference = 'Stop'
    # The calls below into .NET are refused in ConstrainedLanguage mode; fail before touching anything.
    if ($ExecutionContext.SessionState.LanguageMode -ne 'FullLanguage') {
        throw "The shell-panel installer needs FullLanguage mode (this session is $($ExecutionContext.SessionState.LanguageMode)). Nothing was installed."
    }
    # A relative folder would land in PATH as-is and resolve against each process's current folder.
    $InstallDir = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($InstallDir)
    # The progress bar makes Invoke-WebRequest many times slower on Windows PowerShell 5.1.
    $ProgressPreference = 'SilentlyContinue'

    # Styled output (colours, icons, bold, italic) only on a console that understands escape
    # sequences; anywhere else the plain [*] / [OK] / [!] / [i] markers, so logs and CI stay as
    # they were. NO_COLOR always wins. The file stays ASCII: ESC and the icons are built here.
    $styled = $false
    if (-not $env:NO_COLOR) {
        if ($ForceStyle) { $styled = $true }
        else { $styled = try { [bool]$Host.UI.SupportsVirtualTerminal -and -not [Console]::IsOutputRedirected } catch { $false } }
    }
    $e = [char]0x1b
    $icons = @{
        step = [char]::ConvertFromUtf32(0x23F3)
        done = [char]::ConvertFromUtf32(0x2705)
        note = [char]::ConvertFromUtf32(0x26A0) + [char]0xFE0F
        fail = [char]::ConvertFromUtf32(0x274C)
    }
    function Format-Path([string]$text) { if ($styled) { "$e[3m$text$e[23m" } else { $text } }
    # A command span ends with bold and colour off ($cmdEnd); inside a coloured line Write-Styled
    # follows that with the line's own colour again, so the text after the span keeps it.
    $cmdEnd = "$e[22;39m"
    function Format-Cmd([string]$text) { if ($styled) { "$e[1;36m$text$cmdEnd" } else { $text } }
    function Write-Styled([string]$icon, [string]$sgr, [string]$text) {
        Write-Host "$e[${sgr}m$icon $($text.Replace($cmdEnd, "$cmdEnd$e[${sgr}m"))$e[0m"
    }
    function Write-Step([string]$text) { if ($styled) { Write-Styled $icons.step '36' $text } else { Write-Host "[*] $text" } }
    function Write-Done([string]$text) { if ($styled) { Write-Styled $icons.done '32' $text } else { Write-Host "[OK] $text" } }
    function Write-Note([string]$text) { if ($styled) { Write-Styled $icons.note '33' $text } else { Write-Host "[i] $text" } }
    function Write-Fail([string]$text) { if ($styled) { Write-Styled $icons.fail '1;31' $text } else { Write-Host "[!] $text" } }
    function Write-Header([string]$title) { if ($styled) { Write-Host "$e[1m$title$e[0m" } }
    function Write-Detail([string]$text) { if ($styled) { Write-Host "$e[2m$e[3m$text$e[0m" } }

    $repo = 'jonyduque/shell-panel'
    if (-not $ApiUri) { $ApiUri = "https://api.github.com/repos/$repo/releases/latest" }
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

    function Get-LatestVersion([string]$uri) {
        try {
            return (Invoke-RestMethod -UseBasicParsing -Uri $uri).tag_name
        } catch {
            $detail = $_.Exception.Message
            $status = try { [int]$_.Exception.Response.StatusCode } catch { 0 }
            $cause = if ($status -eq 403 -or $status -eq 429 -or $detail -match 'rate limit') {
                'GitHub refused the request (HTTP 403 or "rate limit": the API allows few anonymous requests per hour). '
            } else { '' }
            throw "Could not look up the latest shell-panel release: $cause($detail) Pass -Version (for example -Version 0.2.0) to skip the lookup. Nothing was installed."
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

    $savedProtocol = $null
    $work = Join-Path ([IO.Path]::GetTempPath()) ('shell-panel-install-' + [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $work | Out-Null
    try {
        $arch = Get-Arch
        Write-Header 'shell-panel installer'
        if ($ZipPath) {
            Write-Detail "local zip, $arch"

            if (-not $ChecksumsPath) { throw '-ChecksumsPath is required with -ZipPath.' }
            $zip = (Resolve-Path -LiteralPath $ZipPath).Path
            $sums = (Resolve-Path -LiteralPath $ChecksumsPath).Path
        } else {
            # Restored in the finally: iex runs this in the caller's session, where the setting would outlive it.
            $savedProtocol = [Net.ServicePointManager]::SecurityProtocol
            [Net.ServicePointManager]::SecurityProtocol = $savedProtocol -bor [Net.SecurityProtocolType]::Tls12
            if (-not $Version) {
                Write-Step 'Looking up the latest release'
                $Version = Get-LatestVersion $ApiUri
            }
            $Version = $Version -replace '^v', ''
            Write-Detail "$Version, $arch"

            $name = "shell-panel-$Version-$arch.zip"
            $base = "https://github.com/$repo/releases/download/v$Version"
            $zip = Join-Path $work $name
            $sums = Join-Path $work 'SHA256SUMS.txt'
            Write-Step "Downloading $(Format-Path $name)"
            Invoke-WebRequest -UseBasicParsing -Uri "$base/$name" -OutFile $zip
            Invoke-WebRequest -UseBasicParsing -Uri "$base/SHA256SUMS.txt" -OutFile $sums
        }

        $zipName = Split-Path -Leaf $zip
        $expected = Get-ExpectedHash $sums $zipName
        $actual = (Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash
        if ($actual -ine $expected) {
            throw "Checksum mismatch for ${zipName}: expected $expected, got $actual. Nothing was installed."
        }
        Write-Done 'Checksum verified'

        $extract = Join-Path $work 'files'
        Expand-Archive -LiteralPath $zip -DestinationPath $extract
        $exeSource = Join-Path $extract 'shell-panel.exe'
        if (-not (Test-Path -LiteralPath $exeSource)) { throw "$zipName does not contain shell-panel.exe. Nothing was installed." }

        # The marker tells uninstall.ps1 which folder this installer owns. Never write into a folder
        # of other files: the uninstaller would later be asked to clean it.
        $marker = Join-Path $InstallDir '.shell-panel-install'
        if ((Test-Path -LiteralPath $InstallDir) -and -not (Test-Path -LiteralPath $marker) -and
            @(Get-ChildItem -LiteralPath $InstallDir -Force).Count -gt 0) {
            throw "$InstallDir already holds other files and is not a shell-panel install. Pass an empty or new folder to -InstallDir. Nothing was installed."
        }

        New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
        $exe = Join-Path $InstallDir 'shell-panel.exe'
        $aside = "$exe.old"
        # Left by an update made while shell-panel was running; still locked if that session lives.
        Remove-Item -LiteralPath $aside -Force -ErrorAction SilentlyContinue
        try {
            Copy-Item -LiteralPath $exeSource -Destination $exe -Force
        } catch {
            # A running shell-panel.exe cannot be overwritten, but Windows lets it be renamed: move
            # it aside so that updating from inside a session works. The next install deletes it.
            try {
                Move-Item -LiteralPath $exe -Destination $aside -Force
                Copy-Item -LiteralPath $exeSource -Destination $exe
            } catch {
                throw "Could not replace $exe ($($_.Exception.Message)). Close every shell-panel session and run the installer again."
            }
        }
        foreach ($doc in 'LICENSE', 'README.md') {
            $docSource = Join-Path $extract $doc
            if (Test-Path -LiteralPath $docSource) { Copy-Item -LiteralPath $docSource -Destination $InstallDir -Force }
        }
        Set-Content -LiteralPath $marker -Value 'Written by the shell-panel installer. uninstall.ps1 removes only shell-panel.exe, LICENSE, README.md and this file from this folder.'
        Write-Done "Installed $(Format-Path $exe)"

        Add-UserPath $InstallDir
        Write-Done "$(Format-Path $InstallDir) is on your user PATH"

        Write-TerminalFragment $exe
        Write-Done "Windows Terminal profile $(Format-Cmd '"PowerShell (shell-panel)"') written"

        $installed = & $exe --version
        if ($LASTEXITCODE -ne 0) { throw "$exe --version failed (exit $LASTEXITCODE); the installed binary does not run." }
        Write-Done "$installed"
        Write-Host ''
        if ($styled) {
            Write-Host "$e[1mNext steps$e[0m"
            Write-Host "  - Open the $(Format-Cmd '"PowerShell (shell-panel)"') profile in Windows Terminal"
            Write-Host "  - Or run $(Format-Cmd 'shell-panel') in a new terminal window"
        } else {
            Write-Note 'Open the "PowerShell (shell-panel)" profile in Windows Terminal, or run shell-panel in a new terminal window.'
        }
    } catch {
        # Styled mode only, and only a headline: the message itself comes once, in PowerShell's own
        # report of the throw below. Plain mode leaves the error to that report alone.
        if ($styled) { Write-Fail 'Install failed:' }
        throw
    } finally {
        if ($null -ne $savedProtocol) { [Net.ServicePointManager]::SecurityProtocol = $savedProtocol }
        Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue
    }
} @PSBoundParameters
