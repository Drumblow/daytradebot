# ADR-009: Tipo de entrada configurável por estratégia (stop vs limit)

**Status:** Aprovado  
**Data:** 2026-08-04  
**Autor:** CTO

---

## Contexto

A `pullback-trend-v1` (High 2, Al Brooks) documenta a entrada como **buy stop** acima da máxima da barra de sinal, com cancelamento se o rompimento não ocorrer no candle seguinte. A implementação, porém, enviava a entrada como **limit** no preço de gatilho — que, por ficar acima do mercado, executava imediatamente, sem esperar o rompimento.

O projeto vai testar várias estratégias retiradas de livros. Se a camada de execução não honra a regra da fonte, o que está sendo testado não é a estratégia do livro, e nenhuma comparação entre estratégias é válida.

## Decisão

1. O tipo da ordem de entrada passa a ser um **parâmetro da estratégia** (`entry_order_type`: `"stop"` | `"limit"`), propagado pelo domínio (`Signal.entry_order_type` → `Order.entry_order_type`, enum `EntryOrderType`).
2. A `pullback-trend-v1` usa `"stop"` (regra do livro), com validade configurável (`entry_validity_candles`, default 1 — só o candle seguinte).
3. O `SimulatedBroker` implementa entrada stop como ordem pendente: enche no rompimento (high/low intrabar) e expira sem rompimento. Backtest, replay e live usam a mesma semântica.
4. O adapter IBKR monta o bracket com parent **STP** manualmente (o builder do ibapi só suporta entrada limit/market).
5. O live cancela a entrada stop expirada na corretora e libera para o próximo setup.

## Motivos

- **Fidelidade à fonte**: backtest mede a estratégia como ela foi publicada.
- **Framework multi-estratégia**: futuras estratégias podem exigir entrada stop, limit ou market — o tipo vira configuração, não acidente de adapter.
- **Qualidade de sinal**: buy stop filtra rompimentos não confirmados e libera o slot de trade rapidamente quando o setup falha.

## Consequências

- Backtests anteriores à mudança não são comparáveis; re-rodar ao avaliar.
- Resultado com dados reais (SPY, jun–jul/2026): 13 trades com stop vs 8 com limit — entradas expiradas liberam novos setups; PF 6.96 vs 5.42 (amostra pequena, não conclusivo).
- O caminho STP do adapter IBKR precisa de smoke test na conta paper antes da próxima sessão live.
- A tabela `orders` não tem coluna para o tipo de entrada; ele viaja em `orders.metadata.entry_order_type`.
