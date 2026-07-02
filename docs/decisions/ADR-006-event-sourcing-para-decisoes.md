# ADR-006: Event sourcing para decisões

**Status:** Aprovado  
**Data:** 2026-07-02  
**Autor:** Software Architect  

---

## Contexto

O robô toma decisões financeiras que precisam ser totalmente auditáveis. É necessário saber, para cada operação: por que entrou, por que não entrou, por que saiu, qual era o contexto e qual regra foi acionada.

## Decisão

Tratar **toda decisão importante como um evento imutável** persistido no banco: sinais, rejeições, ordens, fills e trades.

## Eventos principais

```text
CandleReceived
ContextClassified
SignalGenerated
SignalRejected
RiskValidated
RiskRejected
OrderSubmitted
OrderFilled
OrderCancelled
PositionOpened
PositionClosed
TradeCompleted
DailyLimitHit
```

## Justificativa

- **Auditabilidade total:** qualquer decisão pode ser reconstruída.
- **Reprodutibilidade:** backtest e live compartilham os mesmos eventos.
- **Debug:** facilita identificar divergências entre simulação e real.
- **Análise:** permite análise de motivos de rejeição e comportamento do robô.
- **Conformidade:** atende requisitos de retenção e auditoria financeira.

## Consequências

- Maior volume de dados armazenados.
- Modelo de dados mais complexo.
- Necessidade de padronizar `correlation_id` entre eventos relacionados.

## Decisões relacionadas

- ADR-002: PostgreSQL como datastore.
- `docs/DATA-MODEL.md`.
- `docs/SECURITY.md`.
