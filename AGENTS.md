# AGENTS.md — HumanStyle Trader Bot

**Versão:** 1.0  
**Última atualização:** 2026-07-02  

---

## 1. Propósito

Este arquivo orienta agentes de IA (coding agents) que atuarem no projeto *HumanStyle Trader Bot*. Ele complementa o `README.md` com regras técnicas, arquiteturais e de segurança que devem ser seguidas em todo momento.

---

## 2. Leitura obrigatória antes de qualquer mudança

Antes de alterar código, o agente deve ler e entender:

1. `docs/PRD.md` — visão do produto.
2. `docs/ARCHITECTURE.md` — arquitetura e camadas.
3. `docs/TECHNICAL-ROADMAP.md` — fases e prioridades.
4. `docs/DATA-MODEL.md` — tabelas e relacionamentos.
5. `docs/SECURITY.md` — regras de segurança financeira.
6. `docs/strategy-analysis-framework.md` — processo de estratégias.
7. `docs/decisions/ADR-*.md` — decisões arquiteturais.

---

## 3. Regras de ouro

### 3.1 Segurança financeira

- **Nunca operar sem stop.** Toda ordem de entrada deve ter stop definido.
- **Nunca operar dinheiro real no MVP.** O modo deve ser `paper`.
- **Nunca hardcode credenciais, tokens ou secrets.** Use variáveis de ambiente.
- **Nunca aumentar risco após perda.** Sem "martingale" ou dobra de lote.
- **Nunca abrir nova posição se já existir posição aberta no mesmo ativo.**

### 3.2 Arquitetura

- O core (`trader-core`) **não** conhece IBKR, PostgreSQL, HTTP ou async.
- Toda integração externa vive em `trader-adapters` ou `trader-infra`.
- Estratégias implementam a trait `Strategy` e são plugins.
- Domínio (`trader-domain`) contém entidades, enums, traits e erros.

### 3.3 Qualidade de código

- Usar `Decimal` (`rust_decimal`) para preços, quantidades e valores monetários. **Nunca `f64`** para dinheiro.
- Usar `chrono::DateTime<Utc>` para timestamps.
- Toda regra de estratégia deve vir de configuração (`config.rs`), nunca hardcoded.
- Toda rejeição deve produzir um `RejectionReason` específico.
- Todo sinal deve carregar metadados auditáveis (snapshot de contexto).

### 3.4 Testes

- Toda regra de estratégia deve ter teste unitário com candles sintéticos.
- Backtest e live devem compartilhar a mesma lógica.
- Testes de integração usam banco isolado (`sqlx::test`).
- `cargo clippy` deve passar sem warnings críticos.

### 3.5 Banco de dados

- Usar `NUMERIC` para campos monetários no PostgreSQL.
- Timestamps sempre em UTC (`TIMESTAMPTZ`).
- Candles são imutáveis; correções geram novos registros.
- Cada sinal/trade armazena `strategy_id`, `strategy_version` e `config_hash`.

---

## 4. Workflow de desenvolvimento

### 4.1 Antes de implementar

1. Identifique em qual crate a mudança deve acontecer.
2. Verifique se a mudança viola alguma decisão arquitetural (ADRs).
3. Se a mudança for estrutural ou arquitetural, registre uma nova ADR.
4. Se for nova estratégia, siga `docs/strategy-analysis-framework.md`.

### 4.2 Durante a implementação

1. Mantenha o domínio puro e sem dependências externas.
2. Adicione testes unitários junto com a implementação.
3. Atualize migrações do banco se necessário.
4. Valide formatação com `cargo fmt`.
5. Valide lint com `cargo clippy -- -D warnings`.

### 4.3 Após a implementação

1. Execute `cargo test`.
2. Execute `cargo clippy`.
3. Atualize a documentação relevante (`docs/`, `README.md`, ADRs).
4. Se alterar o modelo de dados, atualize `docs/DATA-MODEL.md`.

---

## 5. Convenções de código

### 5.1 Rust

- Código em inglês (nomes de structs, funções, traits).
- Documentação em português ou inglês, consistente com o arquivo.
- Erros de domínio com `thiserror`.
- Erros de aplicação com `anyhow`.
- Async com `tokio`.
- Logging estruturado com `tracing`.

### 5.2 Estrutura de módulos

```text
crates/trader-core/src/
├── lib.rs
├── context/
├── risk/
├── execution/
├── portfolio/
├── strategies/
│   └── pullback_trend_v1/
│       ├── mod.rs
│       ├── context.rs
│       ├── setup.rs
│       ├── entry.rs
│       └── config.rs
└── indicators/
```

### 5.3 Commits e versionamento

- Commits em português ou inglês, mas consistentes.
- Use mensagens descritivas.
- Não faça git mutations (commit, push, rebase) sem confirmação explícita do usuário.

---

## 6. Anti-padrões proibidos

| Proibido | Por quê |
|----------|---------|
| Hardcode preços, quantidades ou parâmetros de estratégia | Impede testes e versionamento |
| Usar `f64` para dinheiro | Perda de precisão financeira |
| Chamar API do broker diretamente do `trader-core` | Quebra arquitetura ports & adapters |
| Ignorar erros com `unwrap()` em código de produção | Pode causar panics em runtime |
| Logar credenciais ou tokens | Violação de segurança |
| Alterar estratégia em produção sem nova versão | Invalida métricas e auditoria |
| Operar sem stop | Violação de segurança financeira |
| Usar dados futuros em backtest | Lookahead bias |

---

## 7. Como adicionar uma nova estratégia

1. Crie `docs/strategies/<nome>.md` seguindo o framework.
2. Defina regras objetivas.
3. Crie diretório em `crates/trader-core/src/strategies/<nome>/`.
4. Implemente `context.rs`, `setup.rs`, `entry.rs`, `config.rs`.
5. Implemente a trait `Strategy` em `mod.rs`.
6. Escreva testes unitários com candles sintéticos.
7. Adicione configuração em `config/strategies/<nome>.toml`.
8. Registre no registry de estratégias.
9. Atualize documentação.

---

## 8. Contatos e escalação

- Decisões arquiteturais: revisar `docs/decisions/`.
- Dúvidas de negócio: consultar `docs/PRD.md`.
- Dúvidas de estratégia: consultar `docs/strategy-analysis-framework.md`.
- Incidentes de segurança: seguir `docs/SECURITY.md`.

---

## 9. Referências

- `docs/ARCHITECTURE.md`
- `docs/PRD.md`
- `docs/SECURITY.md`
- `docs/strategy-analysis-framework.md`
