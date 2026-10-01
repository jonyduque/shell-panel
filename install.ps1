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
[CmdletBinding()]
param(
    [string]$Version,
    [string]$InstallDir,
    [string]$ZipPath,
    [string]$ChecksumsPath,
    [string]$TerminalFragmentDir
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
        [string]$TerminalFragmentDir = (Join-Path $env:LOCALAPPDATA 'Microsoft\Windows Terminal\Fragments\shell-panel')
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
        Write-Host "[OK] Installed $exe"

        Add-UserPath $InstallDir
        Write-Host "[OK] $InstallDir is on your user PATH"

        Write-TerminalFragment $exe
        Write-Host '[OK] Windows Terminal profile "PowerShell (shell-panel)" written'

        $installed = & $exe --version
        if ($LASTEXITCODE -ne 0) { throw "$exe --version failed (exit $LASTEXITCODE); the installed binary does not run." }
        Write-Host "[OK] $installed"
        Write-Host ''
        Write-Host '[i] Open the "PowerShell (shell-panel)" profile in Windows Terminal, or run shell-panel in a new terminal window.'
    } finally {
        Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue
    }
} @PSBoundParameters
