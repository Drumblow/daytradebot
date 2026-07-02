# ADR-007: TWS API/IB Gateway para integração com Interactive Brokers

**Status:** Aprovado  
**Data:** 2026-07-02  
**Autor:** CTO / Arquiteto de Software  

---

## Contexto

O projeto escolheu a Interactive Brokers (IBKR) como broker inicial no ADR-003. Para a Fase 2 do roadmap, era necessário decidir qual API da IBKR usar para market data e execução de ordens.

A IBKR oferece duas APIs principais:

1. **Client Portal API** — REST/HTTP, autenticação via navegador, sessão pode expirar.
2. **TWS API / IB Gateway** — socket TCP persistente, conexão contínua, streaming nativo.

## Decisão

Usar a **TWS API via IB Gateway**, integrando através do crate [`ibapi`](https://crates.io/crates/ibapi) v3.x.

## Motivos

- Conexão persistente adequada para automação contínua (paper trading por horas).
- Streaming nativo de barras em tempo real, execuções e status de ordens.
- Suporte direto a ordens complexas (bracket, stop, OCA).
- O crate `ibapi` v3.x é atualizado, bem mantido e usa o protocolo protobuf da IBKR.
- Conta canadense do usuário já está direcionada ao uso de TWS/IB Gateway.

## Alternativa rejeitada

**Client Portal API**: embora seja mais simples de implementar via HTTP, exige autenticação web e a sessão pode expirar, o que a torna menos robusta para operação automatizada contínua.

## Consequências

- Requer instalação e execução do IB Gateway ou TWS.
- Requer configuração de IP confiável (`127.0.0.1`) e porta (`7497` paper, `7496` real).
- O crate `ibapi` v3.x exige IB Gateway/TWS server version ≥ 213.
- Como a conta ainda não está liberada, os testes automatizados usam `SimulatedBroker` e `SimulatedMarketDataProvider`.
- As operações complexas de conta/posições têm stubs controlados até validação manual com conta liberada.

## Decisões relacionadas

- ADR-003: Interactive Brokers como broker inicial.
- ADR-005: Estratégias como plugins via trait (garante portabilidade entre brokers).
