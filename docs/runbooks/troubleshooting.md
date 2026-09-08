# Runbook — Troubleshooting

## Bot não opera (nenhum sinal há muito tempo)

1. É horário de pregão? **Toda janela do bot é horário de Nova York (ET)**, convertida
   por `chrono-tz` — não refaça a conta em UTC na mão: o offset muda com o DST, e era
   exatamente esse o bug que o hotfix v1.0.1 corrigiu. A janela sai de
   `trading_start_time`/`trading_end_time` da config da estratégia e varia por
   estratégia (a `opening-reversal-v1` só opera 09h30–10h30 ET). Vetos de horário idem:
   o de meio-dia da `range-extreme-fade-v1` é 11h30–14h00 ET.
2. `status`/`journal`: há sinais rejeitados? `analyze --symbol <S> --strategy <E>`
   mostra a distribuição de motivos. **Passe sempre os dois argumentos** — sem eles o
   comando cai nos defaults `SPY`/`pullback-trend-v1`, que não é instância viva.
3. Limite diário atingido? Veja `daily_trades`/`daily_pnl` no log de boot ("estado de risco reconstruído").
4. Mercado sem tendência de alta → rejeição `NoContext` é comportamento esperado.
5. É fim de pregão? A partir de `[session].flatten_start` (15h55 ET) o flatten do
   ADR-018 **cancela a entrada pendente** ("entrada pendente cancelada no fim da
   sessão") — abrir posição a cinco minutos do sino é o oposto do que se quer. Mas o
   flatten roda **uma única vez por pregão** e **não é portão de entrada**: depois de
   rodar ele não volta a cancelar nada até o dia seguinte, e o ADR-018 §3 decidiu
   explicitamente "não bloquear sinal na última barra". Quem impede entrada tardia é o
   `trading_end_time` da estratégia (rejeição `OutsideTradingHours`, janela inclusiva
   nas duas pontas) — nas três em produção ele é ≤ 15h30, então o sinal da barra das
   15h45 nem chega a virar ordem. A `pullback-trend-v1`, desligada desde o ADR-016, ia
   até 16h00 ET e não tinha essa proteção. `[session].last_bar` (15h45 ET) é a última
   barra de 15 min do RTH.

## Falha ao buscar candles / reconectar na IBKR

- Sintoma: warns "falha ao buscar candles na IBKR". O ciclo é pulado e retentado.
- 10 falhas consecutivas → circuit breaker encerra o live (com alerta).
- Causas comuns: IB Gateway deslogado, reinício diário do gateway, firewall, `client_id` em conflito com outra sessão.

## Ordem enviada mas fill não aparece no banco

- O polling de execuções roda a cada 15s; aguarde um ciclo.
- Verifique na TWS se a ordem foi executada de fato (limit de entrada pode não pegar).
- Fills de outras contas/símbolos são ignorados por design (log "fill de outro símbolo ignorado").

## Restart perdeu o trade aberto?

- Não deveria: no boot, o bot recupera ordens abertas do banco e re-lê os fills.
- Se a ordem foi colocada **antes** desta versão (sem persistência de ordem), o fill de saída será logado como "fill sem ordem rastreada" e ignorado — feche/audite manualmente na TWS.
- **Isso agora tem prazo (ADR-018):** posição que nenhuma instância rastreia **não é
  zerada pelo flatten de fim de sessão**. É de propósito — um mesmo símbolo pode rodar
  com duas instâncias (hoje o AVUV, em `balance-area-breakout-v1` e
  `range-extreme-fade-v1`) e, se as duas zerassem a mesma posição, a segunda abriria
  uma posição invertida a mercado a cinco minutos do sino. O flatten grava evento `critical` /
  `untracked_position` em `system_events`, alerta e espera decisão humana. Sem ação até
  o fechamento a posição atravessa a noite **sem stop** (as pernas do bracket vão com
  TIF Day e expiram no sino).
- Zerar na mão: `trader-cli flatten --symbol <S> --provider ibkr --confirm` (sem
  `--confirm` só mostra o que faria; o comando recusa provider diferente de `ibkr`).

## `analyze` avisa "Nenhum run de backtest para (...)"

- Desde o ADR-019 o baseline é buscado por **(estratégia, par, `config_hash`)**, e runs
  marcados `experimental` são ignorados. Sem run compatível o comando **avisa e sai**:
  não há mais fallback para o run mais recente de outro par ou de outra config.
- Saída: rodar `trader-cli walkforward --symbol <S> --strategy <E>` com a config de
  produção — sem `--set`, sem `--strategy-config` e sem `--no-flatten`.

## A amostra de paper encolheu de repente

- O lado live também passou a ser filtrado por `strategy_id` **e** `config_hash`; o
  `analyze` imprime quantos trades do símbolo ficaram de fora.
- **Mudar parâmetro de estratégia muda o `config_hash` e zera a contagem do gate B.**
  Foi o que aconteceu com a `range-extreme-fade-v1` no hotfix v1.0.1
  (`49ee6f045b4c35a7` → `818b53394244ca62`): os trades anteriores continuam no banco,
  mas não são mais a mesma estratégia para efeito de validação.

## Backtest sem dados

- `backtest` falha sem candles reais (a menos que `--allow-synthetic`).
- Rode `trader-cli ingest --symbol SPY --days 180 --provider ibkr`.
- Verifique qualidade: tabela `ingestions` (gaps_detected) por execução de ingest.

## Banco indisponível

- Modo live **não sobe** sem banco (falha fechada — auditoria obrigatória).
- Modos simulated/replay seguem sem persistência, com aviso.
- `docker compose up -d postgres` e confira `DATABASE_URL`.
