# ADR-016 — Desligar a `pullback-trend-v1`

**Status:** proposto (aguarda a atualização do app para entrar em produção)
**Data:** 2026-09-03, revisado em 2026-09-04 após a correção do A4
**Contexto anterior:** ADR-005 (estratégias como plugins), ADR-010 (gate de go-live),
ADR-015 (guarda de overshoot)

---

## Contexto

A `pullback-trend-v1` é a estratégia mais antiga do projeto e a que fechou o
gate A em 2026-08-06, em IWM, IWV e IWO. Hoje ela ocupa 3 das 11 instâncias.

Duas correções mudaram o que o backtest mede, e nenhuma delas existia quando o
gate A foi fechado:

- **ADR-015** (guarda de overshoot): entrada cujo gatilho o mercado já
  atravessou é invalidada, e no simulado o gap passa a encher na abertura.
- **A3 da auditoria** (regra única de expiração): o simulado dava **dois**
  candles para o rompimento e o live dava **um**. Unificados em um — o
  comportamento real da produção.

Os números do gate A foram produzidos sob a regra antiga, mais generosa. Este
ADR revisita a decisão com as regras corrigidas.

## Evidência

Backtest de 18 meses (24/02/2025 → 02/09/2026), código atual, 11 pares:

| Estratégia | Ativo | Trades | WR% | PF | avgR | Veredito ADR-010 |
|---|---|---|---|---|---|---|
| pullback-trend-v1 | IWM | 101 | 39,6 | 1,16 | 0,175 | falha WR, PF |
| pullback-trend-v1 | IWV | 74 | 39,2 | 1,44 | 0,127 | falha WR, avgR |
| pullback-trend-v1 | IWO | 79 | 35,4 | **0,90** | 0,021 | falha WR, PF, avgR |
| balance-area-breakout-v1 | IJS/VBR/AVUV | 25–36 | 40–56 | 2,27–2,53 | 0,19–0,60 | só falta amostra |
| opening-reversal-v1 | IWM/IWN | 31–36 | 45–47 | 1,48–1,72 | 0,30–0,41 | só falta amostra |
| range-extreme-fade-v1 | AVUV/SLYV/IWV | 19–29 | 53–71 | 1,60–3,98 | 0,27–0,73 | só falta amostra |

Contribuição isolada de cada par da pullback:

| Ativo | Trades | PF | P&L | maxDD | P&L/DD |
|---|---|---|---|---|---|
| IWM | 101 | 1,16 | +2.045 | 3.598 | **0,57** |
| IWV | 74 | 1,44 | +1.730 | 1.046 | **1,65** |
| IWO | 79 | 0,90 | −1.100 | 3.141 | **−0,35** |

Portfólio agregado (soma dos trades dos pares, ordenados no tempo) — números
**sem custo de execução realista**, ou seja, antes da correção do A4; ficam
aqui porque foi essa tabela que motivou a primeira versão da decisão:

| Portfólio | Trades | WR% | PF | P&L | maxDD | P&L/DD | 6m recentes |
|---|---|---|---|---|---|---|---|
| A: atual (11 pares) | 486 | 43,8 | 1,59 | 33.589 | 6.283 | 5,35 | +2.260 |
| B: sem pullback (8) | 232 | 50,0 | 2,05 | 30.914 | 2.887 | 10,71 | +2.158 |
| **C: pullback só IWV (9)** | **306** | **47,4** | **1,98** | **32.644** | **2.797** | **11,67** | **+2.796** |
| D: sem IWO (10) | 407 | 45,5 | 1,75 | 34.689 | 4.120 | 8,42 | +3.146 |

## Decisão

**Desligar a `pullback-trend-v1` nos três ativos.** O portfólio passa de 11
para 8 instâncias.

> **Revisão de 04/09/2026 — esta decisão mudou durante a própria análise.**
> A versão original deste ADR mantinha a estratégia em IWV, com base num
> backtest cujo custo de execução efetivo era 0,001% (o defeito A4 da
> auditoria). Corrigido o custo e calibrado para 2 bp, IWV cai para PF 1,09
> com avgR negativo — não é edge, é ruído. Ver §Sensibilidade ao custo.

## Sensibilidade ao custo de execução

Com o A4 corrigido, o backtest passou a aceitar `--slippage-bps` e o portfólio
foi medido em vários níveis de custo (18 meses, P&L somado dos 11 pares):

| Slippage | Portfólio | pullback | balance | opening-rev | rangefade |
|---|---|---|---|---|---|
| 0 bp | +33.452 | **+2.174** | +17.796 | +5.502 | +7.979 |
| 1 bp (0,01%) | +25.731 | **−1.936** | +16.221 | +4.450 | +6.996 |
| 2 bp (0,02%) | +18.080 | **−5.983** | +14.648 | +3.400 | +6.015 |
| 5 bp (0,05%) | — | **−17.793** | +10.002 | +281 | — |

Calibração dos 2 bp: os ativos operados são ETFs cotados entre US$ 120 e
US$ 435, onde **um centavo de spread vale de 0,23 a 0,83 bp**. 2 bp cobre cerca
de um spread cheio nos nomes mais caros de negociar (AVUV, SLYV) e é
conservador nos demais. O valor que estava no código, 0,1%, equivaleria a 12–43
centavos por execução — irreal para estes ativos.

Por par, a 2 bp:

| Estratégia | Ativo | Trades | PF | avgR | P&L |
|---|---|---|---|---|---|
| pullback-trend-v1 | IWM | 101 | **0,87** | −0,047 | −2.205 |
| pullback-trend-v1 | IWV | 74 | **1,09** | −0,169 | +538 |
| pullback-trend-v1 | IWO | 79 | **0,68** | −0,191 | −4.316 |
| balance-area-breakout-v1 | IJS / VBR / AVUV | 25–36 | 1,85–1,96 | 0,05–0,61 | +4.4k a +5.2k |
| opening-reversal-v1 | IWM / IWN | 31–36 | 1,18–1,29 | 0,13–0,24 | +1.6k / +1.8k |
| range-extreme-fade-v1 | AVUV / SLYV / IWV | 19–29 | 1,15–3,04 | −0,02–0,55 | +0,2k a +3,5k |

**A `pullback-trend-v1` é a única estratégia que não sobrevive a um centavo de
custo.** Ela fica negativa já a 1 bp, antes de qualquer premissa agressiva.
As outras três seguem positivas até 2 bp, e a `balance-area-breakout-v1`
aguenta 5 bp.

## Por quê

- **A estratégia não paga o próprio custo de execução.** É o argumento que
  substitui todos os outros: com stops de ~0,13% do preço, o edge medido é da
  mesma ordem de grandeza do spread. Não há margem.
- **IWO é indefensável:** PF 0,90 em 18 meses, ou seja, perde dinheiro, e ainda
  responde por metade do drawdown da estratégia. Sai sem discussão.
- **IWM tem o pior retorno por unidade de risco do portfólio inteiro** (0,57).
  Contribui +2.045 de P&L carregando 3.598 de drawdown. Uma alocação
  profissional julga contribuição ajustada ao risco, não P&L bruto.
- **IWV, que na primeira versão deste ADR seria mantido, também não passa:**
  PF 1,09 com avgR negativo a 2 bp. Um PF marginalmente acima de 1 com R médio
  negativo é ruído, não edge — é o tipo de número que só sobrevive porque o
  modelo de custo era otimista.

## Consequências

- **A amostra do gate B fica bem mais lenta.** Saem 3 das 11 instâncias e, com
  elas, mais da metade dos trades do portfólio (254 de 486 em 18 meses). Em
  compensação, os que restam vêm dos pares que pagam o custo de execução.
- **Nenhuma estratégia passa hoje em todos os critérios do ADR-010.** As três
  restantes falham só por amostra; a pullback falha por qualidade. O gate A
  precisa ser reaberto com o walk-forward re-rodado sob as regras corrigidas —
  este ADR não fecha essa questão, só evita continuar alocando risco na
  estratégia que já se sabe negativa.
- Os `client_id` 1, 2 e 3 ficam livres. **Não devem ser reaproveitados** enquanto
  houver histórico dessas instâncias no banco.

## Riscos e o que enfraquece esta decisão

- **Decisão tomada sobre os mesmos dados que selecionaram os pares.** Cortar
  com base neles tem risco de overfitting. Atenuante: IWO é negativo em termos
  absolutos e IWM tem o pior P&L/DD do portfólio — são sinais grosseiros, não
  diferenças marginais.
- **18 meses é uma janela curta** para concluir que uma estratégia perdeu edge.
  Atenuante: o argumento decisivo não é o P&L da janela, e sim que o edge
  medido tem a mesma ordem de grandeza do custo de execução — isso é estrutural
  da estratégia (stops de ~0,13% do preço), não do período.
- **A estratégia é desligada, não arquivada.** O código e o doc continuam no
  repositório. Se a calibração de slippage se mostrar pessimista contra os
  fills reais, ou se o regime de tendência voltar, ela volta com um novo ADR.
- A comparação de portfólio soma P&L de backtests com capital isolado de 100k
  por par; não modela o risco agregado da conta (que é o C2 da auditoria, ainda
  em aberto).

## Como aplicar

Não basta um deploy: as instâncias são serviços do compose do app.

1. `umbrel-daytradebot-store`: remover `iwm-pullback`, `iwv-pullback` e
   `iwo-pullback` da variável `INSTANCES` do serviço `scheduler` e marcar os
   três serviços com `profiles: [desativado]`, para que `docker compose up` não
   os suba.
2. `botdaytrade`: tirar os três da lista `INSTANCIAS` do job `deploy` em
   `images.yml`.
3. Atualizar o app no host (a mudança é do compose, não da imagem).
4. Conferir com `gh workflow run host-check.yml` que sobem 8 instâncias.

Fazer isso **fora do pregão**, e não junto de outra mudança.
