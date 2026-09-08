# Runbook — Checklist para operar com dinheiro real

Gate formal (ADR-010, que substituiu o "mínimo 3 meses" de docs/OPERATIONS.md): **gate composto** — validação estatística com histórico + 4 semanas de paper live + aprovação documentada. Este checklist operacionaliza o gate.

## 1. Validação da estratégia (pode ser feita agora, com histórico)

Régua válida: **com o flatten de fim de pregão** (ADR-018, implementado em
07/09/2026 — é o padrão do motor). `--no-flatten` existe só para reproduzir
runs anteriores ao ADR-018 e **não vale como gate A**: aquele número carrega
posição pela noite, coisa que o live nunca faz. Confira as saídas `end_of_day`
na tabela "Saídas por motivo" do relatório — com o flatten **ligado** o
relatório não imprime nada sobre a régua (o aviso `⚠️  Flatten: DESLIGADO` só
sai com `--no-flatten`). Para saber a régua que o run usou, olhe o campo
`session_flatten` no JSON de `--output` do walkforward — que é o horário ET
esperado da última barra do RTH (`[session].last_bar`), não o gatilho — ou rode
o `analyze`, que imprime `(slippage X bp · flatten HH:MM)`. Os defaults de
`--symbol`/`--strategy` são `SPY` / `pullback-trend-v1`, que não é instância
viva — passe os dois sempre.

- [ ] Backtest com dados reais ≥ 6 meses: ≥ 50 trades, win rate ≥ 40%, PF ≥ 1.3, DD ≤ 10%, avg R > 0.15
      (`cargo run -p trader-cli -- ingest --symbol <S> --days 365 --provider ibkr` antes;
      depois `backtest --symbol <S> --strategy <E> --from YYYY-MM-DD`)
- [ ] Walk-forward OOS com métricas sustentadas fora da amostra
      (`cargo run -p trader-cli -- walkforward --symbol <S> --strategy <E> --windows 4`)
- [ ] Resultados registrados em `docs/strategies/<estratégia>.md` (checklist de validação)

Os seis critérios acima são os do ADR-010 e continuam sendo o gate. Sob a linha
`--- proposta ADR-019 §7 (ainda não é o gate vigente) ---` o `walkforward` checa
**dois** dos cinco critérios propostos pelo §7 — PF em R ≥ 1.2 e 2 melhores
meses ≤ 60% do net —, e eles só valem como gate quando o dono aprovar o §7 (o
IC95 do PF em blocos, terceiro critério mecanizável, depende do item 8 do plano,
ainda pendente). Já as linhas `[ i]` logo abaixo — t-stat do avg R,
corr(risco, R), custo e concentração por dia — são relatório obrigatório, não
critério: não aprovam nem reprovam nada, nem depois do §7 aprovado.

Estado em 07/09/2026 (`docs/reports/gate-a-com-flatten-2026-09-07.md`): com a
régua do live, a `balance-area-breakout-v1` reprova em VBR (avg R −0,013) e em
AVUV (WR 31,4%, avg R −0,173) e só passa em IJS; a `range-extreme-fade-v1`
reprova em IWV (avg R 0,043). E **nenhuma das oito combinações chega aos 50
trades OOS** (18 a 35) — o critério de amostra não é marcável por backtest, só
pelo paper forward da seção 2. Esses runs saem de um banco com feed degradado a
partir de 07/08/2026 (§6 do relatório): o **nível** do último bloco está
contaminado, então o veredito vale como "a régua certa nos dados que temos",
não como número final.

## 2. Validação operacional (4 semanas, tempo real)

- [ ] 4 semanas de paper live contínuo, uptime ≥ 99%, sem circuit breaker
- [ ] ≥ 20 trades em paper dentro de ±30% das métricas do backtest
      (`cargo run -p trader-cli -- analyze --symbol <S> --strategy <E>`)
- [ ] O `analyze` acima **achou** o backtest de referência — mesma estratégia,
      mesmo par e mesmo `config_hash`. Se ele imprimiu
      "⚠️  Nenhum run de backtest para (...)", ele saiu sem comparar nada e o
      item de cima não pode ser marcado: rode o walkforward com a config de
      produção e repita
- [ ] Nenhuma mudança de `config_hash` durante as 4 semanas. O lado live é
      filtrado por `strategy_id` + `config_hash`, então mudar parâmetro tira os
      trades anteriores da amostra e reinicia a contagem (§3.8 do plano) — foi
      o que o hotfix da range-fade (a nota "v1.0.1") fez: `49ee6f045b4c35a7` →
      `818b53394244ca62`. O `strategy_id` e a `version` continuaram os mesmos
      (`1.0.0`): o hash acompanha os **valores** dos parâmetros, e o que mudou
      foram `midday_start_time`/`midday_end_time`
- [ ] Zero violações de risco (todo trade com stop; limites diários respeitados)
- [ ] Restart no meio do pregão testado: estado de risco e ordens recuperados do banco
- [ ] Alertas configurados e testados (`[alerts].webhook_url`)
- [ ] Reconciliação semanal bot vs IBKR sem divergências (posições + ordens)
- [ ] Se houver circuit breaker ou divergência: corrigir e reiniciar a contagem das 4 semanas

## 3. Segurança

- [ ] `app.mode` continua "paper" até a data de go-live planejada
- [ ] Revisão do código de guardas: `paper.rs` (bail em modo real/porta real), `risk/mod.rs` (NotInPaperMode)
- [ ] Sem credenciais no repositório; `.env` fora do git
- [ ] Limites de risco revisados em `config/default.toml` (risco 1%, diário 2%, 3 trades/dia)
- [ ] `[session]` conferido em `config/default.toml`: `flatten_start` 15:55:00,
      `flatten_end` 16:10:00, `last_bar` 15:45:00, **horário de Nova York**. É o
      que impede posição atravessar a noite sem stop (as pernas do bracket vão
      com TIF Day); live e motor de backtest leem daqui, por isso paridade
- [ ] Antes de qualquer push em `crates/**` ou `config/**`: o
      `.github/workflows/images.yml` dispara no push para main, publica as
      imagens e, com `APP_DEPLOY=enabled` e **fora** da janela seg–sex
      09h25–16h10 ET, recria as 8 instâncias de produção com a config nova.
      Push de mudança de parâmetro no meio do gate B troca o `config_hash` em
      produção e reinicia as 4 semanas da seção 2. A guarda de janela só vale
      para o push automático: `gh workflow run images.yml -f
      forcar_em_pregao=true` recria as instâncias **dentro** do pregão e derruba
      posição aberta — é escape hatch de emergência, não rotina

## 4. Go-live

- [ ] Tamanho de posição mínimo no primeiro mês (reduzir `risk_per_trade_pct` para 0.25–0.5%)
- [ ] Acompanhamento manual das 5 primeiras sessões
- [ ] ADR registrando a decisão de go-live e os resultados do paper
- [ ] Plano de rollback: voltar `app.mode` para "paper" e cancelar ordens abertas.
      `config/default.toml` é só o valor padrão — o `AppConfig::load()` empilha
      `config/default`, o arquivo apontado por `TRADER_CONFIG` e as variáveis
      `TRADER__…`, então o caminho rápido não passa pelo `images.yml`: basta
      `TRADER__APP__MODE=paper` no ambiente das instâncias no host e reiniciá-las.
      Se ainda assim for pelo workflow e estiver em pregão, é
      `gh workflow run images.yml -f forcar_em_pregao=true`, que derruba posição
      aberta (ver seção 3)
