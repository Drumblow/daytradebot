# ADR-016 — Reduzir a `pullback-trend-v1` a um único ativo (IWV)

**Status:** proposto (aguarda a atualização do app para entrar em produção)
**Data:** 2026-09-03
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

Portfólio agregado (soma dos trades dos pares, ordenados no tempo):

| Portfólio | Trades | WR% | PF | P&L | maxDD | P&L/DD | 6m recentes |
|---|---|---|---|---|---|---|---|
| A: atual (11 pares) | 486 | 43,8 | 1,59 | 33.589 | 6.283 | 5,35 | +2.260 |
| B: sem pullback (8) | 232 | 50,0 | 2,05 | 30.914 | 2.887 | 10,71 | +2.158 |
| **C: pullback só IWV (9)** | **306** | **47,4** | **1,98** | **32.644** | **2.797** | **11,67** | **+2.796** |
| D: sem IWO (10) | 407 | 45,5 | 1,75 | 34.689 | 4.120 | 8,42 | +3.146 |

## Decisão

**Manter a `pullback-trend-v1` apenas em IWV. Desligar em IWM e em IWO.**

O portfólio passa de 11 para 9 instâncias.

## Por quê

- **IWO é indefensável:** PF 0,90 em 18 meses, ou seja, perde dinheiro, e ainda
  responde por metade do drawdown da estratégia. Sai sem discussão.
- **IWM tem o pior retorno por unidade de risco do portfólio inteiro** (0,57).
  Contribui +2.045 de P&L carregando 3.598 de drawdown. Uma alocação
  profissional julga contribuição ajustada ao risco, não P&L bruto.
- **A opção C é a melhor em risco-retorno** (11,67 contra 5,35 do atual):
  preserva 97% do P&L com **menos da metade do drawdown**, e é a melhor também
  nos 6 meses recentes. A opção D tem P&L bruto maior, mas comprando 47% mais
  drawdown por 6% mais retorno.
- **Manter IWV preserva opcionalidade.** É o único par da estratégia com
  retorno/risco decente; se o regime de tendência voltar, a estratégia continua
  no ar para mostrar isso, sem custar o drawdown dos outros dois.

## Consequências

- **A amostra do gate B fica mais lenta.** Saem ~2 das 11 instâncias e, pelo
  backtest, cerca de 0,15 trade por pregão. Em compensação, os trades que
  restam vêm dos pares com PF de 1,5 a 2,5.
- **Nenhuma estratégia passa hoje em todos os critérios do ADR-010.** As três
  restantes falham só por amostra; a pullback falha por qualidade. O gate A
  precisa ser reaberto com o walk-forward re-rodado sob as regras corrigidas —
  este ADR não fecha essa questão, só evita continuar alocando risco na
  estratégia que já se sabe negativa.
- Os `client_id` 1 e 3 ficam livres. **Não devem ser reaproveitados** enquanto
  houver histórico dessas instâncias no banco.

## Riscos e o que enfraquece esta decisão

- **Decisão tomada sobre os mesmos dados que selecionaram os pares.** Cortar
  com base neles tem risco de overfitting. Atenuante: IWO é negativo em termos
  absolutos e IWM tem o pior P&L/DD do portfólio — são sinais grosseiros, não
  diferenças marginais.
- **18 meses é uma janela curta** para concluir que uma estratégia perdeu edge.
  Por isso a decisão é reduzir, não arquivar: a `pullback-trend-v1` continua no
  ar em IWV.
- A comparação de portfólio soma P&L de backtests com capital isolado de 100k
  por par; não modela o risco agregado da conta (que é o C2 da auditoria, ainda
  em aberto).

## Como aplicar

Não basta um deploy: as instâncias são serviços do compose do app.

1. `umbrel-daytradebot-store`: remover `iwm-pullback` e `iwo-pullback` da
   variável `INSTANCES` do serviço `scheduler` e marcar os dois serviços com
   `profiles: [desativado]`, para que `docker compose up` não os suba.
2. `botdaytrade`: tirar os dois da lista `INSTANCIAS` do job `deploy` em
   `images.yml`.
3. Atualizar o app no host (a mudança é do compose, não da imagem).
4. Conferir com `gh workflow run host-check.yml` que sobem 9 instâncias.

Fazer isso **fora do pregão**, e não junto de outra mudança.
