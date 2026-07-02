# ADR-003: Interactive Brokers como broker inicial

**Status:** Aprovado  
**Data:** 2026-07-02  
**Autor:** Software Architect  

---

## Contexto

O usuário reside no Canadá e já possui conta na Interactive Brokers. O MVP requer paper trading para validar estratégias sem risco financeiro.

## Decisão

Usar **Interactive Brokers (IBKR)** como broker e provedor de dados inicial.

## Alternativas consideradas

| Alternativa | Prós | Contras |
|-------------|------|---------|
| Alpaca | API simples, dados gratuitos | Disponibilidade limitada para residentes canadenses |
| TD Ameritrade/Schwab | Popular nos EUA | Acesso complexo para não residentes EUA |
| Questrade | Corretora canadense | API menos madura para trading automatizado |
| Wealthsimple | Simples | API pública limitada, não adequada para robô |
| Binance/crypto | Dados 24/7 | Fora do escopo inicial (ativos de renda variável EUA) |

## Justificativa

- Conta canadense já existente do usuário.
- Paper trading disponível para clientes.
- API sem custo adicional.
- Acesso a mercados globais e múltiplos ativos.
- Suporte a ordens complexas (bracket, OCO, stop).

## Consequências

- API considerada complexa em comparação com alternativas modernas.
- Dados em tempo real podem exigir assinaturas pagas.
- Requer atenção especial para autenticação e sessão.
- Decisões de design devem isolar a dependência de IBKR via adapters.

## Decisões relacionadas

- ADR-005: Estratégias como plugins via trait (garante portabilidade).
- `docs/OPERATIONS.md`.
