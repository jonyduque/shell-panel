$input_stream = [Console]::In
$output_stream = [Console]::Out

while ($line = $input_stream.ReadLine()) {
    if (-not $line) { continue }
    if ($line -eq "EXIT") { break }
    try {
        $req = $line | ConvertFrom-Json
        $text = $req.text
        $cursor = if ($null -ne $req.cursor) { [int]$req.cursor } else { $text.Length }
        $completions = [System.Management.Automation.CommandCompletion]::CompleteInput($text, $cursor, $null)
        $results = @()
        if ($completions -and $completions.CompletionMatches) {
            foreach ($match in $completions.CompletionMatches) {
                $results += @{
                    name = $match.CompletionText
                    display = $match.ListItemText
                    description = $match.ToolTip
                    type = $match.ResultType.ToString()
                }
            }
        }
        $json = @{
            replacementIndex = $completions.ReplacementIndex
            replacementLength = $completions.ReplacementLength
            matches = $results
        } | ConvertTo-Json -Compress -Depth 5
        $output_stream.WriteLine($json)
        $output_stream.Flush()
    } catch {
        $output_stream.WriteLine('{"matches":[]}')
        $output_stream.Flush()
    }
}
