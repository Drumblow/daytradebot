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

Novas fontes podem ser adicionadas, desde que passem pelo mesmo processo de análise.

---

## 3. Fases do Processo

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
    mod.rs              → estrutura pública e factory
    context.rs          → regras de contexto de mercado
    setup.rs            → detecção do setup
    entry.rs            → regras de entrada, stop e alvo
    config.rs           → parâmetros da estratégia (Deserialize)
```

Contrato mínimo:

```rust
pub trait Strategy {
    fn name(&self) -> &'static str;
    fn source(&self) -> &'static str; // livro/capítulo
    fn analyze(&self, ctx: &MarketContext, state: &StrategyState) -> Signal;
}
```

**Regras de código:**

* nenhuma regra hardcoded — tudo vem de `config.rs`;
* todo `if` de rejeição deve produzir um `RejectionReason`;
* todo sinal deve carregar metadados auditáveis (valores brutos que originaram a decisão);
* usar `Decimal` para preços, não `f64`;
* usar `chrono::DateTime<Utc>` para timestamps.

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
- aplicar slippage e comissão;
- respeitar as mesmas regras de risco do live.
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
win rate
profit factor
média de R por trade
drawdown máximo
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
[ ] Backtest executado e relatório gerado
[ ] Métricas mínimas atingidas (a definir por estratégia)
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

1. Ler Parte III de *Trading Price Action Trends* (capítulos sobre pullbacks).
2. Preencher o template `docs/strategies/pullback-trend-v1.md`.
3. Refinar as regras objetivas com base nos exemplos do livro.
4. Criar os testes unitários sintéticos.
5. Implementar no crate `trader-core`.
