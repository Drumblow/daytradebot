# Regera os 8 walk-forwards do gate A com a regua do live (ADR-018 + hotfix
# ET), agora com `initial_capital` no JSON. Mesma janela e mesmas 6 janelas do
# relatorio de 07/09; o que muda e so o metadado.
$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot\..

$combos = @(
    @{ e = "balance-area-breakout-v1"; s = "IJS" },
    @{ e = "balance-area-breakout-v1"; s = "VBR" },
    @{ e = "balance-area-breakout-v1"; s = "AVUV" },
    @{ e = "opening-reversal-v1";      s = "IWM" },
    @{ e = "opening-reversal-v1";      s = "IWN" },
    @{ e = "range-extreme-fade-v1";    s = "AVUV" },
    @{ e = "range-extreme-fade-v1";    s = "SLYV" },
    @{ e = "range-extreme-fade-v1";    s = "IWV" }
)

foreach ($c in $combos) {
    $saida = "out/adr018/wf19_$($c.e)_$($c.s).json"
    Write-Host "=== $($c.e) $($c.s) ==="
    .\target\release\trader-cli.exe walkforward `
        --symbol $c.s --strategy $c.e `
        --from 2025-02-24 --to 2026-09-03 -w 6 `
        --label gate-a-adr019 --output $saida 2>&1 |
        Select-String -Pattern "Trades:|Profit factor|Avg R|Net P&L|PF em R|2 melhores|exportado" |
        ForEach-Object { $_.Line.Trim() }
}
