<#
.SYNOPSIS
  Lista todo consumidor de um simbolo antes de alguem chama-lo de morto.
.DESCRIPTION
  "Nao usado" para o rustc/clippy nao e "nao usado": o crate e uma lib (src/lib.rs) e os testes de
  integracao em tests/ consomem itens pub; o script PowerShell embutido e os specs JSON consomem
  nomes por string; docs e wiki citam opcoes e chaves.
  Procura palavra inteira em src/, tests/, assets/, docs/ (menos docs/superpowers: planos historicos citam codigo que ja mudou), README.md e Cargo.toml.
  Exit 0 = ha consumidor de producao/script/doc fora do arquivo de origem;
       1 = so aparece no arquivo de origem;
       2 = fora da origem, so testes consomem.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Symbol,
    [Parameter(Mandatory)][string]$OriginFile
)
$origin = ($OriginFile -replace '\\', '/')
$hits = & git grep -n -w --untracked -e $Symbol -- src tests assets docs README.md Cargo.toml ':!docs/superpowers' 2>$null
if (-not $hits) {
    Write-Output "[i] '$Symbol': nenhuma ocorrencia (nem no arquivo de origem; nome certo?)."
    exit 1
}
$byFile = $hits | ForEach-Object { ($_ -split ':', 3)[0] } | Group-Object | Sort-Object Name
foreach ($g in $byFile) { Write-Output ("{0,4}  {1}" -f $g.Count, $g.Name) }
$outside = $byFile | Where-Object { $_.Name -ne $origin }
$prodOutside = $outside | Where-Object { $_.Name -notmatch '^tests/' }
if ($outside -and -not $prodOutside) {
    Write-Output "[!] '$Symbol' fora de $origin so aparece em tests/: nenhum codigo de producao, script ou doc consome."
    Write-Output "    Codigo que so teste usa e candidato a morto, mas confira se $origin o usa internamente."
    exit 2
}
if ($outside) {
    Write-Output "[i] '$Symbol' tem consumidor fora de $origin. Linhas:"
    $hits | Where-Object { ($_ -split ':', 3)[0] -ne $origin } | Select-Object -First 20 | ForEach-Object { Write-Output "    $_" }
    exit 0
}
Write-Output "[!] '$Symbol' so aparece em $origin."
exit 1
