# ADR-017 — Limite de risco da conta inteira, não só por instância

**Status:** aceito — vigente, com duas notas de 07/09/2026 (abaixo)
**Data:** 2026-09-04
**Fecha:** C2 da auditoria de 2026-08-30 (o bloqueador de go-live)
**Contexto anterior:** ADR-010 (gate de go-live), ADR-013 (app do umbrelOS)

---

## Contexto

Todo o controle de risco do projeto nasceu por processo. Cada instância do live
carrega o próprio `RiskState`, valida contra ele, e o `rebuild_risk_state` soma
**apenas os trades do próprio símbolo**. Isso fazia sentido quando havia uma
instância.

Hoje são 11 instâncias na **mesma conta IBKR**. Consequências, todas medidas na
auditoria:

- `max_daily_loss_pct = 2%` vira, na prática, até **22%** — cada instância só
  enxerga a própria perda.
- O cap de notional do sizing trava cada posição em ~100% do capital, **por
  processo**. Três posições simultâneas somam ~300% de exposição.
- Não havia limite de posições simultâneas nem nada que parasse a conta como um
  todo.

Agrava: os pares aprovados são quase todos small-caps correlacionados (IWM, IWV,
IWO, IJS, VBR, AVUV, IWN, SLYV). Os sinais chegam em cluster no mesmo dia, então
o pior caso não é hipotético — é o caso típico.

## Decisão

Antes de abrir posição, **toda instância verifica os limites da conta inteira**.
Três travas, todas somando as 11 instâncias:

| Trava | Padrão | Fonte da verdade |
|---|---|---|
| Perda diária agregada | 4% do capital | banco (`trades` de hoje, todos os símbolos) |
| Posições simultâneas | 3 | broker (`get_positions`) |
| Notional agregado | 200% do capital | broker (`get_positions`) |

Configuráveis em `[risk]`: `max_portfolio_daily_loss_pct`,
`max_concurrent_positions`, `max_portfolio_notional_pct`.

> ### Nota 1 de 07/09/2026 — o notional agregado não é o que a tabela diz
>
> A linha "notional agregado" descreve um teto sobre a soma **incluindo a
> posição que está para abrir**. O código não faz isso: `exposure_limit_hit`
> (`crates/trader-cli/src/commands/paper.rs:1502-1529`) soma apenas as posições
> **já abertas** e testa `notional >= teto` — o notional prospectivo fica de
> fora. Como o sizing é `trunc(capital / entrada)`
> (`crates/trader-core/src/risk/mod.rs:253`), duas posições cheias somam
> ≈ 199,9% < 200% e a terceira ainda passa pelo teto. Na prática a trava que
> morde é `positions.len() >= 3`, e o teto de 200% quase nunca é atingido
> (medido no replay `portfolio-regime-1`; ADR-020, fato 4).
>
> Fazer a checagem incluir a posição prospectiva
> (`notional_existente + notional_novo >= teto`) é o que a **ADR-020 propõe** —
> ADR **proposta, não implementada** em 07/09/2026. Até ela ser aprovada e
> aplicada, o que vale é esta nota, não a linha da tabela.

Bloqueio **não** é circuit breaker: a instância não morre, só não abre posição
nova naquele ciclo, registra `portfolio_limit` em `system_events` e segue
gerenciando o que já tem aberto.

## Por que sem tabela nova

A alternativa óbvia era persistir o estado agregado numa tabela e cada instância
escrever nela. Foi descartada: seria um terceiro lugar guardando o mesmo fato,
com todas as chances de divergir das outras duas.

As duas fontes usadas já são autoritativas e ninguém precisa mantê-las:

- **O broker sabe a exposição real da conta.** `get_positions()` devolve todas
  as posições, de todas as instâncias. É o mesmo lugar de onde a reconciliação
  já lê, e não mente sobre o que a conta carrega — inclusive posições que o bot
  não abriu (foi assim que as 827 ações órfãs de IWM apareceram).
- **O banco sabe o P&L realizado do dia.** `list_today_account()` é a consulta
  que já existia por símbolo, sem o filtro.

## Escolha dos padrões

- **4% de perda diária:** o dobro do orçamento de uma instância, não a soma dos
  onze. Um dia ruim em que duas ou três estratégias erram junto cabe; um dia em
  que o portfólio inteiro sangra, não.
- **3 posições simultâneas:** com ativos correlacionados, a quarta posição
  quase não diversifica e multiplica a exposição ao mesmo movimento.
- **200% de notional:** o sizing já limita cada posição a ~100% do capital;
  este teto permite duas posições cheias e barra a terceira por tamanho. Era a
  intenção; **o código não barra** — ver a nota 1 acima.

São padrões calibrados para **conta paper com margem**. Antes de dinheiro real
devem ser reapertados — provavelmente 2% de perda diária e 100% de notional.

> ### Nota 2 de 07/09/2026 — não aperte o notional para 100% antes da ADR-020
>
> Com o sizing de hoje, "100% de notional" recusa o cluster de entradas — e o
> cluster é onde está o edge. No replay dos 232 trades dos 8 pares vivos, o P&L
> vai de US$ 19.380 (com flatten, ADR-018) para **US$ 12.286** sob 1 posição /
> 100% de notional; na balance-area, dias com ≥ 2 entradas têm PF 2,69 contra
> 0,70 nos dias de entrada única (ADR-020, fato 4).
>
> A ADR-020 propõe chegar aos 100% **sem** perder o cluster, via
> `capital_fraction = 1 / max_concurrent_positions`: com 3 posições e fração
> 1/3, três posições cheias cabem em 100%. Proposta **não implementada** em
> 07/09/2026. Enquanto ela não estiver no ar, esta ADR deve ser lida como
> "permitir o cluster", e o aperto acima não é a mudança inócua que a frase
> sugere.

## Consequências

- **O gate B fica um pouco mais lento** nos dias de cluster: o 4º sinal
  simultâneo é recusado. É o comportamento desejado, mas muda a estatística de
  amostragem em relação ao backtest, que roda cada par isolado com capital
  próprio.
- **Backtest e live divergem aqui de propósito.** O backtest não modela a conta
  compartilhada; comparar o número de trades entre os dois passa a exigir esta
  ressalva.
- **Falha fechado:** sem banco, a perda do dia não é mensurável e a entrada é
  bloqueada — mesmo princípio do "live não sobe sem banco".
- A parte pura (posições e notional) tem teste; a parte que depende do banco
  não. É a divisão possível sem subir Postgres no teste unitário.

## O que este ADR NÃO resolve

- **Não há kill-switch comum.** Uma instância bloqueada segue rodando; nada
  encerra a operação inteira de uma vez. Para dinheiro real isso ainda falta.
- **O limite é medido na ENTRADA.** Posições já abertas podem ultrapassar os
  tetos se o mercado se mover contra — não há redução forçada de exposição.
- **Não há limite por setor ou por correlação**, só contagem e notional. Três
  posições em três small-caps que andam juntas contam como três posições
  distintas.


---

## Nota de 08/09/2026 — o teto de 200% quase nunca mordia (ADR-020 §4)

Como esta ADR foi implementada, `exposure_limit_hit` (`paper.rs`) testava se
a soma das posições **já abertas** atingia o teto. Como o sizing é
`trunc(equity / preço)`, duas posições somam ~199,8% — abaixo do teto — e a
terceira entrava inteira, levando a conta a ~300% de notional. Quem segurava
a conta era o limite de **contagem** (`max_concurrent_positions = 3`), não o
teto de notional: o número de 200% era quase decorativo.

Desde 08/09/2026 a checagem soma a posição prospectiva: bloqueia quando
`notional_existente + notional_da_nova > teto`. O valor da nova é o teto que
a instância pode ocupar (`capital × capital_fraction × max_notional_multiple`,
limitado por `max_notional_usd`) — um limite superior, porque o tamanho exato
só existe depois do sinal e esta checagem roda antes dele. Erra para o lado
conservador; com os stops medidos, teto e tamanho real coincidem em quase
todo trade.

A checagem antiga (`existente >= teto`) continua valendo em paralelo: o teto
não pode ficar mais frouxo do que era em nenhum caminho.

Esta é uma mudança de comportamento do LIVE, e só entra em produção quando as
imagens forem reconstruídas. Detalhe e números em
`docs/reports/sizing-adr020-2026-09-08.md` §6.
