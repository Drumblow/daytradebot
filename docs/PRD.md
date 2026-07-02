# PRD — Robô Trader Baseado em Price Action e Estratégias Clássicas

**Versão:** 0.2
**Status:** Em implementação — MVP de paper trading simulado funcional
**Nome provisório:** **HumanStyle Trader Bot**
**Objetivo:** Criar um robô trader que opere de forma mais parecida com um trader humano disciplinado, usando estratégias extraídas de livros de Price Action, análise técnica, gestão de risco e psicologia do trading.

---

## 1. Visão do Produto

O projeto será um sistema de trading automatizado/semi-automatizado que analisa o mercado, identifica contexto, detecta setups baseados em regras objetivas e executa operações simuladas inicialmente em ambiente de **paper trading**.

A ideia não é criar um robô de alta frequência, scalping agressivo ou arbitragem por velocidade. O foco é um robô que opere como um trader humano técnico: espera contexto, evita mercado ruim, respeita gestão de risco e registra tudo para análise posterior.

---

## 2. Problema

Bots simples costumam operar de forma burra:

```text
cruzou média → compra
cruzou de volta → vende
```

Isso ignora contexto, lateralidade, risco/retorno, horário, volatilidade, qualidade do setup e comportamento do mercado.

Por outro lado, traders humanos muitas vezes têm dificuldade com disciplina, emoção, overtrading, revenge trading e falta de consistência.

O produto tenta juntar os dois mundos:

> **A leitura contextual de um trader humano + a disciplina e repetibilidade de um robô.**

---

## 3. Objetivos do Produto

### Objetivos principais

1. Criar um robô capaz de operar em **paper trading** com estratégias objetivas.
2. Basear as decisões em conceitos de:

   * Price Action;
   * tendência;
   * lateralidade;
   * reversão;
   * suporte e resistência;
   * volume;
   * volatilidade;
   * gestão de risco.
3. Usar a Interactive Brokers como primeiro ambiente de teste e futura operação real.
4. Manter a arquitetura preparada para migrar futuramente para outra corretora, se necessário.
5. Registrar todos os sinais, operações, rejeições e resultados em banco próprio.
6. Criar base para backtest, replay, auditoria e evolução da estratégia.

### Objetivos secundários

1. Criar dashboard para acompanhar o robô.
2. Gerar diário automático de trades.
3. Comparar performance entre estratégias.
4. Permitir modo semi-automático, onde o bot sugere e o usuário confirma.
5. Permitir múltiplos provedores de dados no futuro.

---

## 4. Não Objetivos

O sistema **não** deve tentar, no MVP:

* fazer high-frequency trading;
* competir por milissegundos;
* operar arbitragem entre corretoras;
* operar com dinheiro real desde o início;
* usar IA generativa para decidir compra/venda sem regras auditáveis;
* operar muitos ativos ao mesmo tempo;
* usar alavancagem agressiva;
* prometer lucro;
* copiar livros inteiros para “treinar” modelo.

---

## 5. Premissas

1. O primeiro ambiente de teste será **Interactive Brokers Paper Trading** (conta de paper trading associada à conta real canadense).
2. A Interactive Brokers oferece paper trading para clientes, mas a disponibilidade e qualidade dos dados dependem das assinaturas de market data configuradas na conta real. ([Interactive Brokers][5])
3. A API de trading da Interactive Brokers é disponibilizada sem custo adicional para clientes IBKR. ([Interactive Brokers][3])
4. Para muitos ativos na Interactive Brokers, dados em tempo real via API exigem assinatura de market data Level 1/top-of-book; forex e cripto são exceções citadas pela própria IBKR. ([Interactive Brokers][4])
6. A Interactive Brokers será a corretora principal para operação real no Canadá.
7. O banco PostgreSQL será usado como memória estratégica do robô, não como substituto da API de mercado.

---

## 6. Persona Principal

### Usuário Desenvolvedor-Trader

Perfil:

* sabe programar;
* quer estudar trading de forma técnica;
* quer transformar livros e estratégias em regras objetivas;
* não quer depender de velocidade extrema;
* quer validar antes de arriscar dinheiro real;
* quer construir um sistema auditável e evolutivo.

Necessidades:

* testar estratégias sem dinheiro real;
* entender por que o bot entrou ou não entrou;
* guardar histórico;
* comparar resultados;
* migrar de corretora sem reescrever tudo;
* reduzir emoção no processo de trading.

---

## 7. Conceito do Produto

O sistema será dividido em módulos:

```text
Market Data Provider
        ↓
Market Context Analyzer
        ↓
Setup Detector
        ↓
Risk Manager
        ↓
Broker Adapter
        ↓
Execution Engine
        ↓
Trade Journal / Analytics
```

A estratégia principal não deve depender diretamente da Interactive Brokers ou qualquer corretora específica.

A camada de estratégia deve falar com traits Rust genéricas, por exemplo:

```rust
#[async_trait]
trait MarketDataProvider {
    async fn get_candles(&self, request: CandleRequest) -> Result<Vec<Candle>>;
    async fn get_current_price(&self, symbol: &str) -> Result<Quote>;
}

#[async_trait]
trait Broker {
    async fn place_order(&self, order: Order) -> Result<OrderId>;
    async fn cancel_order(&self, id: OrderId) -> Result<()>;
    async fn get_position(&self, symbol: &str) -> Result<Option<Position>>;
    async fn get_account(&self) -> Result<AccountSummary>;
}
```

Implementações concretas:

```text
InteractiveBrokersMarketDataProvider
InteractiveBrokersBroker
```

Toda troca de corretora acontece apenas na camada de adapters, sem tocar no core da estratégia.

Hoje essa interface será implementada com Interactive Brokers Canada. Amanhã poderá ser implementada com outra corretora, desde que respeite as mesmas traits.

---

## 8. Estratégia do MVP

### Mercado inicial

Para o MVP, o ideal é começar com ativos líquidos dos EUA em paper trading:

* SPY;
* QQQ;
* AAPL;
* MSFT.

A escolha inicial recomendada é **SPY ou QQQ**, porque têm alta liquidez e comportamento mais “limpo” para estudar tendência, pullback e rompimento.

### Tempo gráfico inicial

O robô deve operar em tempos gráficos maiores, para evitar dependência de latência:

* 15 minutos;
* 1 hora;
* diário para contexto.

Não será foco inicial operar gráfico de 1 minuto ou scalping.

### Estratégia inicial

O primeiro setup deve ser simples:

> **Pullback em tendência com confirmação de Price Action.**

Exemplo conceitual:

```text
1. Gráfico maior indica tendência.
2. Preço está acima de uma média longa.
3. Preço rompe resistência.
4. Depois retorna para testar a região.
5. Pullback não demonstra força contrária excessiva.
6. Surge candle de rejeição/continuidade.
7. Risco-retorno mínimo é aceitável.
8. Spread e volatilidade estão dentro do limite.
9. Bot entra com stop técnico e alvo definido.
```

---

## 9. Requisitos Funcionais

### RF01 — Conectar com provedor de dados

O sistema deve conseguir buscar:

* candles históricos;
* candles em tempo real;
* preço atual;
* volume;
* horário do candle;
* fonte dos dados.

No MVP: Interactive Brokers.

---

### RF02 — Conectar com ambiente de paper trading

O sistema deve conseguir:

* autenticar na Interactive Brokers;
* consultar saldo paper;
* consultar posições;
* enviar ordens simuladas;
* cancelar ordens;
* receber status de ordens;
* registrar fills simulados.

---

### RF03 — Classificar contexto de mercado

O robô deve classificar o mercado como:

```text
tendência de alta
tendência de baixa
lateralidade
alta volatilidade
baixa volatilidade
mercado sem operar
```

Essa classificação deve ser baseada em regras objetivas, versionáveis e testáveis unitariamente.

Exemplos de regras concretas:

* inclinação de médias (ex.: SMA 50 acima/abaixo de SMA 200);
* preço acima/abaixo de média longa por N candles;
* ATR(14) acima/abaixo de um percentil recente;
* distância entre máximas e mínimas do dia abaixo de um threshold;
* rompimentos confirmados por volume relativo acima da média;
* horário de mercado (pré-market, regular, after-hours).

O resultado da classificação deve ser persistido a cada candle fechado, com os valores brutos que a originaram, para permitir auditoria e backtest.

---

### RF04 — Detectar setups

O sistema deve detectar pelo menos um setup no MVP:

```text
Pullback em tendência
```

No futuro, poderá detectar:

* rompimento com confirmação;
* reversão em suporte/resistência;
* falso rompimento;
* lateralidade com range;
* continuação após consolidação;
* candle de exaustão.

---

### RF05 — Rejeitar setups ruins

O bot deve registrar não apenas entradas, mas também setups rejeitados.

Motivos possíveis:

```text
mercado lateral
risco-retorno ruim
spread alto
volatilidade alta demais
perto de notícia
fora do horário permitido
perda máxima diária atingida
setup incompleto
confirmação fraca
```

Esse ponto é essencial para o robô parecer mais com um humano disciplinado.

---

### RF06 — Gestão de risco

O sistema deve possuir regras rígidas e validadas antes de qualquer ordem:

* risco máximo por operação, em percentual do capital;
* perda máxima diária acumulada;
* número máximo de trades por dia;
* stop obrigatório com preço técnico definido;
* alvo obrigatório ou regra de saída objetiva;
* bloqueio após sequência de perdas consecutivas;
* bloqueio fora do horário permitido;
* spread máximo permitido;
* volatilidade máxima permitida (ex.: ATR percentual).

Exemplo inicial:

```text
Risco por trade: 0.5% a 1% do capital
Perda máxima diária: 2% do capital
Máximo de trades por dia: 3
Risco-retorno mínimo: 1:2
Stop obrigatório: sim
Spread máximo: 0.05%
```

Cálculo do tamanho da posição:

```text
risco_monetario = capital * risco_por_trade
distancia_stop  = |preco_entrada - stop|
quantidade      = floor(risco_monetario / distancia_stop)
```

Exemplo: capital de $10.000, risco de 1%, entrada em $500, stop em $495:

```text
risco_monetario = $100
distancia_stop  = $5
quantidade      = 20 ações
```

O `RiskManager` deve recusar a ordem se qualquer regra for violada e registrar o motivo.

---

### RF07 — Execução de ordens

O sistema deve suportar:

* ordem market;
* ordem limit;
* stop loss;
* take profit;
* cancelamento;
* status parcial;
* rejeição;
* timeout;
* reconciliação com posição real/paper.

No MVP, pode começar simples com ordem de entrada + stop + alvo.

---

### RF08 — Banco de dados

O sistema deve usar PostgreSQL para armazenar:

```text
ativos
candles
indicadores calculados
contexto de mercado
sinais detectados
sinais rejeitados
ordens
trades
logs
configurações de estratégia
resultados de backtest
```

O objetivo do banco é permitir auditoria, análise e melhoria.

---

### RF09 — Diário automático de trades

Cada operação deve gerar um registro parecido com:

```text
Ativo: SPY
Setup: Pullback em tendência
Direção: Compra
Contexto: tendência de alta no 1h
Entrada: 10:45
Stop: abaixo do fundo do pullback
Alvo: 2R
Motivo da entrada: rompimento + pullback + candle de rejeição
Motivo da saída: alvo atingido / stop / saída por tempo
Resultado: +2R / -1R
```

---

### RF10 — Modo paper trading

O MVP deve rodar apenas em paper trading.

O sistema deve exibir claramente:

```text
MODO: PAPER TRADING
NÃO OPERANDO DINHEIRO REAL
```

---

### RF11 — Arquitetura multi-broker

O sistema deve ter uma interface genérica de broker.

Primeiro adaptador:

```text
InteractiveBrokersAdapter
```

O core do bot não deve depender diretamente da API da Interactive Brokers.

---

## 10. Requisitos Não Funcionais

### RNF01 — Auditabilidade

Toda decisão do robô deve ser explicável.

O sistema deve responder:

```text
Por que entrou?
Por que não entrou?
Por que saiu?
Qual regra foi acionada?
Qual era o contexto?
Qual era o risco?
```

---

### RNF02 — Segurança

O sistema deve proteger:

* API keys;
* tokens;
* credenciais;
* configurações sensíveis.

As chaves devem ficar em variáveis de ambiente ou secret manager, nunca hardcoded.

---

### RNF03 — Robustez

O bot deve lidar com:

* queda de internet;
* API fora do ar;
* timeout;
* ordem rejeitada;
* ordem parcialmente executada;
* posição inconsistente;
* dados atrasados;
* candle duplicado;
* reconexão.

---

### RNF04 — Baixa dependência de latência

O robô deve operar em tempos gráficos onde atraso de segundos não destrua a estratégia.

O sistema não deve depender de execução em milissegundos.

---

### RNF05 — Portabilidade

O projeto deve ser desenhado para migrar da Interactive Brokers para outra corretora com o mínimo de reescrita possível.

---

## 11. Stack Técnica Sugerida

### Backend

```text
Rust
```

O backend será desenvolvido em Rust desde o início, com as seguintes justificativas:

* **Performance previsível**: sem pausas de garbage collector, essencial para execução de ordens e processamento de candles em tempo real.
* **Concorrência segura**: o compilador impede data races, permitindo coletar market data, executar regras e enviar ordens em paralelo sem medo de corrupção de estado.
* **Tipagem forte**: `Decimal` para dinheiro, enums para status de ordem, `chrono` para timestamps — tudo verificado em tempo de compilação.
* **Ecossistema maduro para o domínio**: `tokio` (async runtime), `reqwest` (HTTP), `sqlx` (PostgreSQL com queries verificadas em compile time), `serde` (serialização), `tracing` (logs estruturados), `thiserror`/`anyhow` (erros), `rust_decimal` (precisão monetária).
* **Binário único**: fácil de empacotar, versionar e deployar em VPS, Docker ou local.

O backend será dividido em crates internos:

```text
trader-domain        → entidades, enums, traits (Order, Trade, Candle, etc.)
trader-core          → lógica de estratégia, contexto, setup, risco, execução
trader-adapters      → implementações de broker e data provider
trader-infra         → banco de dados, repositories, configuração, logging
trader-backtest      → engine de backtest
trader-cli           → entrypoint principal (binário CLI)
```

> **Nota:** O diário automático e analytics estão atualmente implementados dentro do `trader-cli` e persistidos no banco (tabela `trades` com campo `journal`). Um crate separado `trader-journal` ou `trader-api` poderá ser criado em fases futuras.

Não haverá código JavaScript/Node.js/TypeScript no backend.


### Banco de dados

```text
PostgreSQL
```

Pode começar com:

* PostgreSQL local;
* Docker;
* Oracle/VPS.

### Dashboard

Opções:

```text
Next.js / React / Tailwind / Shadcn
```

O dashboard é um frontend separado, opcional na Fase 6, e consome a API Rust. Pode ser escrito em TypeScript/React sem impactar a decisão do backend.

### Jobs / Worker

```text
Rust service
```

O worker será um binário Rust independente, executado sob `tokio`, responsável pelo ciclo contínuo de coleta, análise e execução. Eventuais tarefas pesadas (backtest, replay, geração de relatórios) podem ser executadas como jobs em segundo plano ou binários auxiliares.

### Hospedagem inicial

```text
Local PC → MVP
Oracle Free Tier
```

---

## 12. Dados e Histórico

### Fonte de dados no MVP

A Interactive Brokers será usada inicialmente.

Dados em tempo real via API na Interactive Brokers frequentemente exigem assinaturas de market data. Sem assinatura adequada, o bot pode operar com dados atrasados ou limitados. ([Interactive Brokers][4])

Isso é aceitável para:

* MVP;
* paper trading;
* aprendizado;
* bot lento (timeframes de 15min ou mais);
* testes de arquitetura.

Não é ideal para:

* scalping;
* estratégia sensível a preço exato;
* validação profissional intraday sem dados consolidados;
* execução com dinheiro real sem assinatura de market data.

---

## 13. Backtest

O backtest deve ser implementado após o primeiro fluxo de dados estar funcionando.

Requisitos da engine de backtest:

* executar sobre candles históricos armazenados no PostgreSQL;
* simular execução no fechamento do candle de sinal (modo conservador);
* opcionalmente simular execução em intra-candle para cenários avançados;
* aplicar slippage configurável por ativo;
* aplicar comissões da corretora;
* respeitar as mesmas regras de risco do modo live;
* evitar lookahead bias: o backtest só pode usar dados disponíveis até o momento do candle;
* gerar logs de decisão para comparação com live trading.

Inputs:

```text
ativo
período
timeframe
estratégia
configuração de risco
slippage
comissão
```

Métricas mínimas:

```text
número de trades
taxa de acerto
lucro/prejuízo total
drawdown máximo
profit factor
média de R por trade
sequência máxima de perdas
melhor trade
pior trade
tempo médio na operação
expectativa matemática (edge)
sharpe simplificado
```

A implementação será em Rust, reutilizando o mesmo `RiskManager`, `SetupDetector` e `MarketContextAnalyzer` do modo live, garantindo consistência entre backtest e execução real.

---

## 14. Paper Trading

O paper trading será a principal fase de validação.

A Interactive Brokers oferece paper trading para clientes, permitindo testar estratégias com ordens simuladas. A qualidade dos fills e dados depende das configurações da conta e das assinaturas de market data. ([Interactive Brokers][5])

Objetivos do paper trading:

* testar execução;
* testar reconexão;
* testar lógica de ordens;
* testar diário;
* verificar se o robô respeita risco;
* comparar resultado real-time vs backtest;
* identificar bugs operacionais.

---

## 15. Migração Futura para Interactive Brokers

A Interactive Brokers será avaliada para operação real no Canadá.

Motivos:

* suporte a residentes canadenses;
* corretora mais global;
* mercados mais amplos;
* API disponível para clientes sem custo. ([Interactive Brokers][3])

Pontos de atenção:

* API mais complexa;
* dados em tempo real podem exigir assinaturas;
* permissões de trading por ativo;
* estrutura de ordens diferente de outras corretoras;
* paper trading depende das configurações e assinaturas da conta real. ([Interactive Brokers][5])

Graças à trait `Broker` e à trait `MarketDataProvider`, a migração para IBKR consistirá em criar novos adapters (`IbkrBroker`, `IbkrMarketDataProvider`) sem reescrever a estratégia. A trait define os contratos mínimos necessários para execução.

---

## 16. Roadmap

### Fase 0 — Planejamento

Entregáveis:

* definição do primeiro ativo (ex.: SPY);
* definição do primeiro setup com regras objetivas (ex.: pullback em tendência de alta);
* definição dos timeframes (15min operacional, 1h contexto, diário macro);
* desenho das tabelas principais no PostgreSQL;
* escolha da stack: **Rust + PostgreSQL + Docker + (futuro) React dashboard**;
* criação da conta na Interactive Brokers Canada e ativação do paper trading;
* definição da estrutura de crates Rust e traits principais.

---

### Fase 1 — MVP de conexão

Entregáveis:

* criar crate `trader-adapters` com `InteractiveBrokersMarketDataProvider`;
* criar crate `trader-infra` com conexão PostgreSQL via `sqlx`;
* conectar com Interactive Brokers (API nativa via TWS/Gateway ou REST/Web API, conforme escolha);
* buscar candles históricos;
* receber dados em tempo real (trades/quotes ou atualizações de bar);
* salvar candles no banco com deduplicação por `(symbol, timeframe, timestamp)`;
* configurar `tracing` para logs estruturados.

Critério de sucesso:

```text
Sistema consegue coletar dados de um ativo e armazenar corretamente sem candles duplicados.
```

---

### Fase 2 — Motor de contexto

Entregáveis:

* implementar `MarketContextAnalyzer` no crate `trader-core`;
* classificar tendência com regras objetivas (médias, máximas/mínimas);
* classificar lateralidade (range, Bollinger Bands estreitas, ATR baixo);
* calcular volatilidade (ATR, ATR percentual);
* calcular volume relativo;
* salvar contexto no banco a cada candle fechado.

Critério de sucesso:

```text
Para cada candle fechado, o sistema registra o estado do mercado e os valores brutos usados na decisão.
```

---

### Fase 3 — Primeiro setup

Entregáveis:

* implementar `SetupDetector` no crate `trader-core`;
* detectar pullback em tendência com regras objetivas (ex.: preço acima da média, pullback sem rompimento de estrutura, candle de confirmação);
* integrar com `RiskManager` para validar risco-retorno;
* rejeitar setups ruins com motivo registrado;
* registrar sinais aceitos e rejeitados no banco.

Critério de sucesso:

```text
O bot detecta oportunidades, avalia risco-retorno e registra por que entrou ou por que rejeitou.
```

---

### Fase 4 — Paper trading

Entregáveis:

* implementar `InteractiveBrokersBroker` no crate `trader-adapters`;
* enviar ordem simulada (market ou limit);
* criar stop loss e take profit (bracket order ou OCO);
* acompanhar posição e ordens pendentes;
* registrar fills, resultados e estatísticas no banco;
* gerar diário automático por trade e por dia.

Critério de sucesso:

```text
O bot realiza operações simuladas completas sem intervenção manual, respeitando stop, alvo e gestão de risco.
```

---

### Fase 5 — Backtest

Entregáveis:

* rodar estratégia em dados históricos;
* gerar estatísticas;
* comparar períodos;
* exportar relatório.

Critério de sucesso:

```text
O usuário consegue avaliar se a estratégia tem vantagem estatística.
```

---

### Fase 6 — Dashboard

Entregáveis:

* painel de status do bot;
* trades recentes;
* sinais rejeitados;
* gráfico de equity curve;
* métricas principais;
* logs.

Critério de sucesso:

```text
O usuário entende o que o bot está fazendo sem abrir terminal.
```

---

### Fase 7 — Suporte a múltiplas corretoras (opcional)

Entregáveis:

* validar interface genérica de broker com uma segunda corretora de teste;
* garantir que `InteractiveBrokersAdapter` esteja isolado;
* documentar o processo de adicionar novos adapters.

Critério de sucesso:

```text
A estratégia roda sem depender diretamente da Interactive Brokers.
```

---

## 17. Métricas de Sucesso

### Técnicas

```text
uptime do bot
número de falhas de API
ordens rejeitadas
latência média de execução
candles perdidos
erros de reconexão
```

### Estratégicas

```text
win rate
profit factor
drawdown máximo
média de R por trade
risco-retorno médio
número de trades por semana
performance por tipo de mercado
```

### Comportamentais

```text
quantidade de sinais rejeitados
motivos de rejeição
operações fora do plano: deve ser zero
violação de risco: deve ser zero
overtrading: deve ser zero
```

---

## 18. Riscos

### Risco 1 — Dados gratuitos limitados

Dados limitados ou atrasados na Interactive Brokers (sem assinatura de market data adequada) podem distorcer backtests e sinais intraday.

Mitigação:

```text
Usar estratégia lenta no MVP (15min ou mais).
Avaliar assinatura de market data consolidado antes de operar real.
Não validar estratégia profissional apenas com dados limitados.
```

---

### Risco 2 — Paper trading diferente do real

Ordens no paper não sofrem exatamente os mesmos efeitos de mercado real.

Mitigação:

```text
Rodar por longo período em paper.
Simular slippage e custos.
Operar real apenas com capital mínimo no futuro.
```

---

### Risco 3 — Estratégia subjetiva demais

Conceitos dos livros podem ser difíceis de transformar em regra.

Mitigação:

```text
Começar com apenas um setup.
Definir regras objetivas.
Registrar motivos de entrada e rejeição.
Evitar IA decidindo sem regra clara.
```

---

### Risco 4 — Ficar preso na Interactive Brokers

Se o código for escrito diretamente contra a API da Interactive Brokers, migrar para outra corretora será difícil.

Mitigação:

```text
Criar BrokerAdapter desde o início.
Separar estratégia de execução.
Separar data provider de broker.
Manter traits genéricas Rust como contrato entre core e adapters.
```

---

### Risco 5 — Overengineering

O projeto pode ficar grande demais antes de validar a ideia.

Mitigação:

```text
MVP com um ativo, um setup e paper trading.
Dashboard só depois da lógica principal.
IA só depois da base estatística.
```

---

## 19. Regras de Segurança Financeira

O sistema deve sempre respeitar:

```text
Nunca operar sem stop.
Nunca operar fora do horário configurado.
Nunca operar após perda máxima diária.
Nunca dobrar lote após perda.
Nunca abrir nova posição se já existir posição ativa no mesmo ativo.
Nunca operar dinheiro real no MVP.
```

---

## 20. Uso de IA no Projeto

A IA não deve ser responsável direta por comprar ou vender no MVP.

Uso recomendado de IA:

* transformar conceitos dos livros em regras;
* analisar diário de trades;
* gerar relatórios;
* classificar screenshots ou padrões no futuro;
* sugerir melhorias;
* explicar operações;
* revisar setups rejeitados.

Uso não recomendado no MVP:

```text
"IA decide se compra ou vende com base no gráfico"
```

Motivo:

```text
Para trading, a decisão precisa ser repetível, auditável e testável.
```

---



## 22. Definição do MVP Final

O MVP está funcional quando o sistema consegue:

```text
1. Conectar na Interactive Brokers (codificado; validação real depende de conta liberada).
2. Receber candles de SPY ou QQQ (via simulado ou IBKR).
3. Salvar dados no PostgreSQL.
4. Classificar contexto de mercado.
5. Detectar pullback em tendência.
6. Rejeitar setups ruins com motivo registrado.
7. Enviar ordem em paper trading simulado.
8. Gerenciar stop e alvo.
9. Registrar resultado do trade.
10. Gerar diário automático.
11. Exibir relatório básico de performance (backtest no terminal).
```

> **Status atual (2026-07-02):** Itens 3–11 estão implementados e testados no modo simulado/replay. Itens 1 e 2 com IBKR real aguardam validação com conta liberada.

---

## 23. Resumo Executivo

Este projeto será um **robô trader contextual**, não um bot de velocidade.

A primeira versão usará **Interactive Brokers Paper Trading** (conta canadense), pois é a corretora disponível para o usuário e a mais adequada para operação real no Canadá. O robô deve ser arquitetado desde o início para não depender diretamente da Interactive Brokers, permitindo futura migração para outra corretora caso necessário.

A principal vantagem competitiva do projeto não será “ser mais rápido”, mas sim:

```text
operar pouco,
operar com contexto,
respeitar risco,
registrar tudo,
aprender com os dados,
e evitar os erros emocionais do humano.
```

O produto ideal é um sistema que se comporta como um trader humano disciplinado, mas com memória, consistência e auditoria de máquina.

[1]: https://www.interactivebrokers.com/campus/glossary-terms/paper-trading-account/?utm_source=chatgpt.com "Paper Trading Account | IBKR Glossary"
[2]: https://www.interactivebrokers.com/campus/ibkr-api-page/market-data-subscriptions/?utm_source=chatgpt.com "Market Data Subscriptions | IBKR API | IBKR Campus"
[3]: https://www.interactivebrokers.com/campus/ibkr-api-page/web-api-trading/?utm_source=chatgpt.com "Trading Web API"
[4]: https://www.interactivebrokers.com/campus/ibkr-api-page/market-data-subscriptions/?utm_source=chatgpt.com "Market Data Subscriptions | IBKR API | IBKR Campus"
[5]: https://www.interactivebrokers.com/campus/glossary-terms/paper-trading-account/?utm_source=chatgpt.com "Paper Trading Account | IBKR Glossary"
