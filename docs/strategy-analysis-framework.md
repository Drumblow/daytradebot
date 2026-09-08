# Framework de Análise e Implementação de Estratégias

Este documento define o processo padrão para transformar conceitos de livros de trading em regras objetivas, código Rust e testes auditáveis. Deve ser seguido para **toda nova estratégia** adicionada ao robô.

---

## 1. Objetivo

Garantir que:

* toda estratégia tenha origem documentada;
* conceitos subjetivos sejam convertidos em regras mensuráveis;
* a implementação seja testável unitariamente, em backtest e em paper trading;
* o histórico de decisões seja auditável;
* novas estratégias não quebrem estratégias antigas.

---

## 2. Fontes de Estratégia

Livros aprovados como base inicial:

| Livro | Foco | Quando usar |
|---|---|---|
| **Trading Price Action Trends — Al Brooks** | Tendências, pullbacks, continuidade | Setup principal do MVP (pullback em tendência) |
| **Trading Price Action Trading Ranges — Al Brooks** | Ranges, reversões, false breakouts | Fases futuras, quando o bot já operar com tendência |
| **Reading Price Charts Bar by Bar — Al Brooks** | Price action barra a barra; failed breakouts, opening reversals | Setups de range/reversão e janela de abertura (análise: `docs/books/analysis/brooks-bar-by-bar.md`) |
| **The Art and Science of Technical Analysis — Adam Grimes** | Estrutura de mercado, pullbacks validados, failure test (spring) | Fonte da `failure-test-long-v1` (análise: `docs/books/analysis/grimes-art-science-ta.md`) |
| **Mind over Markets — James Dalton** | Market Profile: value area, initial balance, tipos de dia | Setups de abertura/balance via proxy de candles (análise: `docs/books/analysis/dalton-mind-over-markets.md`) |
| **Advances in Financial Machine Learning — López de Prado** | Validação: triple-barrier, purged CV, Deflated Sharpe, meta-labeling | Método de validação/backtest, não setups (análise: `docs/books/analysis/lopez-afml.md`) |
| **Technical Analysis of the Financial Markets — John J. Murphy** | Livro-texto canônico: Donchian/weekly rule, pivot points intraday, Starc/Keltner, ADX, objetivos medidos | Fonte de `pivot-point-intraday-v1`, `keltner-breakout-v1`, `donchian-channel-v1` e dos filtros de regime (análise: `docs/books/analysis/murphy-technical-analysis.md`) |
| **The New Age of Technical Analysis — Brandon Rosewag** | EMAs 5/8/21/55 com papéis fixos, opening range breakout, compressão de volatilidade (squeeze) | ⚠️ Fonte de **menor rigor** (autopublicado, sem estatísticas, exemplos selecionados) — usar só como gerador de hipóteses (análise: `docs/books/analysis/rosewag-new-age-ta.md`) |

Novas fontes podem ser adicionadas, desde que passem pelo mesmo processo de análise.

> Pendente: *Algorithmic Trading — Ernest Chan* (PDF escaneado sem camada de texto completa; requer OCR antes da análise).

---

## 3. Fases do Processo

### Fase 0 — Screener com fill honesto (proposta de 07/09/2026, pendente de aprovação do dono)

> Origem: `docs/cto-plano-lucratividade-2026-09.md` §5.7. Em um dia, um screener em SQL sobre os candles do banco reprovou 7 candidatos que as análises de livro ranqueavam no top-3 — cada um custaria 1–2 semanas de Rust, doc e walk-forward, o destino das 5 estratégias arquivadas.

Antes de escrever qualquer código de estratégia (Fase 4), o setup passa por um mini-backtest em SQL sobre os candles 15m do banco, com o **fill honesto** que o simulador aplica (ADR-015): entrada em `max(open, gatilho)` (long) / `min(open, gatilho)` (short), stop avaliado primeiro na barra do fill, custo de 4 bp ida e volta, saída no fechamento do dia. Ferramentas e regras em `sql/screens/README.md`.

```text
Regra de decisão, declarada ANTES de rodar:
  - avg R líquido agregado com t ≥ 1,5;
  - mesmo sinal em 2025 e em 2026;
  - melhor trimestre ≤ 50% do P&L;
  - ≥ 40 fills/ano nos ativos vivos, contados por DATA (11 ETFs correlacionados são o mesmo dia);
  - o sinal não inverte ao mover o parâmetro principal ±20%.
Reprovou → não vai para a Fase 3. Passou → segue, e o N de variantes testadas
entra no relatório (docs/reports/screens-<data>.md) para a contagem de tentativas.
```

O screener é um filtro de **reprovação**, calibrado contra controles (positivos: `range-extreme-fade-v1` em AVUV e SLYV, na configuração com o hotfix ET (nota v1.0.1) — `config_hash` `818b53394244ca62`, OOS PF 1,47 / avg R 0,159 e PF 2,64 / avg R 0,424; negativos: `pullback-trend-v1` e a `balance-area-breakout-v1` em VBR e AVUV, que sob a régua do live reprovam o gate A com avg R −0,013 e −0,173 e PF em R 0,97 e 0,74 — `docs/reports/gate-a-com-flatten-2026-09-07.md` §3 e §7). A balance-area com flatten serve como **referência de reprodução** do fill do screener (PF 1,55 / avg R −0,007 no agregado in-sample dos 3 pares), não como controle positivo: um controle positivo tem de passar a regra de decisão desta mesma fase, e calibrar o screener para deixá-la passar seria calibrá-lo para aprovar instância sem edge. Ele é mais pessimista que o motor em dias de gap (o motor cancela a entrada quando o gap excede 25% do stop; o screener enche pior) e não modela barra de sinal — um setup cujo edge está na barra de sinal precisa modelá-la explicitamente no screen.

### Fase 1 — Extração do conceito

Ler o capítulo/setup do livro e responder:

```text
1. Qual é o nome do setup?
2. Em qual contexto de mercado ele funciona? (tendência, range, volatilidade alta/baixa)
3. Qual timeframe é recomendado?
4. Quais são os sinais de entrada?
5. Onde colocar o stop?
6. Qual é o alvo ou regra de saída?
7. Quando NÃO operar o setup?
8. O autor cita estatísticas de acerto, R médio ou edge?
```

**Entregável:** `docs/strategies/<nome-do-setup>.md` com o resumo acima e citações do livro.

---

### Fase 2 — Subjetivo → Objetivo

Converter cada elemento vago em regra numérica. Exemplos:

| Conceito subjetivo (livro) | Regra objetiva (código) |
|---|---|
| "tendência de alta forte" | preço acima da EMA 20 por N candles consecutivos, com máximas e mínimas ascendentes |
| "pullback para a média" | preço toca ou penetra a EMA 20 após rompimento |
| "barra de sinal de reversão" | candle com corpo bullish, sombra inferior ≥ 2x corpo, fechamento no terço superior |
| "confirmação" | próximo candle rompe a máxima da barra de sinal |
| "spread alto demais" | spread relativo > X% do preço ou ATR percentual > Y% |
| "perto de notícia" | horário fora do período permitido (ex.: 15min antes/depois de relatório conhecido) |

**Entregável:** tabela de regras objetivas no mesmo arquivo da estratégia.

---

### Fase 3 — Especificação técnica

Definir:

```text
Inputs:
  - candles (timeframe operacional)
  - candles de contexto (timeframe maior)
  - indicadores necessários (médias, ATR, volume)
  - configuração de risco

Outputs:
  - Signal (buy/sell/none)
  - Motivo da entrada
  - Preço de entrada
  - Stop inicial
  - Alvo inicial
  - Risco estimado (R)
  - Motivo da rejeição (se none)

Estado interno:
  - posição atual
  - trades do dia
  - perda acumulada do dia
  - último sinal emitido

Eventos que disparam análise:
  - fechamento de candle
  - atualização de preço em tempo real
```

**Entregável:** seção "Especificação Técnica" no arquivo da estratégia.

---

### Fase 4 — Implementação

A estratégia vive no crate `trader-core`, dividida em:

```text
trader-core/src/strategies/
  <nome_setup>/
    mod.rs              → estrutura pública e trait Strategy
    context.rs          → regras de contexto de mercado
    setup.rs            → detecção do setup
    entry.rs            → regras de entrada, stop e alvo
    config.rs           → parâmetros da estratégia (`Deserialize` +
                          `#[serde(deny_unknown_fields)]`, obrigatório desde o
                          ADR-019: é o que faz `--set chave=valor` e
                          `--strategy-config` do walk-forward falharem fechado
                          em vez de ignorar um parâmetro escrito errado e
                          produzir um run que parece válido)
```

Contrato mínimo (atual):

```rust
pub trait Strategy {
    fn id(&self) -> StrategyId;
    fn name(&self) -> &'static str;
    fn source(&self) -> &'static str; // livro/capítulo
    fn version(&self) -> &'static str;
    fn analyze(&self, ctx: &MarketContext, state: &StrategyState) -> SignalResult;
}
```

**Regras de código:**

* nenhuma regra hardcoded — tudo vem de `config.rs`;
* todo `if` de rejeição deve produzir um `RejectionReason`;
* todo sinal deve carregar metadados auditáveis (valores brutos que originaram a decisão);
* usar `Decimal` para preços, não `f64`;
* usar `chrono::DateTime<Utc>` para timestamps — mas **toda janela de horário do dia (veto, sessão, hora de entrada, fim de pregão) é declarada em horário de Nova York** e comparada por `trader_core::session` (`et_time`, `et_date`, `parse_et_time`, `within_trading_window`). Guardar em UTC continua certo; comparar hora do dia em UTC fixo é o erro — foi assim que o veto de meio-dia da `range-extreme-fade-v1` deslizou 1h fora do DST (hotfix v1.0.1, `docs/strategies/range-extreme-fade-v1.md` §17);
* motivo de saída novo é mudança de **quatro** lugares de uma vez: a variante em `ExitReason`, a linha correspondente em `ExitReason::as_str()`, o braço em `parse_exit_reason` (`crates/trader-infra/src/repositories/trade_repository.rs`) e uma migração que amplie o CHECK de `trades.exit_reason` (precedente: `0004_exit_reason_end_of_day.sql`, do `end_of_day` do ADR-018). `as_str()` não é a tabela que o serde lê: `ExitReason` deriva `#[serde(rename_all = "snake_case")]`, e a igualdade entre as duas tabelas é garantida por teste (`exit_reason_serializa_em_snake_case`), não por reuso. Esquecer cada lugar quebra um lado diferente: sem a migração, o CHECK antigo rejeita a **escrita** do trade (INSERT recusado, SQLSTATE 23514); sem o braço em `parse_exit_reason`, quebra a **leitura** — desde o ADR-018 ela falha fechado (`TryFrom<TradeRow>` no lugar do `From`, sem o `_ => Target` silencioso), então um texto que o enum não conhece vira erro em vez de virar "alvo" nas métricas do gate em silêncio.

---

### Fase 5 — Testes

#### 5.1 Testes unitários com candles sintéticos

Criar séries de candles artificiais que representem:

* setup perfeito (deve gerar sinal);
* setup sem contexto de tendência (deve rejeitar);
* setup com risco-retorno ruim (deve rejeitar);
* setup com spread alto (deve rejeitar);
* setup em horário proibido (deve rejeitar).

```rust
#[test]
fn pullback_em_tendencia_gera_sinal_de_compra() {
    let candles = vec![
        candle!(open: 100.00, high: 101.00, low: 99.50, close: 100.80, volume: 1000),
        // ... mais candles formando tendência e pullback
    ];
    let signal = strategy.analyze(&candles);
    assert_eq!(signal.direction, Direction::Long);
}
```

#### 5.2 Backtest

Rodar a estratégia em dados históricos reais do banco:

```text
- mínimo 6 meses de dados;
- mínimo 50 sinais para começar a avaliar;
- aplicar slippage e comissão (`--slippage-bps`; o gate A de 07/09/2026 roda a
  2 bp + US$ 0,35 por perna);
- respeitar as mesmas regras de risco do live;
- rodar com o flatten de fim de pregão LIGADO (padrão desde o ADR-018): o motor
  encerra no fechamento da última barra do pregão porque o live encerra tudo a
  mercado na janela 15h55–16h10 ET (seção `[session]` de `config/default.toml`,
  lida pelo live E pelo motor). `--no-flatten` existe só para reproduzir runs
  antigos e não vale como evidência;
- nenhum run anterior a 07/09/2026 é comparável com os atuais (mesmo precedente
  do ADR-015).
```

#### 5.3 Paper trading

Só migrar para paper quando:

```text
- testes unitários passarem;
- backtest mostrar edge positivo;
- não houver bugs de execução por pelo menos 1 semana.
```

---

### Fase 6 — Métricas e Diário

Toda estratégia deve produzir:

```text
número de sinais
número de entradas
número de rejeições (por motivo)
número de saídas por motivo (ExitReason: target, stop, end_of_day, manual...)
win rate
profit factor em $ E profit factor em R (o PF em $ infla com o sizing)
média de R por trade e t-stat do avg R
correlação entre o risco do trade e o resultado
drawdown máximo
concentração: melhor dia, 5 melhores dias, 2 melhores meses,
              meses positivos / meses totais
custo total = comissão + taxas (o slippage NÃO entra: já está embutido nos
              preços de execução e não é recuperável do trade — ele aparece
              no relatório como o parâmetro `--slippage-bps` do run)
recortes por direção e por hora ET da entrada
expectativa matemática
razão risco/retorno média
tempo médio na operação
```

O diário automático deve registrar:

```text
setup, ativo, direção, contexto, entrada, stop, alvo,
motivo da entrada, motivo da saída, resultado em R, timestamp.
```

---

### Fase 7 — Validação e aprovação

Antes de uma estratégia ir para produção (paper), ela deve ser aprovada por checklist:

```text
[ ] Documentação da estratégia preenchida
[ ] Regras objetivas definidas
[ ] Especificação técnica completa
[ ] Código revisado
[ ] Testes unitários passando
[ ] Backtest e walk-forward executados COM o flatten (padrão desde o ADR-018)
    e relatório gerado — run com `--no-flatten` não é evidência
[ ] Gate A VIGENTE (ADR-010) atendido no OOS: ≥ 50 trades, WR ≥ 40%,
    PF ≥ 1,3, DD ≤ 10%, avg R > 0,15, net > 0 (o `walkforward` imprime
    o veredito)
[ ] Critérios PROPOSTOS do ADR-019 §7 registrados — não são gate vigente.
    O §7 propõe cinco além dos seis do ADR-010: IC95 do PF por bootstrap
    em blocos ≥ 1,0; PF em R ≥ 1,2; 2 melhores meses ≤ 60%; holdout
    travado; sensibilidade a custo a 4–5 bp. Destes, o `walkforward`
    imprime hoje DOIS sob o rótulo "proposta ADR-019 §7" (PF em R e 2
    melhores meses) — o IC em blocos depende do relatório Python do §8,
    ainda pendente
[ ] Relatório obrigatório do §7 anotado (não é critério, nem proposto):
    t-stat do avg R e corr(risco, resultado), que o CLI imprime fora do
    bloco da proposta, na linha `[ i]`
[ ] Métricas mínimas da própria estratégia atingidas (a definir por estratégia)
[ ] Nenhuma violação de regra de segurança financeira
[ ] Versionada no git
```

---

## 4. Versionamento de Estratégias

Cada estratégia deve ter um identificador fixo e versionado:

```text
pullback-trend-v1
pullback-trend-v2
```

Nunca alterar uma estratégia em produção. Se precisar mudar uma regra, crie uma nova versão e teste do zero.

Correção de bug que não muda a regra pretendida é a única exceção, e ela não é barata: o hotfix da `range-extreme-fade-v1` (veto de meio-dia em UTC fixo → ET) manteve o `strategy_id` **e** a `version` (`1.0.0`; o "v1.0.1" é rótulo da nota de correção — `docs/strategies/range-extreme-fade-v1.md` §17, que diz "Não é bump de versão" —, não campo de config). O que mudou foram os *valores* de `midday_start_time`/`midday_end_time` no TOML; como o `config_hash` é o hash do JSON da configuração inteira, isso já bastou para virá-lo (`49ee6f045b4c35a7` → `818b53394244ca62`). `config_hash` novo em produção é estratégia nova para efeito de amostra: reinicia a janela de 4 semanas do gate B (§3.8 do plano). Por isso o deploy de um hotfix desses é decisão do dono, não consequência de um push.

O banco deve armazenar:

```text
strategy_id: "pullback-trend-v1"
strategy_version: "1.0.0"
strategy_source: "Al Brooks - Trading Price Action Trends, Capítulo X"
config_hash: sha256 da configuração usada
```

---

## 5. Regras de Ouro

1. **Nenhuma regra sem fonte.** Toda regra deve citar o livro/capítulo que a originou.
2. **Subjetivo não entra no código.** Se não conseguimos medir, não implementamos.
3. **Rejeição é tão importante quanto entrada.** Devemos saber por que o bot não entrou.
4. **Nunca mude uma estratégia durante um teste.** Isso invalida as métricas.
5. **Live só depois de paper; paper só depois de backtest; backtest só depois de testes unitários.**
6. **Mantenha o core genérico.** Estratégias são plugins; o core não sabe que estamos operando pullback.

---

## 6. Exemplo Aplicado: Pullback em Tendência (Al Brooks)

**Fonte:** Trading Price Action Trends, Parte III — Pullbacks.

**Conceito:** em uma tendência de alta, o preço eventualmente faz uma pausa (pullback). Se o pullback for pequeno e apresentar uma barra de sinal de reversão bullish, a tendência provavelmente continua.

**Regras objetivas iniciais (rascunho):**

```text
Contexto:
  - timeframe operacional: 15min
  - timeframe de contexto: 1h
  - preço acima da EMA 20 no 1h por pelo menos 10 candles
  - máximas e mínimas ascendentes no 1h

Setup:
  - preço no 15min fez nova máxima (breakout) nos últimos 10 candles
  - pullback toca ou penetra a EMA 20 no 15min
  - pullback tem no máximo 5 candles
  - aparece barra de sinal bullish:
      * corpo positivo
      * sombra inferior ≥ 1.5x o tamanho do corpo
      * fechamento no terço superior da barra

Entrada:
  - buy stop 1 tick acima da máxima da barra de sinal

Stop:
  - 1 tick abaixo da mínima da barra de sinal

Alvo:
  - 2R (duas vezes o risco)

Rejeições:
  - contexto não é tendência de alta
  - pullback ultrapassa 5 candles
  - pullback quebra estrutura (faz lower low abaixo do início do impulso)
  - barra de sinal não atende critérios
  - risco-retorno < 1:2
  - spread relativo > 0.05%
  - perda máxima diária já atingida
```

> Este é um rascunho. Deve ser validado com leitura detalhada do livro e testes antes de virar código definitivo.

---

## 7. Próximos Passos

1. ✅ Ler Parte III de *Trading Price Action Trends* (capítulos sobre pullbacks).
2. ✅ Preencher o template `docs/strategies/pullback-trend-v1.md`.
3. ✅ Refinar as regras objetivas com base nos exemplos do livro.
4. ✅ Criar os testes unitários sintéticos.
5. ✅ Implementar no crate `trader-core`.
6. Executar backtest com dados históricos reais do banco.
7. Rodar paper trading simulado/replay por período significativo.
8. Avaliar métricas mínimas de aprovação definidas no documento da estratégia.
