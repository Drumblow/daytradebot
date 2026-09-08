# Estratégia: Range Extreme Fade v2

**ID:** `range-extreme-fade-v2`
**Versão:** 2.0.0 (proposta)
**Status:** proposto / especificado (Fases 1–3 do framework) — **NÃO implementado (07/09/2026)**
**Criada:** 2026-09-07
**Base:** `range-extreme-fade-v1` (`docs/strategies/range-extreme-fade-v1.md`), aprovada para acumular amostra em AVUV/SLYV/IWV (gate A de 04/09/2026: 64 OOS, PF 2,10)
**Fonte:** Al Brooks, *Reading Price Charts Bar by Bar*, Cap. 9/10 e Cap. 5; Dalton, *Mind over Markets*, Cap. 3; Murphy, *Technical Analysis of the Financial Markets*, Cap. 15
**Plano-mestre:** `docs/cto-plano-lucratividade-2026-09.md` §6.1 (a v2), §2.3 achado 5 (o problema), §5.4 (hotfix v1.0.1)
**Pesquisa de origem:** proposta `setups-novos-1` / `edge-existente-5` e as 6 críticas correspondentes (06–07/09/2026)

**Regra de leitura:** a v2 é a v1 **com uma única mudança** — a política de contexto `allow_neutral_context` (ContextPolicy por estratégia). Nada na v1 muda (framework §4; AGENTS.md §6). A única exceção é o hotfix v1.0.1 do veto de meio-dia (§13), que é correção de bug com nota, precedente A2. Todos os números deste documento são a 2 bp/lado e, salvo indicação, **sem** o flatten de fim de sessão (plano-mestre §2.3 achado 1); nenhum veredito vale antes do flatten no backtest (ADR-018, proposto).

---

## 1. Fonte

- **Brooks, Cap. 10, "Selecting a Market":** "maybe 80 percent of days" são trading range days; as melhores entradas do dia são "second entries in the form of reversals at new swing highs and lows on non-trending days" (`docs/books/analysis/brooks-bar-by-bar.md` §1 e §2.6).
- **Brooks, Cap. 9, "Failed Higher High and Lower Low Breakouts":** "Most days are trading range days and offer many entries on failed swing high and low breakouts" — a fonte do setup da v1 (§2.6 da análise).
- **Brooks, Cap. 5, "Barb Wire" e "Middle of the Day, Middle of the Range":** vetos literais da v1, mantidos na v2 sem alteração (§2.7 da análise). A regra do meio do dia é "meio do dia **e** terço central do range" — não há regra de hora isolada no livro.
- **Dalton, Cap. 3, "Trending versus Bracketed Markets":** "markets trend only 20 to 30 percent of the time" (`docs/books/analysis/dalton-mind-over-markets.md` §2.8; a tabela §4 da análise registra que a frase "não vira regra; justifica priorizar setups de range/reversão").
- **Murphy, Cap. 15, DMI/ADX:** "A falling ADX line suggests a trading range and favors oscillators" (`docs/books/analysis/murphy-technical-analysis.md` §3.12). Usado aqui **só como fundamento conceitual** de que o regime de range favorece fades; o ADX **não** entra na v2 (§14, item 6).

**O que a v2 não tem de fonte — e por isso é falsificável, não literal:** a exigência global de `trend_state ∈ {Uptrend, Downtrend}` em `is_tradeable` nasceu na era da `pullback-trend-v1` e nunca teve capítulo de livro para a range-fade. A v2 **não adiciona regra**: remove, para esta estratégia, um filtro sem fonte, e deixa o detector de dia de range da v1 (§4.1 daquele doc — interpretação nossa de Cap. 9/10) como o único critério de contexto. Tudo o que é "interpretação nossa" está marcado como tal em §8.

## 2. Conceito em uma frase

A estratégia dos ~80% de dias sem tendência (Brooks) está sendo vetada, pelo gate global de contexto, justamente nas barras que o contexto classifica como `Neutral` — a v2 dá à estratégia o direito de operar nessas barras, mantendo todos os outros vetos (volatilidade alta, fase da sessão, Barb Wire, meio do dia/meio do range, dia não-range).

## 3. O problema (medido)

`is_tradeable = trend ∈ {Up, Down} ∧ vol ≠ High ∧ phase == Regular` (`crates/trader-core/src/context/mod.rs:79-81`; `classify_trend` em `:120-128` exige `close > EMA20 > SMA200` ou o inverso). O **único** lugar que rejeita por isso é `RiskManager::validate` (`crates/trader-core/src/risk/mod.rs:158-163`, `RejectionReason::NoContext`); `engine.rs` e `paper.rs` não consultam `is_tradeable` (grep: fora do core só a definição no domínio, `crates/trader-domain/src/context.rs:59`, e a persistência em `market_context_repository.rs` — nenhuma decisão). A `range-extreme-fade-v1` tem o próprio analisador (`range_extreme_fade_v1/mod.rs:38-51`) e **nunca lê `trend_state`** — o fluxo em `mod.rs:104-146` é janela → ATR → ATR diário → `check_range_day` → meio-do-dia → Barb Wire → setup. Ou seja: a estratégia aprova o sinal por conta própria e o RiskManager o derruba por um critério que ela não conhece.

Medição direta do crítico de implementação (re-simulação dos críticos, 06/09/2026; backtest da v1 a 2 bp com `RUST_LOG=trader_backtest=debug,trader_core::execution=debug`, 24/02/2025 → 02/09/2026):

| Par | Sinais rejeitados por `NoContext` | Entradas executadas | Trades (run) | PF in-sample |
|---|---|---|---|---|
| AVUV | **24** | 31 | 29 (run 543) | 1,63 |
| SLYV | **18** | 30 | 21 (run 545) | 3,04 |
| IWV | **31** | 35 | 19 (run 547) | 1,15 |

A contagem inclui repetições em barras consecutivas do mesmo setup (não vira 1:1 em trades), mas a ordem de grandeza é essa: 43% dos sinais que a estratégia aprova morrem no `is_tradeable` (73 de 169; 24/18/31 por par — o "~60%" era 146/242, com o numerador dobrado por log duplicado). Proxy nos candles (mede **barras**, não sinais; regra EMA20/SMA200 de 15m reproduzida pelos críticos): `Neutral` = 39–42% de todas as barras RTH e 44–49% das barras de dia de range / EMA20 plana (AVUV 47–49%, SLYV 45–48%, IWV 40–44%, conforme o proxy). Em produção, 36% das barras foram `neutral` em 31/08–02/09 (`docs/reports/pregoes-2026-08-31_a_09-02.md:30-36`).

**O que o problema NÃO é** (crítica estatística e de código, 06/09): (a) o proxy `Neutral` **não discrimina** dia de range — 44,4% das barras de dia de range são Neutral contra 41,9% em dias de tendência; (b) "EMA20 plana" (< 0,05%/barra em 12 barras) cobre 80–89% de todas as barras, e 53–60% das barras planas são Up/Down — a afirmação "o detector da v1 coincide com Neutral por construção" é **falsa**. O argumento válido é o do **detector contraditório** (§3, 1º parágrafo), não "Neutral = range". O PF dos sinais bloqueados é **desconhecido em todo o projeto** — é o que a v2 mede.

## 4. Hipótese falsificável (pré-registrada em 07/09/2026)

**H1.** Os trades que só existem com `allow_neutral_context = true` (o "delta neutral-only": presentes na rodada com a flag e ausentes na rodada sem, casados por `entry_time`), somados nos 3 pares vivos, têm **≥ 30 trades OOS** (walk-forward 6 janelas, com flatten), **PF ≥ 1,3**, e **avg R ≥ 0 por direção (long/short) e por ano (2025/2026)**.

**H2.** A v2 agregada (v1 + delta) passa o gate A **com flatten** (ADR-010 + os critérios de ADR-019 propostos: IC95 em blocos do PF ≥ 1,0, PF_R ≥ 1,2, share dos 2 melhores meses ≤ 60%, holdout travado). Reportar lado a lado com a v1 com flatten no mesmo período (pares vivos: PF 1,74, avg R 0,218) — comparação obrigatória, não critério adicional.

**Se H1 ou H2 falhar, a v2 fica só com o hotfix de §13** (não há v2; a v1 continua como está). Ninguém procura outro limiar, outro par ou outra hora para salvar a hipótese — cada tentativa dessas é ablação separada e conta no N da família (§14, item 6).

**Potência, declarada antes de rodar:** com sd(R) = 1,18 (medido nos pares vivos), SE(avg R) a n = 30 é ≈ 0,22 — PF 1,3 **não se distingue de 1,0** com essa amostra. O critério de H1 é um critério de **morte** (PF < 1,3 mata), não de prova; a prova é H2 + holdout + paper forward. Reportar junto: IC95 do PF do delta por bootstrap em blocos (§5.3 do plano) e P(PF ≥ 1,3 | sem edge) para o n obtido.

**Contagem N:** esta v2 registra 2 configurações (`require_trend` / `allow_neutral`) × 4 pares (AVUV, SLYV, IWV, IWN) = +8 no contador da família range-fade, além das 14 combinações da validação de 17/08 e das tentativas dos críticos (§14, item 7).

## 5. Fase 1 — Extração do conceito (8 perguntas)

Só a pergunta 2 muda em relação à v1 (`range-extreme-fade-v1.md` §3); as demais são herdadas sem alteração.

1. **Nome:** o mesmo da v1 (Failed HH/LL breakout fade em dia de range) com política de contexto por estratégia.
2. **Contexto:** dia sem tendência (~80% dos dias, Brooks Cap. 10; "20–30% do tempo em tendência", Dalton Cap. 3). **Na v2 o contexto é decidido pelo detector da estratégia** (EMA20 plana + sem HH/HL + range do dia < 1,5 × ATR diário — interpretação nossa) e não pelo classificador global de 15m, que exige alinhamento `close/EMA20/SMA200` — condição de **tendência**, contrária ao conceito.
3. **Timeframe:** 15min (v1).
4. **Entrada:** idêntica à v1 (stop 1 tick além da barra de sinal forte, Cap. 9/10).
5. **Stop:** idêntico (1 tick além do extremo oposto da barra de sinal).
6. **Alvo:** idêntico (1,5R fixo; alvo estrutural continua candidato separado).
7. **Quando NÃO operar:** todos os vetos da v1 (breakout forte, meio do dia + meio do range, Barb Wire, lado errado da EMA quando ligado, dia de tendência) **mais** os vetos globais que sobrevivem à v2: volatilidade `High` e fase ≠ `Regular` (§6).
8. **Estatísticas do autor:** as da v1. Nenhuma estatística do livro fala do incremento Neutral — o número vem do Passo 1 (§6.1).

## 6. Contexto de Mercado — a única mudança

A v2 mantém os cinco filtros de §4 da v1 (dia de range, extremo por pouco, regra da EMA opcional, veto meio-do-dia/meio-do-range, Barb Wire) e muda **só** o que o `RiskManager` faz com `trend_state`:

| Política (`ContextPolicy`) | `trend_state` Neutral | vol `High` | fase ≠ `Regular` |
|---|---|---|---|
| `require_trend` (default; = comportamento atual de todas as estratégias) | rejeita `NeutralContext` | rejeita `HighVolatility` | rejeita `OutsidePhase` |
| `allow_neutral` (v2) | **aceita** | rejeita `HighVolatility` | rejeita `OutsidePhase` |

Detalhe verificado no código: o veto de vol `High` **já é explícito e independente** em `risk/mod.rs:165-170` (roda logo depois do `is_tradeable`); o que hoje só existe dentro de `is_tradeable` é a fase ≠ `Regular`. Liberar Neutral trocando apenas a condição de `:158` **sem** um veto explícito de fase passaria a aceitar sinais fora de `Regular` — por isso a v2 exige os três vetos separados (tabela acima), cada um com o próprio `RejectionReason`.

### 6.1 Passo 1 — medir sem módulo novo (pré-requisito da v2)

Sem código de estratégia. Objetivo: obter o PF do delta antes de escrever `range_extreme_fade_v2`.

1. `RiskConfig.allow_neutral_context: bool` (default `false`; `RiskConfig` é `Copy`, `risk/mod.rs:16-35` — um `bool` cabe) plumbado por `StrategyRiskParams` (`crates/trader-cli/src/risk_config.rs:27-36`; os 9 `impl From` em `:38-153` recebem `allow_neutral_context: false`, mecânico) e por `build_risk_config` (`risk_config.rs:165-201`). Para o Passo 1 a flag precisa de um segundo caminho de entrada, **fora** de `[strategy.parameters]`: `RiskSettings.allow_neutral_context` (`crates/trader-infra/src/config/mod.rs:74`, `[risk]` do `default.toml`, default `false`), que `build_risk_config` combina com o parâmetro da estratégia (`OR`) — o loader já aceita override por env `TRADER__RISK__ALLOW_NEUTRAL_CONTEXT=true` (`config/mod.rs:171`, prefixo `TRADER` com `__`, mesmo mecanismo do plano §5.5). Motivo: a `StrategyParameters` da v1 não tem a chave e não vai ganhá-la (v1 intacta), então um `--set`/TOML em `[strategy.parameters]` seria rejeitado por `deny_unknown_fields` (ADR-019) ou, hoje, ignorado em silêncio — a armadilha que a ADR-019 descreve. Backtest e paper chamam a mesma `build_risk_config` (`backtest.rs:126`): paridade automática.
2. Em `risk/mod.rs:158-163`: rejeitar `NeutralContext` só se `!allow_neutral_context && trend_state == Neutral`; rejeitar `OutsidePhase` se `market_phase != Regular`; manter `HighVolatility` (`:165-170`) como está. `NoContext` deixa de ser emitido por `validate` (fica no enum para compatibilidade com as linhas já persistidas em `signals.rejection_reason`, que é `TEXT` sem CHECK — `crates/trader-infra/src/db/migrations/0001_initial_schema.sql:111`).
3. Override de pesquisa: env `TRADER__RISK__ALLOW_NEUTRAL_CONTEXT=true` (item 1) ou uma flag própria do backtest/walkforward (`--allow-neutral-context`, sugestão dos críticos de código e operação), sempre com `--label rf-neutral` do harness (§5.2 do plano / ADR-019 proposto). **Não** usar `--set` para isto (item 1). **Enquanto o harness não existir**, o run com a flag ligada tem de ser rotulado à mão e **não pode virar baseline** do gate B (hoje `analyze` pega o run mais recente da estratégia em qualquer símbolo, `analyze.rs:75-78` conforme o plano §5.2).
4. Rodar a v1 **com e sem** a flag em AVUV, SLYV, IWV e IWN, com flatten, 2 bp, `-o` JSON; diffar trades por `entry_time`; reportar do delta: n, PF, PF_R, avg R, WR, hora ET, direção, ano, share do melhor dia e dos 2 melhores meses, e a distribuição por hora **comparada** com a da v1.
5. Falsificação barata em produção (crítico operacional): `select rejection_reason, count(*) from signals where strategy_id = 'range-extreme-fade-v1' group by 1` no dump do servidor — ainda não feita; o banco dev só tem sinais da `pullback-trend-v1`, e nos 3 pregões auditados o único sinal da range-fade (AVUV 01/09) caiu por overshoot (ADR-015), não por contexto.

### 6.2 Passo 2 — módulo `range_extreme_fade_v2` (só se H1 e H2 passarem)

Cópia da v1 + parâmetro `allow_neutral_context = true` no TOML + `version = "2.0.0"` + política gravada no `market_snapshot` (`"context_policy": "allow_neutral"`, ao lado dos `trend_state` e `is_tradeable` que a v1 já grava em `range_extreme_fade_v1/entry.rs:95-99`). Sem ADX, sem oscilador, sem veto de hora novo. Arquivos em §12.

## 7. Setup, Entrada, Stop, Alvo e Gestão de Risco

Idênticos à v1 (`range-extreme-fade-v1.md` §5, §6 e §7): barra de sinal forte da família `opening-reversal-v1`, extremo do dia ou pivô-2 superado por ≤ `max_extension_atr_mult` × ATR14 (0,5 na calibração de 17/08), entrada stop 1 tick além da barra de sinal com validade 2 candles (ADR-009), stop 1 tick além do extremo oposto, alvo 1,5R, RR mínimo 1,2, risco 1,0% (nominal — o risco real é 0,15–0,32% pelo cap de notional, plano §2.3 achado 4), janela 09:45–15:15 ET, long e short. Guarda de overshoot da entrada (ADR-015) e limites da conta (ADR-017) inalterados.

## 8. Fase 2 — Tabela Subjetivo → Objetivo (só o que muda)

| Conceito (livro) | Regra objetiva (15min) | Origem |
|---|---|---|
| "~80% dos dias são trading range days" (Brooks Cap. 10); "markets trend only 20–30% of the time" (Dalton Cap. 3) | o contexto global de 15m **não** exige tendência para esta estratégia: `allow_neutral_context = true` → `trend_state = Neutral` é aceito pelo `RiskManager` | **interpretação nossa** (remoção de filtro sem fonte; a regra de dia de range continua sendo o detector da v1) |
| "dia de trading range" | detector da v1 (§4.1 do doc v1): EMA20 plana + sem 2+ HH/HL ou LH/LL em 12 barras + range do dia < 1,5 × ATR diário | interpretação nossa de Cap. 9/10 (herdada) |
| "range favorece osciladores/fades, não ferramentas de tendência" (Murphy Cap. 15) | nenhuma regra nova — fundamento conceitual; ADX é ablação separada (§14, item 6) | literal, mas **não implementado** |
| vetos que sobrevivem à política | `volatility_regime = High` → `HighVolatility`; `market_phase ≠ Regular` → `OutsidePhase` | regra do motor (`RiskManager` / `context/mod.rs:79-81`), sem livro — herdada, não nova |
| "meio do dia, meio do range: não operar" (Cap. 5) | veto 11:30–14:00 **ET** ∧ terço central — igual à v1, corrigido pelo hotfix §13 | literal, Cap. 5 |

Todas as demais linhas da tabela da v1 (§8 daquele doc) valem sem alteração.

## 9. Fase 3 — Especificação Técnica

```text
Inputs (iguais à v1):
  - candles 15min do dia corrente + histórico (mín. 40 barras; ATR diário exige dias anteriores)
  - EMA20, ATR14 (15min), ATR14 diário médio, pivôs de 2 barras
  - config da estratégia (TOML) — campo novo: allow_neutral_context: bool (Passo 2;
    no Passo 1 a flag entra por [risk]/env, §6.1 item 1)
  - MarketContext do analisador GLOBAL (engine.rs:127-128 / paper.rs:155-156,
    ContextAnalyzerConfig::default), que é o que o RiskManager recebe via
    ExecutionEngine (execution/mod.rs:121)

Outputs:
  - Signal (long|short) | Rejected(RejectionReason, detalhes)
  - entrada (stop), stop, alvo 1,5R — iguais à v1
  - market_snapshot: tudo da v1 + "context_policy": "allow_neutral" | "require_trend"

RiskManager::validate (ordem dos vetos de contexto):
  1. market_phase != Regular            -> OutsidePhase
  2. volatility_regime == High          -> HighVolatility   (já existe, risk/mod.rs:165-170)
  3. atr_pct > max_atr_pct              -> HighVolatility   (já existe)
  4. trend_state == Neutral && !allow   -> NeutralContext
  (NoContext não é mais emitido; fica no enum por compatibilidade)

Estado interno: nenhum (stateless, como a v1)
Eventos: fechamento de candle 15min na janela 09:45–15:15 ET
Paridade: automática — o RiskManager é o mesmo no engine e no paper; a flag
  vem do mesmo TOML pelos dois caminhos (build_risk_config).
```

Esboço (Passo 1, `risk/mod.rs`, só para fixar a semântica — não é código final):

```rust
// Contexto de mercado: três vetos independentes e auditáveis.
if !matches!(ctx.market_phase, MarketPhase::Regular) {
    return RiskCheck::Rejected(RejectionReason::OutsidePhase, "fora da sessão regular".into());
}
// (HighVolatility: blocos existentes em :165-182, inalterados)
if matches!(ctx.trend_state, TrendState::Neutral) && !self.config.allow_neutral_context {
    return RiskCheck::Rejected(RejectionReason::NeutralContext, "contexto neutro e política exige tendência".into());
}
```

## 10. Rejeições Registradas pelo Bot

Reuso (v1): `OutsideTradingHours`, `IncompleteSetup`, `WeakConfirmation`, `StopTooWide`, `PoorRiskReward`, `MaxTradesReached`, `DailyLossLimitReached`, `ConsecutiveLosses`, `HighVolatility`, `NotARangeDay`, `BreakoutTooStrong`, `WrongSideOfEma`, `MiddayMidrange`, `BarbWire`, `SetupInvalidated`, `PositionAlreadyOpen`.

Novas no domínio (`crates/trader-domain/src/signals.rs:39-147`; adicionar ao teste de round-trip serde em `:275`):

- `neutral_context` — `trend_state = Neutral` com política `require_trend` (substitui o uso atual de `no_context` no `RiskManager`).
- `outside_phase` — `market_phase ≠ Regular` (hoje só vetado por dentro de `is_tradeable`; nota: `classify_market_phase` em `context/mod.rs:150-163` usa janela UTC 13:30–21:00, larga o bastante para DST e EST, e a janela da estratégia é mais estreita — o veto deve ser raro, mas precisa existir).
- `no_context` — mantido no enum; deixa de ser emitido. Nenhuma migração: `signals.rejection_reason` é `TEXT` sem CHECK.

Motivo de dividir: o count de cada razão passa a aparecer em `signals.rejection_reason` no paper, e a hipótese de §4 vira auditável em produção sem log de debug.

## 11. Plano de Testes Unitários (candles sintéticos)

**RiskManager (`risk/mod.rs:326+`, usar `make_context` de `:334`):**

1. Contexto `Neutral`, `allow_neutral_context = false` (default) → `NeutralContext`. (Substitui a asserção atual de `NoContext`, se houver.)
2. Contexto `Neutral`, `allow_neutral_context = true` → sinal aprovado (com stop, RR e janela válidos).
3. Contexto `Neutral` + `volatility_regime = High`, `allow_neutral_context = true` → **`HighVolatility`** (vol High continua rejeitada com a política liberada).
4. Contexto `Neutral` + `atr_pct > max_atr_pct`, flag `true` → `HighVolatility`.
5. Contexto `Uptrend` + `market_phase = PreMarket`, qualquer política → `OutsidePhase`.
6. Contexto `Uptrend`, flag `false` → aprovado (regressão: nada muda para as outras 8 estratégias).
7. `RiskConfig::default().allow_neutral_context == false` e os 9 `From<&…Params>` produzem `false` (regressão de plumbing).
8. `build_risk_config` com `[risk].allow_neutral_context = true` (Passo 1) ou com o parâmetro da estratégia `true` (Passo 2) → `RiskConfig` com a flag; os dois `false` → `false`. E a chave `allow_neutral_context` posta em `[strategy.parameters]` de um TOML da **v1** deve **falhar** o parse (`deny_unknown_fields`, ADR-019) — hoje seria ignorada em silêncio e o run mentiria que houve override.

**Módulo v2 (Passo 2; cópia dos 13 testes de `range_extreme_fade_v1/tests.rs` + os seguintes):**

9. `market_snapshot` carrega `"context_policy": "allow_neutral"`.
10. `config_hash` da v2 difere do da v1 com os mesmos demais parâmetros (a política entra no hash).
11. `id() == "range-extreme-fade-v2"`, `version() == "2.0.0"`.

**Hotfix v1.0.1 (§13):** testes 12–14 listados lá.

## 12. Decisões de Implementação

### Passo 1 (medição; sem módulo v2)

| Arquivo | Mudança |
|---|---|
| `crates/trader-core/src/risk/mod.rs:16-35` | campo `allow_neutral_context: bool` em `RiskConfig` (+ `Default` = `false`, `:37-53`) |
| `crates/trader-core/src/risk/mod.rs:158-163` | trocar o bloco `!is_tradeable → NoContext` pelos vetos `OutsidePhase` / `NeutralContext` (§9); `:165-182` intactos |
| `crates/trader-domain/src/signals.rs:39-147, :275` | variantes `NeutralContext`, `OutsidePhase` + casos no teste serde |
| `crates/trader-cli/src/risk_config.rs:27-36, :38-153, :165-201` | campo em `StrategyRiskParams`, os 9 `impl From` (todos `false`), `build_risk_config` (flag = `[risk]` **ou** parâmetro da estratégia) |
| `crates/trader-infra/src/config/mod.rs:74` | `RiskSettings.allow_neutral_context: bool` (serde default `false`) — caminho do override de pesquisa por `[risk]`/env no Passo 1 (§6.1 item 1) |
| `crates/trader-core/src/strategies/range_extreme_fade_v1/config.rs:27-90` | **nada** (a v1 não ganha a chave; a flag de pesquisa entra por `[risk]`/env, nunca por `[strategy.parameters]` da v1) |
| `crates/trader-cli/src/commands/walkforward.rs:18-25` | depende do harness §5.2: hoje só `backtest.rs:29,36` tem `-o` e `--slippage-bps` (e é `Option<u32>`); o walk-forward não tem nenhum dos dois |

Sem ADR própria na proposta do plano (§6.1): é campo de configuração com default de paridade, e a mudança de `RejectionReason` entra na ADR-019 (harness), que já mexe no relatório por razão de rejeição. Ressalva: o bloco toca o `RiskManager::validate` de todas as 9 estratégias; pela regra de AGENTS.md §4.1 (item 3: mudança estrutural registra ADR), cabe ao dono decidir se isto exige uma ADR curta antes do Passo 1 (§18, item 2).

### Passo 2 (módulo v2)

| Arquivo | Mudança |
|---|---|
| `crates/trader-core/src/strategies/range_extreme_fade_v2/{mod,context,setup,entry,config,tests}.rs` | cópia da v1; `config.rs` ganha `allow_neutral_context: bool`; `entry.rs` grava `context_policy` no snapshot; `mod.rs` devolve id/versão 2.0.0 |
| `crates/trader-core/src/strategies/mod.rs:3-11` | `pub mod range_extreme_fade_v2;` |
| `crates/trader-cli/src/dispatch.rs:30, :59-61, :90, :105, :120, :136, :152-213` | variante `RangeExtremeFadeV2` no enum e nos 10 `match` (load, `entry_validity_candles`, `config_hash`, `risk_params`, `time_exit`, `id`, `name`, `source`, `version`, `analyze`) |
| `crates/trader-cli/src/risk_config.rs` | 10º `impl From<&RangeFadeV2Params>` com `allow_neutral_context: p.allow_neutral_context` |
| `config/strategies/range-extreme-fade-v2.toml` | v1 + `allow_neutral_context = true` + `version = "2.0.0"` + `source` citando Brooks Cap. 9/10/5, Dalton Cap. 3, Murphy Cap. 15; veto de meio-dia já em ET (§13) |
| `crates/trader-web/src/instances.rs:21-32` | tabela hardcoded instância → `strategy_id` → `client_id` (hoje `avuv-rangefade` 9, `slyv-rangefade` 10, `iwv-rangefade` 11) — nova linha ou troca, conforme §18 |
| `deploy/home/docker-compose.yml:155-190` e `deploy/home/trader-containers.sh:8-11` | serviço/env da instância v2 e a lista `INSTANCES` do scheduler (não estavam na proposta original — apontado pelo crítico de implementação) |
| `docs/strategies/range-extreme-fade-v2.md` | este doc, com §16/§17 preenchidos |

Esforço: **M** (confirmado pelos críticos) — o grosso é plumbing mecânico e a cópia do módulo; a parte difícil é a medição, não o código.

## 13. Hotfix v1.0.1 — veto de meio-dia em ET (seção separada; correção, não melhoria)

**Bug.** `is_midday_midrange` (`crates/trader-core/src/strategies/range_extreme_fade_v1/context.rs:220-238`) compara `last.timestamp.time()` **em UTC** com `midday_start_time`/`midday_end_time` do TOML, que estão em UTC fixo de horário de verão (`config/strategies/range-extreme-fade-v1.toml:26-29`: `"15:30:00"`–`"18:00:00"`, comentário "Em UTC, horário de verão americano"). O comentário no código ("comparar em UTC para consistência com `check_trading_hours`") ficou obsoleto: A2 (`e4231f3`, 30/08/2026) converteu `check_trading_hours` para ET (`context.rs:308-329`, via `crate::session::et_time`/`parse_et_time`) mas **não tocou o veto de meio-dia** — o diff daquele commit no TOML da range-fade só altera `trading_start_time`/`trading_end_time` e **apaga** o comentário "Ajustar na virada do DST" do bloco do veto sem converter os valores (`git show e4231f3 -- config/strategies/range-extreme-fade-v1.toml`). Resultado: no horário de inverno (EST) o veto cobre **10:30–13:00 ET** em vez de 11:30–14:00 ET; a regra documentada (`range-extreme-fade-v1.md` §4.4/§8: 11:30–14:00 ET, literal Cap. 5) não é a que roda de novembro a março. ~24% da amostra do gate A foi medida com o veto deslocado (plano §2.3 achado 3).

**Correção (v1.0.1).**
- `context.rs:220-238`: `let time = crate::session::et_time(last.timestamp)`; `parse_et_time(&params.midday_start_time)` / `..._end_time` (`crates/trader-core/src/session.rs:18, :26`) — mesmo padrão de `check_trading_hours`; remover o closure `parse` local.
- TOML: `midday_start_time = "11:30:00"` / `midday_end_time = "14:00:00"` com comentário "horário de Nova York (ET); convertido com chrono-tz, não ajustar no DST". Idem nos defaults de `config.rs:121-122` (`"15:30:00"`/`"18:00:00"` hoje).
- Testes (adicionar em `range_extreme_fade_v1/tests.rs`): 12. timestamp de **julho** 12:30 ET, preço no terço central → `MiddayMidrange`; 13. timestamp de **janeiro** 12:30 ET (= 17:30 UTC), terço central → `MiddayMidrange` (hoje passa, porque 17:30 UTC está na janela; o teste vale como regressão do horário certo); 14. timestamp de **janeiro** 10:45 ET (= 15:45 UTC, dentro da janela UTC antiga), terço central → **sinal não vetado** (prova que o deslizamento acabou).
- **`config_hash` muda.** Nota formal de correção no doc da v1 (§16, adendo datado), **sem bump do campo `version`** — o precedente A2 mudou 9 TOMLs sem alterar `version` (verificado no diff de `e4231f3`); o rótulo "v1.0.1" é da nota e do changelog, não do `strategy_version` gravado. Re-rodar o walk-forward da v1 junto com o flatten (ADR-018) para o gate B ter baseline coerente (plano §5.4).
- **Efeito em P&L: imensurável** — a subamostra EST tem ≈ 20 trades. É correção de paridade doc ↔ código, não hipótese de lucro; **não** entra no N de tentativas.

**Este documento não edita a v1**; a nota entra em `range-extreme-fade-v1.md` quando o hotfix for aplicado. A v2 nasce da v1 **já corrigida** — o TOML da v2 já vem em ET.

## 14. Ressalvas dos críticos (06–07/09/2026)

1. **`Neutral` não é "range".** Inclui transições de tendência (whipsaw), chop multi-dia com EMA20 e SMA200 cruzadas, e repiques contra-tendência (`close > EMA20` com `EMA20 < SMA200`). Nesses casos o fade de novo extremo do dia parece mais Barb Wire do que o range day do Cap. 9 — e o veto Barb Wire da v1 só olha 3 barras. O detector de dia de range da v1 continua ativo e é o que filtra; se não bastar, H1 falha e ponto.
2. **O long da v1 é outra população.** Nos pares vivos: long n = 33, PF 3,34 (t = 2,79); short n = 36, PF 1,00 (re-simulação dos críticos, 06/09/2026; outro crítico da lente estatística, na proposta `edge-existente-5`, registrou 2,22 / 1,28 — a direção do achado é a mesma). O long da v1 é "comprar a nova mínima do dia com `close > EMA20 > SMA200`" — reversão **a favor** do contexto de 15m. Liberar Neutral adiciona fades **sem** esse vento. Por isso H1 exige avg R ≥ 0 por direção, e o relatório separa o delta em long/short.
3. **O teto do delta é ~+100%, não a expectativa.** O scanner de 17/08 (§12.1 do doc v1, variante C) via ~52 sinais/ativo em 17,5 meses contra 19–29 trades nos runs 543/545/547; fill rate com validade de 2 candles, janela, cancelamento por overshoot (ADR-015) e colisões reduzem isso. "+80%" e "×1,6–1,9" das propostas são limites superiores.
4. **Mais sinais pressionam `max_trades_per_day = 3`** (`config/default.toml:31`) **e as 3 posições / 200% da conta** (`default.toml:37-38`, ADR-017). AVUV já tem **duas instâncias que se bloqueiam** (`avuv-balance`, client 6, e `avuv-rangefade`, client 9 — `instances.rs:21-32`; uma posição por símbolo). SLYV **não** roda balance-area (erro factual da proposta original). Medir a sobreposição de dias antes de prometer amostra ao gate B. Não responder com teto por direção/cluster: dias com ≥ 2 entradas têm PF 2,69 contra 0,70 com 1 entrada (plano §2.3 achado 6).
5. **Reinício do gate B.** Se a v2 substituir a v1 em AVUV/SLYV, a amostra live acumulada da v1 é descartada e o gate B recomeça (ADR-010: 4 semanas, ≥ 20 trades, métricas dentro de ±30% do backtest) — hoje a 0,60 trade/pregão (232 trades em 384 pregões nos 8 pares vivos; o 0,74 era com 9 pares, incluindo a pullback já desligada) e com 0 trades das aprovadas desde 18/08. A regra do plano (§6) é v2 em símbolo onde a v1 **não** roda, com a v1 como controle; mas para a range-fade os candidatos (IWN/IJR/MDY) ficaram ~1,0 com flatten na re-simulação, e IWN já tem a `opening-reversal-v1` (bloqueio mútuo por símbolo). Decisão do dono (§18), não deste doc.
6. **NÃO incluir ADX, oscilador nem veto 12h–14h.** São ablações separadas, cada uma contada no N. O veto de hora "independente da posição no range" **não é a regra do Cap. 5** (meio do dia **e** terço central) — alargar é regra sem fonte. O bucket 12h–14h nos pares vivos é n = 30, −US$ 543, PF 0,87, avg R 0,00, t = 0,02: **ruído, não perda**, e 43% da amostra — vetá-lo contraria o objetivo (amostra). O PF 0,44 do mesmo bucket em IJR/IWN/MDY vem de ativos fracos o dia inteiro (não rodam). ADX(14) diário reamostrado precisa de ~28 pregões de aquecimento e o live entrega 600 candles ≈ 23 pregões — quebraria a paridade (C4).
7. **Viés de seleção e potência.** O walk-forward não é OOS em relação ao desenho da regra (estratégias são funções puras; o "OOS" é a mesma rodada determinística após o 1º bloco). A família range-fade já tem os 14 ativos do backtest de 17/08 (runs 210–223, doc v1 §16), as 8 configurações de §4 (o crítico estatístico registrou 2 políticas × 14 símbolos = 28 se a medição for estendida a todos) e as ablações dos críticos. Reportar DSR como faixa, IC95 do PF em blocos (v1 OOS: iid [1,20; 3,90], blocos [1,39; 3,57]) e holdout travado; a prova é o paper forward.
8. **Toca o `RiskManager` de todas as 9 estratégias.** `default = false` preserva o comportamento; os testes 6–7 de §11 são a regressão. Paridade live == backtest é automática (mesmo `RiskManager`, mesma `build_risk_config`).
9. **Tudo aqui é in-sample e sem flatten** salvo indicação. Com flatten a v1 nos pares vivos cai de PF 1,88 / avg R 0,297 / +6.015 para **1,74 / 0,218 / +4.578** (9 trades overnight = +1.933, 32% do P&L). O delta só é medido depois de ADR-018.
10. **Walk-forward em 14 símbolos exige reingerir** IJR, MDY, SCHA, VB, SPY, QQQ (parados em 06/08/2026 no banco dev — plano §5.6). Não é pré-requisito para os 4 pares de §4.
11. **IWV sai** (plano §5.10: PF 1,15 in-sample a 2 bp, avg R −0,018, stop mediano 13 bp). Entra na medição de §6.1 por ser par vivo com dado, não como candidato a paper da v2.

## 15. Filtros de Horário e Ativo

- Janela: 09:45–15:15 ET (v1), veto de meio-dia 11:30–14:00 ET (após §13).
- Medição (Passo 1): AVUV, SLYV, IWV, IWN — os 3 pares vivos mais o candidato small-cap value com histórico completo.
- Paper forward (Passo 2): 1–2 pares, escolhidos por §18; nunca junto com outra troca de par na mesma semana (plano §3.8).

## 16. Métricas de Avaliação e comandos

Gate A (ADR-010) **com flatten** e os adendos propostos na ADR-019: ≥ 50 OOS, WR ≥ 40%, PF ≥ 1,3, DD ≤ 10%, avg R > 0,15; PF_R ≥ 1,2; IC95 em blocos do PF ≥ 1,0; share dos 2 melhores meses ≤ 60%; pass no holdout travado. Mais os critérios de H1 sobre o delta (§4).

```bash
# Passo 1 — baseline v1 (2 bp, JSON por trade). Hoje só o backtest tem -o e --slippage-bps.
trader-cli backtest -s AVUV --strategy range-extreme-fade-v1 --from 2025-02-24 --to 2026-09-02 -t 15m --slippage-bps 2 -o out/rf-base-AVUV.json
# Contagem das rejeições de contexto sem harness (comando usado pelo crítico em 06/09):
RUST_LOG=trader_backtest=debug,trader_core::execution=debug trader-cli backtest -s AVUV --strategy range-extreme-fade-v1 --from 2025-02-24 --to 2026-09-02 -t 15m --slippage-bps 2

# Passo 1 — com a flag. NÃO usar --set (patcha [strategy.parameters]; a v1 não tem a chave — §6.1 item 1).
# O override entra pelo [risk] via env (o loader já aceita TRADER__RISK__*) ou por flag própria do comando;
# --label é a sintaxe do harness §5.2/ADR-019, ainda não implementada:
TRADER__RISK__ALLOW_NEUTRAL_CONTEXT=true trader-cli backtest -s AVUV --strategy range-extreme-fade-v1 --label rf-neutral --from 2025-02-24 --to 2026-09-02 -t 15m --slippage-bps 2 -o out/rf-neutral-AVUV.json
# idem SLYV, IWV, IWN. Delta = trades em rf-neutral-*.json ausentes em rf-base-*.json (chave: entry_time).

# Walk-forward OOS (6 janelas), com e sem a flag, após ADR-018 (flatten):
trader-cli walkforward -s AVUV --strategy range-extreme-fade-v1 --from 2025-02-24 --to 2026-09-02 -t 15m -w 6
TRADER__RISK__ALLOW_NEUTRAL_CONTEXT=true trader-cli walkforward -s AVUV --strategy range-extreme-fade-v1 --label rf-neutral-wf --from 2025-02-24 --to 2026-09-02 -t 15m -w 6

# Passo 2 — v2 como estratégia própria:
trader-cli backtest -s AVUV --strategy range-extreme-fade-v2 --from 2025-02-24 --to 2026-09-02 -t 15m --slippage-bps 2 -o out/rf-v2-AVUV.json
trader-cli walkforward -s AVUV --strategy range-extreme-fade-v2 --from 2025-02-24 --to 2026-09-02 -t 15m -w 6

# Paper (após 4 semanas): a política e o trend_state ficam no market_snapshot do sinal.
# SELECT market_snapshot->>'trend_state', count(*), sum(net_pnl) FROM trades t JOIN signals s ON s.id = t.signal_id
#  WHERE t.strategy_id = 'range-extreme-fade-v2' GROUP BY 1;
```

Relatório obrigatório do delta: n, PF, PF_R, avg R (com t-stat), WR, por direção, por ano, por hora ET, por bloco do walk-forward; share do melhor dia e dos 2 melhores meses; IC95 bootstrap em blocos; corr(risk_amount, R); fração de sinais do delta cancelados por overshoot; dias em que balance-area e range-fade sinalizam AVUV juntas.

## 17. Checklist de Validação e critério de veredito

```text
[x] Fase 1 — extração do conceito com citações (este documento, §1/§5)
[x] Fase 2 — tabela subjetivo → objetivo (§8)
[x] Fase 3 — especificação técnica (§9)
[x] Hipótese e critério de morte pré-registrados antes de qualquer rodada (§4, 07/09/2026)
[ ] ADR-018 (flatten no backtest) aplicada — pré-requisito de qualquer número
[ ] Hotfix v1.0.1 do veto de meio-dia aplicado (§13) + walk-forward da v1 re-rodado
[ ] Harness §5.2 (--label/--holdout, PF_R, métricas por dia/direção/hora) — ou override por [risk]/env com run rotulado à mão (§6.1 item 3)
[ ] Passo 1: flag em RiskConfig/StrategyRiskParams; NoContext dividido; testes 1–8 de §11 passando
[ ] Passo 1: A/B em AVUV/SLYV/IWV/IWN com flatten; delta medido e relatado (§16)
[ ] Veredito de H1 e H2 registrado neste doc (§16 → tabela de resultados), inclusive se negativo
[ ] Passo 2 (só se H1 e H2 passarem): módulo v2, TOML, dispatch, instances.rs, compose; testes 9–11
[ ] Replay de portfólio com 3 posições (plano §6.4) para estimar trades/pregão após travas
[ ] Decisão do dono sobre símbolo e reinício do gate B (§18)
[ ] Paper forward 4 semanas, ≥ 20 trades, ±30% do backtest com flatten (ADR-010 B)
```

**Critério de veredito (uma frase):** a v2 existe se, e só se, o delta neutral-only tiver ≥ 30 OOS com PF ≥ 1,3 e avg R ≥ 0 por direção e por ano **e** a v2 agregada passar o gate A com flatten; qualquer outro resultado arquiva a v2 com número e deixa a v1 (com o hotfix) como está. Sem "quase passou", sem novo limiar.

## 18. O que só o dono decide

1. **Onde a v2 roda em paper:** substituir a v1 em AVUV/SLYV (reinicia o gate B, descarta a amostra live da v1, sem controle no mesmo símbolo) ou entrar num símbolo sem v1 (IWN — mas a v1 lá é ~1,0 com flatten e o símbolo já tem a openrev). Se o delta medido em IWN for positivo, a segunda opção passa a ter evidência própria; se não, é a primeira ou nada.
2. **Aceitar o Passo 1 antes de existir a v2:** a flag entra no `RiskConfig` compartilhado (default de paridade) para medir; se H1 falhar, a flag fica como infra de pesquisa e nenhum TOML de produção a liga. Decidir também se a troca do bloco `NoContext` em `RiskManager::validate` (afeta as 9 estratégias) merece ADR curta (AGENTS.md §4.1, item 3) ou basta o registro na ADR-019 (§12).
3. **Hotfix como nota (sem bump de `version`)**, conforme §5.4 do plano e o precedente A2 — ou bump formal para "1.0.1" no TOML, que muda `strategy_version` gravado e obriga a tratar as linhas antigas nos relatórios.
