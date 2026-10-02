# shell-panel integration for PowerShell with PSReadLine 2.x. Passed with -EncodedCommand, with the
# session secret written into __SP-Send as a literal (see integration.rs).
# Messages to shell-panel: ESC ] 6973;<token>;<payload> BEL with payloads RS;<cwd>, RE and
# CMP;<json>. Output of programs cannot know the token, so it cannot forge a message.

# Everything outside printable ASCII, and `\` and `;`, goes as \xHH escapes of its UTF-8 bytes.
# [Console]::Write encodes with the console code page (OEM 850, 437, ...), which turns every
# character it cannot represent into '?'; ASCII is the same in all of them. One pass over the
# bytes of the whole string (a surrogate pair becomes one 4-byte sequence, a lone half U+FFFD):
# a byte loop is far faster than a regex with a scriptblock per character.
function Global:__SP-Escape([string]$value) {
    $out = New-Object System.Text.StringBuilder
    foreach ($b in [System.Text.Encoding]::UTF8.GetBytes($value)) {
        if ($b -ge 0x20 -and $b -le 0x7e -and $b -ne 0x3b -and $b -ne 0x5c) { [void]$out.Append([char]$b) }
        else { [void]$out.Append('\x' + $b.ToString('x2')) }
    }
    $out.ToString()
}

function Global:__SP-Send([string]$payload) {
    [Console]::Write("$([char]0x1b)]6973;__SP_TOKEN__;$payload$([char]0x07)")
}

# Mark the time PSReadLine spends reading a line. Unlike a prompt wrapper this survives the user
# redefining `prompt` and does not change what the prompt function sees.
if (Get-Command PSConsoleHostReadLine -ErrorAction Ignore) {
    # Wrapping our own wrapper on a second load would send every marker twice. The wrapper is
    # recognised by the marker comment in its body, since no global variable may be relied on.
    if (-not $function:PSConsoleHostReadLine.ToString().Contains('shell-panel-readline-wrapper')) {
        # The closure is made in a child scope (& { param(...) ... }) so that it captures only the
        # original: GetNewClosure at global scope would also copy $PWD and the preference
        # variables into the wrapper, freezing the location it reports. The original lives in the
        # closure, so a script that clears the global variables cannot take it away.
        Set-Item -Path function:Global:PSConsoleHostReadLine -Value (& {
            param($sp_original)
            {
                # shell-panel-readline-wrapper
                $cwd = if ($pwd.Provider.Name -eq 'FileSystem') { $pwd.ProviderPath } else { '' }
                __SP-Send "RS;$(__SP-Escape $cwd)"
                try { $sp_original.Invoke() } finally { __SP-Send 'RE' }
            }.GetNewClosure()
        } $function:PSConsoleHostReadLine)
    }
}

# PSReadLine's prediction list covers the rows shell-panel draws its dropdown on, so the two
# collide. Only the list view is in the way: an inline prediction and the prediction source itself
# are left as the user configured them. Older PSReadLine versions have no such option.
try {
    if ((Get-PSReadLineOption).PredictionViewStyle -eq 'ListView') {
        Set-PSReadLineOption -PredictionViewStyle InlineView
    }
} catch {}

# Ctrl+Alt+Shift+F12 (sent by shell-panel when Tab is pressed): report line, cursor and completions.
try {
    Set-PSReadLineKeyHandler -Chord 'Ctrl+Alt+Shift+F12' -BriefDescription 'ShellPanelReport' -ScriptBlock {
        # A failure here must never print over the line the user is editing.
        try {
            $line = $null
            $cursor = $null
            [Microsoft.PowerShell.PSConsoleReadLine]::GetBufferState([ref]$line, [ref]$cursor)
            $report = @{ line = [string]$line; cursor = $cursor; replacementIndex = $cursor; replacementLength = 0; matches = @() }
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
        } catch {}
    }
} catch {}
