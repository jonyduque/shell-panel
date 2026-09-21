try {
    Set-PSReadLineOption -PredictionSource None
} catch {}

$Global:__IsOriginalPrompt = $function:Prompt

function Global:__IsTestingPrompt() {
    return "PS > "
}

if ($env:ISTERM_TESTING -eq "1") {
    $Global:__IsOriginalPrompt = $function:__IsTestingPrompt
}

function Global:__IS-Escape-Value([string]$value) {
    [regex]::Replace($value, "[$([char]0x1b)$([char]0x07)\\\n;]", { param($match)
        -Join (
            [System.Text.Encoding]::UTF8.GetBytes($match.Value) | ForEach-Object { '\x{0:x2}' -f $_ }
        )
    })
}

function Global:__SP-Escape([string]$value) {
    [regex]::Replace($value, '[\x00-\x1f\x7f\\;]', { param($match)
        -join ([System.Text.Encoding]::UTF8.GetBytes($match.Value) | ForEach-Object { '\x{0:x2}' -f $_ })
    })
}

function Global:__SP-Send([string]$payload) {
    [Console]::Write("$([char]0x1b)]6973;$payload$([char]0x07)")
}

# Mark the time PSReadLine spends reading a line. Unlike a prompt wrapper this survives the user
# redefining `prompt` and does not change what the prompt function sees.
if (Get-Command PSConsoleHostReadLine -ErrorAction Ignore) {
    $Global:__SP_OriginalReadLine = $function:PSConsoleHostReadLine
    function Global:PSConsoleHostReadLine {
        $cwd = if ($pwd.Provider.Name -eq 'FileSystem') { $pwd.ProviderPath } else { '' }
        __SP-Send "RS;$(__SP-Escape $cwd)"
        try { $Global:__SP_OriginalReadLine.Invoke() } finally { __SP-Send 'RE' }
    }
}

# Ctrl+Alt+Shift+F12 (sent by shell-panel when Tab is pressed): report line, cursor and completions.
try {
    Set-PSReadLineKeyHandler -Chord 'Ctrl+Alt+Shift+F12' -BriefDescription 'ShellPanelReport' -ScriptBlock {
        $line = $null
        $cursor = $null
        [Microsoft.PowerShell.PSConsoleReadLine]::GetBufferState([ref]$line, [ref]$cursor)
        $report = @{ line = $line; cursor = $cursor; replacementIndex = $cursor; replacementLength = 0; matches = @() }
        try {
            $completion = [System.Management.Automation.CommandCompletion]::CompleteInput($line, $cursor, $null)
            $report.replacementIndex = $completion.ReplacementIndex
            $report.replacementLength = $completion.ReplacementLength
            $report.matches = @($completion.CompletionMatches | Select-Object -First 100 | ForEach-Object {
                $tip = "$($_.ToolTip)"
                if ($tip.Length -gt 120) {
                    $cut = if ([char]::IsHighSurrogate($tip[119])) { 119 } else { 120 }
                    $tip = $tip.Substring(0, $cut)
                }
                , @($_.CompletionText, $_.ListItemText, $_.ResultType.ToString(), $tip)
            })
        } catch {}
        __SP-Send "CMP;$(__SP-Escape (ConvertTo-Json -InputObject $report -Compress -Depth 4))"
    }
} catch {}

function Global:Prompt() {
    $Result = "$([char]0x1b)]6973;PS`a"
    if ($Global:__IsOriginalPrompt) {
        $Result += $Global:__IsOriginalPrompt.Invoke()
    } else {
        $Result += "PS > "
    }
    $Result += "$([char]0x1b)]6973;PE`a"

    if ($pwd.Provider.Name -eq 'FileSystem') {
        $Result += "$([char]0x1b)]6973;CWD;$(__IS-Escape-Value $pwd.ProviderPath)`a"
    }
    return $Result
}
