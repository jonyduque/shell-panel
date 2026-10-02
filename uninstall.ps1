<#
.SYNOPSIS
    Removes shell-panel installed by install.ps1.
.DESCRIPTION
    Removes what the installer wrote (shell-panel.exe, LICENSE, README.md and its marker file), the
    install folder when nothing else is left in it, the folder's entry in the user PATH and the
    Windows Terminal profile. A folder without the installer's marker is never touched. Your
    configuration (%USERPROFILE%\.config\shell-panel.toml and .config\shell-panel\) is kept unless
    -Purge is given. Windows PowerShell 5.1 and PowerShell 7.
.PARAMETER InstallDir
    Default: %LOCALAPPDATA%\Programs\shell-panel.
.PARAMETER Purge
    Also delete the configuration file and the custom specs directory.
.PARAMETER TerminalFragmentDir
    Testing only: where the Windows Terminal profile fragment was written.
.PARAMETER ForceStyle
    Testing only: use colours and icons even when the output is redirected (NO_COLOR still wins).
#>
[CmdletBinding()]
param(
    [string]$InstallDir,
    [switch]$Purge,
    [string]$TerminalFragmentDir,
    [switch]$ForceStyle
)
# Same structure as install.ps1: parameters bound here, body in a child-scope script block so that
# `iex` leaves nothing behind, and @PSBoundParameters so that -Purge:$false stays false.
& {
    param(
        [string]$InstallDir = (Join-Path $env:LOCALAPPDATA 'Programs\shell-panel'),
        [switch]$Purge,
        [string]$TerminalFragmentDir = (Join-Path $env:LOCALAPPDATA 'Microsoft\Windows Terminal\Fragments\shell-panel'),
        [switch]$ForceStyle
    )
    Set-StrictMode -Version Latest
    $ErrorActionPreference = 'Stop'
    # The PATH edit below needs .NET calls that ConstrainedLanguage refuses: fail before deleting.
    if ($ExecutionContext.SessionState.LanguageMode -ne 'FullLanguage') {
        throw "The shell-panel uninstaller needs FullLanguage mode (this session is $($ExecutionContext.SessionState.LanguageMode)). Nothing was removed."
    }
    $InstallDir = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($InstallDir)

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

    try {
        Write-Header 'shell-panel uninstaller'
        Write-Detail $InstallDir
        $marker = Join-Path $InstallDir '.shell-panel-install'
        $exe = Join-Path $InstallDir 'shell-panel.exe'
        if (Test-Path -LiteralPath $marker) {
            try {
                if (Test-Path -LiteralPath $exe) { Remove-Item -LiteralPath $exe -Force }
            } catch {
                throw "Could not remove $exe ($($_.Exception.Message)). Close every shell-panel session and run the uninstaller again."
            }
            # Only what the installer wrote; a moved-aside binary may still be locked by a live session.
            foreach ($name in 'shell-panel.exe.old', 'LICENSE', 'README.md', '.shell-panel-install') {
                Remove-Item -LiteralPath (Join-Path $InstallDir $name) -Force -ErrorAction SilentlyContinue
            }
            if (@(Get-ChildItem -LiteralPath $InstallDir -Force).Count -eq 0) {
                Remove-Item -LiteralPath $InstallDir -Force
                Write-Done "Removed $(Format-Path $InstallDir)"
            } else {
                Write-Done "Removed shell-panel from $(Format-Path $InstallDir) (files you added there were kept)"
            }
            Remove-UserPath $InstallDir
            Write-Done 'Removed from the user PATH'
        } elseif (Test-Path -LiteralPath $exe) {
            # -InstallDir is user input: a folder of other tools holding a hand-copied shell-panel.exe.
            throw "$InstallDir was not created by the shell-panel installer (no .shell-panel-install file). Nothing was removed."
        } else {
            Write-Note "No shell-panel install in $(Format-Path $InstallDir)"
        }

        $fragment = Join-Path $TerminalFragmentDir 'shell-panel.json'
        if (Test-Path -LiteralPath $fragment) {
            Remove-Item -LiteralPath $fragment -Force
            if (@(Get-ChildItem -LiteralPath $TerminalFragmentDir -Force).Count -eq 0) {
                Remove-Item -LiteralPath $TerminalFragmentDir -Force
            }
            Write-Done 'Removed the Windows Terminal profile'
        }

        $config = Join-Path $env:USERPROFILE '.config\shell-panel.toml'
        $specs = Join-Path $env:USERPROFILE '.config\shell-panel'
        if ($Purge) {
            foreach ($item in $config, $specs) {
                if (Test-Path -LiteralPath $item) {
                    Remove-Item -LiteralPath $item -Recurse -Force
                    Write-Done "Removed $(Format-Path $item)"
                }
            }
        } else {
            Write-Note "Kept your configuration ($(Format-Path $config) and $(Format-Path $specs)); run with $(Format-Cmd '-Purge') to remove it."
        }
    } catch {
        # Styled mode only, and only a headline: the message itself comes once, in PowerShell's own
        # report of the throw below. Plain mode leaves the error to that report alone.
        if ($styled) { Write-Fail 'Uninstall failed:' }
        throw
    }
} @PSBoundParameters
