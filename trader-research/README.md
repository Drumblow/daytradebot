# trader-research — estatística de pesquisa

Implementa o **§5.3** de `docs/cto-plano-lucratividade-2026-09.md` e o **item 1**
da lista canônica de pendências do `ADR-019`. Consome o JSON de
`trader-cli walkforward --output` e produz o relatório com PSR, DSR, IC95 do
profit factor por bootstrap em blocos, Monte Carlo de drawdown e concentração.

**Nada aqui é gate.** O critério "limite inferior do IC95 em blocos ≥ 1,0" é
proposta do ADR-019 §7; o gate vigente continua sendo o do ADR-010. Este
pacote produz o número para o dono decidir olhando o efeito real.

## Por que Python e não Rust

O plano é explícito: pesquisa em Python; **o que virar critério é portado para
`crates/trader-backtest/src/stats.rs`**. Enquanto for relatório, fica aqui.

## Uso

```bash
cd trader-research
uv sync
uv run trader-research ../out/adr018/wf19_*.json --por par --out ../docs/reports/x.md
```

`--por` aceita `par` (padrão), `estrategia` (agrega os símbolos da mesma
estratégia) e `portfolio`. `--trials` define os N do DSR. `--reamostras` o
número de reamostras (padrão 10.000). A seed é fixa (`20260907`): rodar duas
vezes dá o mesmo número.

## As travas que fazem o comando falhar

Falham fechado, de propósito — cada uma corresponde a um erro que já aconteceu
neste projeto:

1. **Réguas diferentes não somam.** A régua tem quatro eixos — `slippage_bps`,
   `session_flatten`, `commission_model` e `limit_fill_haircut_bps` —, e
   qualquer divergência aborta (`ReguaDivergente`). Precedente: ADR-015 §4 e
   ADR-018 invalidaram todo run anterior porque o motor mudou de regra; o §5.6
   fez o mesmo com o custo, e a comissão real é **9,9×** a antiga. O lado Rust
   usa a mesma régua em `latest_for` para escolher o baseline do gate B.
2. **Ablação não entra em relatório de gate.** Run com `experimental: true`
   (feito com `--set`/`--strategy-config`) aborta. Precedente: ADR-019 §3 — "a
   primeira ablação vira baseline em silêncio".
3. **Divergência contra o motor aborta o relatório.** Antes de calcular
   qualquer coisa, `crosscheck.py` recalcula em Python o que o `metrics.rs` já
   gravou — 20 campos escalares mais os três mapas por grupo (`by_exit_reason`,
   `by_direction`, `by_entry_hour_et`) — a partir dos mesmos trades, e exige
   igualdade. Se divergir, o relatório **não é gerado**. Campo ausente conta
   como divergência: um export truncado não pode passar por "conferido".
4. **Run sem calendário de pregões não roda.** Sem `oos_sessions` no JSON não
   dá para saber quais dias tiveram zero, e o esquema pré-registrado (abaixo)
   é inexequível. Rode o `walkforward` de novo com o binário atual.
5. **Arquivo repetido e run duplicado abortam.** Passar o mesmo JSON duas
   vezes (glob + caminho, ou uma cópia com outro nome) dobrava trades, P&L e
   amostra sem nenhum aviso.
6. **`--por portfolio` exige `--capital`.** Somar os capitais iniciais de runs
   de estratégias diferentes produz um drawdown percentual que não descreve
   conta nenhuma.

## Decisões que o número depende (e que estão nos docstrings)

- **O esquema é o PRÉ-REGISTRADO no ADR-019 §8: bootstrap sobre o P&L diário
  de _todos_ os pregões, zeros incluídos.** Não é detalhe: estas estratégias
  operam em 5% a 10% dos pregões, então reamostrar só os dias com trade é
  reamostrar 17–32 pontos onde o pré-registro manda reamostrar ~330 — e um
  bloco médio de 5 vira 16% a 29% da série, faixa em que o bootstrap circular
  degenera em rotação e o "IC95" perde cobertura. O relatório imprime o
  esquema alternativo ao lado, como sensibilidade declarada, porque **os dois
  chegam a discordar do veredito** (a balance em IJS passava por um e reprova
  pelo outro).
- **A unidade reamostrada é o pregão; a estatística é recalculada sobre os
  trades daquele pregão.** Assim o ponto central bate com o `profit_factor` do
  motor e a dependência serial entre pregões sobrevive. Um PF sobre P&L já
  agregado por dia seria outro número.
- **Dia = data de Nova York do fechamento** (`exit_time`), igual ao
  `metrics.rs`. Com a régua atual (flatten às 15h45) nenhum trade dos runs
  chega a divergir entre UTC e ET; a regra existe por paridade com o motor e
  para o dia em que houver trade overnight ou saída em sessão estendida —
  `tests/test_fuso.py` fabrica os dois casos, porque com os dados de hoje a
  conversão é indemonstrável.
- **A direção do erro do DSR é desconhecida.** Sem `n_trials` registrado
  (item 3 das pendências do ADR-019), a variância entre tentativas é
  substituída pela variância do estimador de uma série. Se as tentativas forem
  correlacionadas — variações de parâmetro da mesma regra, que é o caso
  registrado —, o número impresso é **pessimista**; só se cobrissem
  estratégias genuinamente diferentes ele seria otimista. Uma versão anterior
  deste texto afirmava "limite superior"; era suposição apresentada como fato.
- **Os modos de sizing B/B'/C ignoram o teto de notional**, que é o que hoje
  realmente prende (§5.5). São teto, não previsão.
- **Um pool não é uma conta.** O P&L de N pares vem de N backtests que
  dimensionaram cada um sobre o próprio capital; o drawdown percentual do pool
  usa a soma (ou o `--capital` informado). Nenhum dos dois descreve a conta
  compartilhada da produção — isso é o replay do §6.4, que este pacote não faz.

## Testes

```bash
uv run pytest -q
```

`test_crosscheck.py` roda contra os JSONs reais de `out/adr018/`; se não
houver nenhum, os testes que dependem deles são pulados.
