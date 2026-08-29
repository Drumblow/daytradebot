# ADR-015 — Guarda de overshoot na entrada stop (live ≡ backtest)

**Status:** ACEITO — implementado em 2026-08-29
**Contexto:** trades 7/8 (dia 1, artefatos de latência), trade 11 (gap IWO,
2026-08-13) e trade 12 (AVUV, 2026-08-20) — quatro perdas da mesma família.

---

## 1. Problema

A entrada das estratégias é um **stop order** (ADR-009): rompeu o gatilho,
entrou. Entre o fechamento da barra do sinal e a ordem chegar ao broker passam
segundos (poll ~30s + processamento); com gap de abertura de barra, o preço
pode já ter corrido **além** do gatilho quando a ordem começa a trabalhar. O
stop vira marketable e enche no preço corrente — não no gatilho.

Consequência medida em produção (trade 12): fill 0.38 além de um gatilho cujo
stop distava 0.35 → **o risco real dobrou** (−$1.494,50, a maior perda do
histórico). No trade 11, o gap de 0.67 deixou o alvo *abaixo* da entrada: o TP
ficou marketable e o trade abriu e fechou em 2s (−$83,39). A "anomalia de
finalização do bracket" registrada no day8 **não era bug de classificação** —
a perna de alvo executou de verdade (limit abaixo do mercado enche a preço
melhor); o defeito era entrar depois do preço já ter ido embora.

Três estratégias tinham uma guarda parcial (rejeitar se o *close da barra
fechada* já passou do gatilho — dia 1); as outras três, incluindo a rangefade
do trade 12, não tinham nenhuma. E nenhuma guarda olhava o preço **corrente**.

Do outro lado, o backtest era otimista: o broker simulado enchia a entrada
sempre em `gatilho ± slippage fixo`, ignorando gaps — o custo que o live paga
não aparecia nos números.

## 2. Decisão

Uma régua única, configurável, aplicada nos dois mundos:

> **Se o preço já passou do gatilho mais do que `tolerância × distância do
> stop`, a entrada é invalidada — não se persegue o rompimento.**
> Dentro da tolerância, entra-se pagando o preço corrente (custo real do gap).

- Config: `[risk] entry_overshoot_tolerance` (fração da distância do stop,
  **default 0.25** = aceita até 25% de risco extra). Flui por
  `build_risk_config` → `RiskConfig` → live, replay e backtest.
- **Live** (`ExecutionEngine::process_signal`): novo parâmetro
  `reference_price` — o close da **barra em formação** do último fetch (o
  fetch do live já a inclui; `closed` só filtra o processamento). Overshoot
  além da tolerância ⇒ sinal rejeitado com `SetupInvalidated` e persistido
  (aparece no painel com o motivo). Só vale para `EntryOrderType::Stop` — um
  limit nunca enche pior que o próprio preço.
- **Backtest/replay/simulated** (`SimulatedBroker::set_market_candle`): quando
  a barra aciona o gatilho, o fill parte de `max(open, gatilho)` (long; espelho
  no short) — **gap agora custa** — e um open além da tolerância **cancela** a
  entrada (`OrderStatus::Cancelled`), espelhando a guarda pré-envio do live.

As guardas por estratégia do dia 1 (close da barra fechada além do gatilho ⇒
rejeita, tolerância zero) **permanecem** — são mais estritas e testadas; a
guarda central cobre as estratégias que não as têm e o caso intrabar.

## 3. Assimetrias conhecidas (aceitas e documentadas)

1. No live, depois de submetida, a ordem ainda pode encher num gap da barra
   seguinte (validade de 1 candle); o simulado cancelaria nesse caso. Janela
   pequena, e o novo fill-na-abertura do simulado é estritamente mais honesto
   que o modelo antigo (gatilho + 0,1% sempre).
2. A referência de preço do live é o último print conhecido do feed (barra em
   formação), não um tick real-time — a conta paper não tem subscrição
   (erro 10168). É o melhor dado disponível e teria pegado os 4 casos reais.

## 4. Consequências

- **Backtests anteriores a este ADR não são comparáveis** com os novos (mesmo
  precedente do ADR-009, quando a entrada saiu de limit para stop): o fill de
  gap encarece entradas e a invalidação corta trades que antes "enchiam no
  gatilho". Os números novos são mais baixos e mais verdadeiros.
  Recomendação: re-rodar o backtest completo do portfólio para rebaixar a
  régua de comparação do gate B antes do pregão de 2026-08-31.
- Trades da família "gap além do gatilho" (7, 8, 11, 12 — todas as 4 maiores
  fontes de perda evitável do live) deixam de existir; no lugar surge um sinal
  `rejected/setup_invalidated` auditável.
- Testes: guarda central (long/short/tolerância/limit isento) em
  `execution/mod.rs`; fill de gap e cancelamento em `simulated/broker.rs`.
