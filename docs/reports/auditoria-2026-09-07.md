# Auditoria da pesquisa de lucratividade — verificação dos 10 documentos e as análises que faltavam (07/09/2026)

**Por que existe:** depois de entregue a pesquisa de 06–07/09/2026, o dono pediu para "verificar tudo o que foi feito nessa sessão e fazer as análises que ficaram faltando". A passagem de consistência cruzada e parte da rodada extra não tinham sido executadas por limite de uso.

**Método:** 13 agentes em 4 fases (07/09/2026, ~1h27 de relógio, 927 chamadas de ferramenta). Seis verificadores independentes (motor Rust, números empíricos, consistência cruzada, conformidade com AGENTS.md/framework/ADR-010, recálculo aritmético, referências), quatro analistas fechando as análises pendentes (setups novos, alavanca do contexto Neutral, síntese do screener, crítico de completude) e dois adversários instruídos a derrubar achados e análises, com reexecução própria dos artefatos decisivos. Nada foi implementado, nada foi commitado, nada foi escrito no banco.

**Escopo auditado:** `docs/cto-plano-lucratividade-2026-09.md`, `docs/reports/pesquisa-lucratividade-2026-09-07.md`, `docs/reports/roadmap-decisao-2026-09-07.md`, `docs/reports/estudo-politicas-de-saida-2026-09-07.md`, as três specs v2 (`range-extreme-fade-v2`, `balance-area-breakout-v2`, `opening-reversal-v2`), as ADRs propostas 018/019/020, as alterações em `README.md`, `docs/HANDOFF.md` e `docs/strategy-analysis-framework.md`, e `sql/screens/` + `sql/stats/`.

**Estado das correções:** as 52 correções cirúrgicas que este relatório justifica **já foram aplicadas** nos documentos originais em 07/09/2026 (nenhuma commitada). O que exige reescrita maior está na §7 como pendência aberta, e **não** foi aplicado.

---

## 1. Veredito

Os documentos servem para decidir **depois** das correções abaixo, não antes. O diagnóstico central sobrevive inteiro e foi reproduzido de forma independente por três auditores: o backtest não faz flatten e o live faz (`engine.rs:136-240` vs `paper.rs:2189-2198`), a balance-area reprova o gate A pelo avg R sob a régua do live, o veto de meio-dia é um bug de fuso real, o sizing trava no cap de notional e o feed do Gateway está degradado. O baseline de dinheiro do roadmap (41.410 / 27.175 ao ano / DD 3,8% / risco mediano US$ 576) foi refeito do zero e bateu ao dólar.

O que muda depois da auditoria: (a) **o item de maior valor esperado do plano (#10, liberar contexto `Neutral`) está apoiado num número dobrado** — são 24/18/31 rejeições, não 48/36/62 — e o delta, que o plano declara desconhecido, foi medido por dois instrumentos independentes e é **negativo**; (b) **a corrente de dinheiro do §2.5 e o cenário C do roadmap misturam réguas e rótulos** (12.286 é sem flatten; "1 posição de cada vez" na verdade permite 2), o que muda o baseline de decisão em 29% e 41%; (c) **"stop estreito perde" não é regra transversal** — o sinal inverte em duas das três estratégias, e o piso de 20 bp do item #13 apagaria 33 dos 69 trades da range-fade, que somam +5,7R positivos; (d) **o banco dev que gera todos os backtests está contaminado** pelo mesmo feed que o plano manda diagnosticar só em produção; (e) as três ADRs propostas têm defeitos que fazem a implementação falhar em silêncio (catch-all `_ => Target`, índice único com `label` NULL, cap de liquidez sem caminho de dados no `RiskManager`); (f) o cronograma não cabe no orçamento de horas declarado no próprio roadmap.

---

## 2. O que foi verificado e como

Seis verificadores checaram 174 afirmações contra as fontes primárias (código-fonte em `crates/`, banco dev por `docker exec … psql`, JSONs de backtest e scripts do scratchpad, páginas web salvas, ADRs e relatórios anteriores). Quatro analistas produziram as medições que faltavam. Dois adversários revisaram 128 achados dos verificadores e 32 afirmações das análises novas, com reexecução própria dos artefatos decisivos.

| Dimensão | Itens | Confirmados | Refutados | Imprecisos | Não verificáveis |
|---|---|---|---|---|---|
| Motor (Rust): afirmações A–H sobre backtest, live e adapter IBKR | 23 | 14 | 3 | 6 | 0 |
| Números empíricos (SQL no banco dev + JSONs de backtest) | 35 | 20 | 6 | 7 | 2 |
| Consistência cruzada entre os 10 documentos | 22 | 0 | 11 | 10 | 1 |
| Conformidade das 3 specs v2 e das 3 ADRs com AGENTS.md / framework / ADR-010 | 39 | 0 | 12 | 26 | 1 |
| Recálculo aritmético (baseline, IC95, EV, esforço, limiares) | 29 | 9 | 8 | 12 | 0 |
| Referências (arquivo:linha, caminhos, SQL, fontes web, livros) | 26 | 6 | 4 | 14 | 2 |
| **Total** | **174** | **49** | **44** | **75** | **6** |

Revisão adversarial, aplicada por cima: dos 128 achados revisados, o adversário declara **105 sobrevivem, 15 enfraquecidos e 8 derrubados** (seis identificáveis nominalmente: roadmap:68 "64–82%"; "+12.286 não verificável"; fade-v2:12 "nada na v1 muda"; fade-v2:227 "precedente A2 não é análogo"; ADR-018:154-156 "analyze.rs:46-50"; roadmap:137-140 "4–5,5 meses"). Nas quatro análises novas, o segundo adversário reexecutou os artefatos e classificou **17 sobrevivem, 8 enfraquecidas e 7 derrubadas**. Nenhum achado derrubado entra como erro neste relatório; os enfraquecidos entram com a ressalva.

Limite de método declarado desde já: **nenhum verificador rodou `cargo build/test/run`** (proibido pela tarefa) e **nada foi escrito no repositório nem no banco**. Toda conclusão de código é leitura de fonte; toda conclusão numérica vem de SQL de leitura, dos JSONs do binário de 06/09 ou de re-simulação em Python no scratchpad.

---

## 3. Erros que mudam decisão (severidade alta)

### Bloco A — A alavanca #10 (contexto `Neutral`): número dobrado e delta negativo

**Onde:** `docs/cto-plano-lucratividade-2026-09.md:14` e `:89`; `docs/reports/roadmap-decisao-2026-09-07.md:157`; `docs/strategies/range-extreme-fade-v2.md:36-42`; `docs/HANDOFF.md:228`; `docs/reports/pesquisa-lucratividade-2026-09-07.md:278-279`, `:375`.

**O que dizia:** "`NoContext` rejeitou **48 sinais em AVUV (31 entradas), 36 em SLYV (30), 62 em IWV (35)**"; "146 sinais rejeitados vs 96 entradas: **até 2× trades**"; "~60% dos sinais que a estratégia aprova morrem no `is_tradeable`"; "o PF desses sinais bloqueados é desconhecido — é o teste de maior valor esperado deste plano"; EV do item #10 = **+2,1k/ano** (teto +5–7k).

**O que é verdade:** os números são exatamente o dobro. A rejeição por risco é logada **duas vezes no mesmo run** — `crates/trader-core/src/execution/mod.rs:154` e `crates/trader-backtest/src/engine.rs:228`, ambos `debug!(?reason, …)` — e o `RUST_LOG` da medição original ligou os dois alvos. Duas rotas independentes fecham em 24/18/31 (73 no total): (i) os logs INFO preservados dão 55/48/66 sinais detectados menos 31/30/35 ordens enviadas, com zero rejeições do broker; (ii) uma réplica em Python do `is_tradeable` sobre as barras de sinal devolve 55/48/66 sinais com 24/18/31 bloqueados. A igualdade fecha nos três pares, o que exclui qualquer outra razão de rejeição. Consequências: a razão é 0,76 (não 1,52); a fração que morre no `is_tradeable` é **43,2%** (73/169), não ~60% — e o "~60%" da spec v2 é literalmente 146/(146+96); o teto de trades adicionais é **~34/ano** nos três pares, e **nem no teto** qualquer par chega aos 50 trades OOS do gate A (48/37/47).

Além disso, o PF do delta **não era desconhecido**: sai dos artefatos existentes. Re-simulando os 73 sinais bloqueados com a mecânica do simulador (replay validado contra o backtest oficial — AVUV reproduz n=29 / WR 58,6% / PF 1,64 / avg R +0,323 dígito a dígito): **54 trades, WR 33,3%, PF$ 0,70, PF_R 0,62, avg R −0,321, −US$ 3.257**; com flatten da ADR-018, **PF 0,56 e −US$ 3.985**. Negativo em 2025 (PF 0,52) e em 2026 (0,88), no long (0,52) e no short (0,95). O conjunto cai de PF$ 1,90 para 1,16 (IC90 [0,80; 1,66]). Um segundo instrumento independente — o screener SQL do próprio repo, `sql/screens/09-controle-pos-range-fade.sql`, subtraindo as tags `base` e `ctx` — dá o mesmo sinal: 42 fills nos pares vivos, **−9,1R**. **Ressalva do adversário:** o corte por par não aparece no resumo e desmente "abandonar a alavanca em todo lugar" — em AVUV o incremento é **positivo** (PF$ 1,33, PF_R 1,31, avg R +0,206, n=20; com flatten vira 0,60); o agregado é carregado por IWV (PF 0,28, −US$ 3.083). Segunda ressalva: a base (AVUV/SLYV/IWV) é o conjunto que o gate A selecionou, então a permutação p=0,0066 mistura efeito do filtro com viés de seleção; o que não depende disso é o nível absoluto (PF_R 0,62, P(PF_R<1,0) = 91,4%).

**Correção:** trocar 48/36/62 por 24/18/31 nas seis ocorrências; trocar "~60%" por 43%; trocar "até 2× trades" por "até +76%"; substituir "o PF é desconhecido" pelo número medido, com a ressalva de AVUV; e reescrever a linha #10 do roadmap — o EV pela própria conversão do documento (34,1 trades/ano × avg R × US$ 130) passa de +2,1k para ≈ **−1,4k/ano**. Patch aplicado nos documentos; a decisão de rodar ou não o A/B no motor vira pendência.

**Erro colateral no mesmo achado:** `plano:89` afirma que o detector de dia de range "coincide com Neutral por construção". A própria spec da v2 (`range-extreme-fade-v2.md:44`) diz que essa frase é **falsa** e traz a medição (44,4% das barras de dia de range são Neutral contra 41,9% nas de tendência; "EMA20 plana" cobre 80–89% de todas as barras). Dois documentos do mesmo commit se contradizem no fato que sustenta a hipótese.

### Bloco B — Baseline de dinheiro: troca de régua e rótulo errado de trava

**B1. A corrente do Nível 1 muda de régua no meio** (`plano:107-109` e `:344`; repetido em `ADR-020:74` e `:194`). Dizia: "24.063 isolados → 19.380 com flatten → **12.286** sob 1 posição/100%". Medido: os 12.286 (com 49 de 232 trades recusados) são a trava aplicada ao conjunto **sem** flatten; aplicada ao conjunto **com** flatten, a mesma trava dá **9.513** — também com 49 recusados. A seta infla o resultado final em **29,2%**. (O adversário derrubou a leitura de que o 12.286 seria "não verificável": ele reproduz exato em `_aud5_base.py`.)

**B2. O cenário C do roadmap está rotulado errado** (`roadmap:35`, `:65-66`, `:84-88`; ecoado em `plano:116`). Dizia: "regra de dinheiro real do ADR-017 (100% de notional, 2% de perda diária = **1 posição de cada vez**): 22,7k / 6,3k / 12,3k por ano". O script que produziu a tabela (`replay.py`, `sum(o[2] for o in open_) >= notional_cap_pct*equity`) testa **só as posições já abertas**; com qty = trunc(238000/preço) uma posição vale ~99,9% e a segunda entra sempre — o cenário C permite **2 posições**. Com a trava como o ADR-017 a descreve (teto incluindo a posição nova): 48 trades recusados, **13.484/ano** (contra 22.731, −41%), **2.439/ano** ex jun–out (contra 6.268, −61%), **10.374/ano** só em 2026. Agrava-se que o plano §5.5 **já diagnostica** esse comportamento ("duas posições somam 199,9% < 200% e a terceira entra") — o rótulo contradiz o diagnóstico do próprio documento.

**Correção:** publicar as duas linhas com rótulos distintos (C = régua do live; C' = ADR-017 como escrito) nos dois documentos e marcar a linha do Nível 1 como sendo da régua com flatten (9.513).

### Bloco C — Custo e stop: a varredura publicada está errada e a regra transversal não se sustenta

**C1. Varredura de custo** (`plano:56`). Dizia: "PF de 0 → 2 bp: balance 2,39 → 1,70; opening-reversal 1,58 → 1,12; range-fade 2,38 → 2,05; pullback 1,10 → 0,71". Medido no banco (runs 337–412 de 04/09, lotes identificados pela tabela do ADR-016): **balance 2,29 → 1,92; openrev 1,41 → 1,23; fade 2,39 → 1,88; pullback 1,07 → 0,84**; a 5 bp: 1,52 / 1,02 / 1,46 / 0,60. Três fontes independentes convergem (soma dos JSONs, SQL por lote, e o próprio §2.3 do plano, que usa 1,92 / 1,23 / 1,88). Ou seja, **o §2.2 e o §2.3 do mesmo documento publicam PFs diferentes para o mesmo custo**.

**C2. "Stop estreito perde" como regra transversal** (`plano:56-66`, `:365` item #13, `:328` critério de triagem §10.6). A tabela de tercis é de **uma** estratégia (92 trades OOS da balance, tercis definidos depois de ver os dados) e reproduz: PF 0,50/2,02/2,61 (cortes 17,5 e 28,5 bp). Nas outras duas o sinal **inverte ou desaparece**: range-fade (69 t, cortes 17,6/24,4 bp) PF **2,50** / 1,33 / 2,09 — o tercil mais estreito é o melhor; opening-reversal (67 t, cortes 26,4/37,7 bp) PF 1,69 / 1,32 / **1,02** — decresce com stop largo. E a medição que o próprio §6.9 manda fazer "antes de decidir": um piso de 20 bp remove 38 dos 96 trades da balance (−13,2R, ajuda), **33 dos 69 trades da range-fade (+5,7R, apaga trades lucrativos)** e 5 dos 67 da openrev (neutro).

**C3. O custo de ida e volta não é 4 bp no backtest** (`plano:56`). O alvo é ordem limite e não escorrega (`simulated/broker.rs`, doc de `apply_slippage`), e 116 de 232 trades (50%) saem no alvo — o custo médio efetivamente modelado é **3,0 bp**, isto é 14% do R na balance e 13% na fade, não 18–19%. Na direção oposta, os 4 bp não incluem a comissão real (~0,9 bp), que levaria a 22% / 23% / 16% / 32%.

**Correção:** corrigir a varredura; rebaixar o item #13 de regra global do `RiskManager` para parâmetro da balance-area-v2, publicando os tercis das três estratégias; declarar as duas colunas de custo (modelado e real).

### Bloco D — Liquidez e sizing: a tabela de elegibilidade subdimensiona os ativos líquidos por 2–4×

**Onde:** `plano:274` (§5.5) e `ADR-020:127-131`. Dizia: "notional ≤ 1/3 da barra mediana de 15m dos últimos 60 pregões: SLYV ≈ US$ 50–90k, IJS ≈ 85–120k, VBR ≈ 180k, AVUV/IWN/IWO 250–300k". Medido (mediana de `close*volume`, 60 pregões anteriores a 07/08/2026, dividida por 3): **SLYV 104k, IJS 169k, VBR 424k, IWN 713k, IWO 923k, AVUV 1,22M**. Com a janela contaminada pelo feed esparso (60 pregões até 02/09) só SLYV (87k) e IJS (122k) se aproximam do publicado; VBR, IWN, IWO e AVUV ficam 2 a 4× acima. Pior: **1/3 das medianas que os próprios documentos publicam** já daria SLYV 97–122k e VBR 333–447k — a tabela contradiz a regra que a define, e a ADR-020:296 chega a calcular "mediana 290.000 com fração 1/3 → teto ≈ 96.667", contradizendo a própria tabela.

**Erro estrutural que impede a implementação:** a ADR-020 manda `median_bar_notional(candles, n)` ser "consumida pelo `RiskManager`", mas `RiskManager::validate(&self, signal, ctx, quote, state, capital)` (`risk/mod.rs:91-98`) **não recebe série**, e o `MarketContext` não carrega notional mediano. É a única peça nova da ADR que não é "só o campo e o plumbing", e não há caminho de dados escrito. Some-se `max_notional_usd` declarado `f64` no `RiskSettings` (contra AGENTS.md:47, "nunca f64 para dinheiro") e `Decimal` no `RiskConfig`, o que obriga um `from_f64_retain` no meio de um teto de risco.

**Correção:** republicar a tabela a partir da medição pré-Gateway (patch aplicado); escrever o caminho do dado (o candidato natural é o `MarketContextAnalyzer` calcular o campo e ele entrar no `MarketContext`, o que fecha a paridade live/backtest por construção); `Decimal` nos dois lados.

### Bloco E — Dados: o banco que gera todos os backtests está contaminado

**Onde:** o plano trata o feed esparso como problema de **produção** (`§2.3` achado 7 e `§5.8`) e manda reingerir apenas os 6 símbolos parados (`§5.6`). Medido no banco dev: nas 19 sessões posteriores a 07/08/2026, **4 a 15 sessões por símbolo** têm volume abaixo de 30% da mediana de julho (AVUV 10/19, IJS 9/19, IWM 15/19, IWN 8/19, IWV 8/19, VBR 7/19, SLYV 4/18). O range médio por barra cai para 2,0–3,5 bp contra 11,0–12,7 bp nos dias ingeridos pelo PC/TWS, enquanto o range **diário** fica no mesmo patamar (0,834% vs 0,897%). **Ressalva do adversário, que muda o conserto:** todas essas sessões têm as 26 barras completas — o que colapsa é o range intrabar e o volume, não a contagem de barras; "barras esparsas" descreve o mecanismo errado (feed amostrado/congelado, não candles faltando), e o mínimo de 0,2 bp não reproduz (o mínimo medido é 1,3 bp em IWV).

Isso importa porque **o stop das três estratégias sai do range da barra de 15m**: todo número medido em ago–set/2026 tem distância de stop contaminada, incluindo o holdout que a ADR-019 propõe (que termina em 02/09) e o re-run do gate A do M1. Correlato medido: as razões pós/pré-Gateway pela mediana **por barra** são AVUV 9,0%, IJS 23,2% e **IWM 3,3%** — a faixa "IWM 7–18%" do `plano:93` está errada por um fator de 2 a 5 (nenhuma das quatro janelas testadas chega a 7%).

**Correção:** reingerir 08/08→02/09 dos 7 pares vivos pelo PC/TWS **antes** de gravar qualquer baseline novo, e ligar um check de qualidade de barra (volume e range contra a mediana móvel de 60 pregões). Nenhum número que inclua ago–set/2026 deve decidir nada até lá.

### Bloco F — Paridade live × backtest: guardas assimétricas e divergências não registradas

**F1. `pesquisa:967` está refutado.** Dizia: "o live já reproduz essas invalidações [de overshoot do simulador] pela guarda pré-envio do ADR-015". As duas guardas observam coisas diferentes. No backtest `reference_price` = close da barra do **sinal** (`engine.rs:200-203`) e o gatilho é sempre além do extremo dessa mesma barra (`range_extreme_fade_v1/entry.rs:36-39`, `balance_area_breakout_v1/entry.rs:40-41`), logo o overshoot é **sempre negativo** e a guarda é estruturalmente inerte: 0 ocorrências de "passou do gatilho" em 10 logs de backtest, contra 1 a 7 ocorrências por run da guarda do simulador ("abertura além do gatilho"). No live `reference_price` é o close da barra **em formação** (`paper.rs:1352-1356`), 30 s a ~15 min depois. Há rejeições que só existem no live e rejeições que só existem no backtest. O §6.7 do plano (`:357`) descreve isso corretamente — o relatório de pesquisa contradiz o plano no mesmo corpo de trabalho.

**F2. Divergências que nenhum documento registra** (todas confirmadas em código):
- o flatten do live **não** roda em posição que a instância não rastreia — vira alerta crítico e **atravessa** o fechamento (`paper.rs:2246-2254`, com o caso real das 827 ações de IWM órfãs desde 07/08/2026). A regra do live é "nenhuma posição **rastreada** atravessa", e a ADR-018 vai simular um encerramento que o live não executa exatamente no caso que ocorreu em produção;
- guarda de estabilização de barra só no live: até 30 polls de 30 s e, depois disso, a barra é **abandonada sem ser avaliada** (`paper.rs:1035-1150`, evento `bar_abandoned`, com o comentário citando as três barras puladas em 01–02/09/2026). O backtest avalia 100% das barras — viés de amostragem de mão única;
- warmup do live limitado a 600 barras (~23 pregões) contra a série inteira no backtest (`paper.rs:704`, `:992-993`);
- o limite de exposição ignora **ordens pendentes**: `exposure_limit_hit` recebe só `Vec<Position>`, e uma entrada stop trabalhando (até 30 min com `entry_validity_candles = 2`) não produz linha de posição. Com 8–11 instâncias na mesma conta, várias podem passar na checagem ao mesmo tempo e estourar o teto de 3 posições. A correção proposta na ADR-020 §4 só soma a posição prospectiva, não as ordens abertas;
- o alvo não paga slippage no simulador e a comissão é fixa em US$ 0,35/perna (já registrado em §5.6, mas com efeito de 10–12% do net que ninguém reconciliou — ver bloco G3).

### Bloco G — As três ADRs propostas: defeitos que fazem a implementação falhar em silêncio

**G1. ADR-018 (`:33-37`)** afirma que `trade_repository.rs` tem "`match` exaustivo nos dois sentidos". A leitura (`:325-331`) termina em `_ => trader_domain::ExitReason::Target`: sem o braço `"end_of_day"`, **todo trade de flatten lido do banco volta como alvo**, sem erro de compilação — corrompendo exatamente a métrica por `exit_reason` que a ADR existe para criar. Só a escrita (`:25-31`) é exaustiva.

**G2. ADR-018 (`:38-40`)** baseia a regra de "última barra" em "26 barras por dia … inequívoca". Há **4 pregões de meio expediente** na amostra (03/07/2025, 28/11/2025, 24/12/2025, 07/08/2026), com 14 barras terminando às 12:45 ET. Nesses dias o segundo critério da Decisão 3 nunca dispara e — mais grave — o flatten do **live** (janela fixa 15:55–16:10 ET, `paper.rs:2189-2197`) roda com o mercado fechado: a posição atravessa a noite sem stop, porque as pernas do bracket são TIF Day. `grep` por `half|early_close|holiday` em `paper.rs`/`session.rs` não devolve nada: não há tratamento de fechamento antecipado em lugar nenhum.

**G3. ADR-018 vs estudo de saídas — dois números oficiais para a mesma régua.** A ADR publica balance com flatten em **avg R −0,007 / +6.730**; o estudo pareado de 07/09 publica, para os mesmos 96 trades, **avg R −0,037 / net 6.062**. A diferença é integralmente o modelo de comissão (US$ 0,35/perna no simulador contra US$ 0,005/ação com mínimo de US$ 1,00 da IBKR Canada): medido, o delta é −668 (balance), −528 (fade) e −282 (openrev) dólares, ~US$ 7/trade. **Ressalva do adversário, que derruba metade da leitura:** a queda "de PF 1,55 para PF_R 0,94" **não** é efeito de comissão, é troca de métrica — nos mesmos trades sem flatten e com a mesma comissão, PF$ = 1,92 e PF_R = 1,32. A balance reprova o PF_R ≥ 1,2 proposto pela ADR-019, mas reprovaria também com a comissão do simulador.

**G4. ADR-019 (`:158-160`)**: o índice único `(strategy_id, asset_id, config_hash, period_start, period_end, label)` **não constrange**, porque `label` é nulável (`0003_backtest_runs.sql:17`) e no Postgres NULLs são distintos — **566 de 687 runs (82%) continuariam duplicáveis**. O servidor é 15.15 e suporta `NULLS NOT DISTINCT`, que a ADR não pede. E a magnitude do dedupe está errada por ~12×: pela chave da própria ADR são **109 grupos e 279 linhas**, não "22 cópias".

**G5. ADR-019 (`:124-126`)**: o holdout travado (últimos 4–6 meses) **já foi lido** e está impresso nos documentos das v2 ("após 01/12/2025 PF 0,88", "2026: 50 trades, PF 0,60", "blocos 6–7 = −586 e −5.228"). Além disso ele não tem potência: medido, o holdout de 6 meses tem **33 / 16 / 18 trades em 19 / 14 / 16 datas** (4 meses: 28/10/11), com SE(avg R) de 0,26 a 0,38 — nunca cumprirá os "≥ 50 trades" do ADR-010 e não distingue avg R 0,15 de zero. Como está, o critério será afrouxado em silêncio no M2.

**G6. ADR-019 §1** está intitulada "CLI do walkforward", mas quatro documentos assumem `--label`, `--set`, `--output`, `--holdout-from` e `--no-flatten` também no `backtest` (`fade-v2:265`, `balance-v2:209/:223`, `ADR-020:304`, `ADR-018:128`). Hoje o `backtest` tem apenas symbol/strategy/from/to/timeframe/allow_synthetic/output/slippage_bps (`main.rs:96-122`). Metade do protocolo das v2 não roda. Correlato: o override por env proposto na fade-v2 é **invisível à auditoria** — o `config_hash` é SHA-256 só da config da estratégia, o `[risk]` não entra, e a ADR-019 só grava `overrides` para `--set`; o run com a flag ligada fica indistinguível do baseline no banco, que é exatamente a contaminação que a ADR existe para impedir.

**G7. ADR-020 (`:102-104`, `:203-204`)** afirma que o teto de 200% "bloqueia" a alavancagem intraday. Com a checagem atual a **primeira** posição alavancada abre (o teto só olha posições já abertas); a frase só passa a valer depois do item 4 da própria ADR. E deixar `max_daily_loss_pct` "em aberto" (`:243-247`) afrouxa um limite de risco em silêncio: se o sizing usar `capital_ef = capital × 1/3` e o corte diário continuar sobre `capital` cru (`risk/mod.rs:121`), os 2% viram 6% da fatia efetivamente operada.

### Bloco H — Specs v2: regra do repo quebrada e testes que já nasceram reprovados

**H1.** Das três specs, apenas a `range-extreme-fade-v2` toca código de v1, e por dois caminhos: o Passo 1 altera `risk/mod.rs:158-163`, o `RiskManager` **compartilhado pelas 9 estratégias** (a decisão aceitar/rejeitar é preservada com `allow_neutral_context = false` — verifiquei a álgebra —, mas o `rejection_reason` gravado em `signals` muda para todas as v1); e o hotfix do §13 edita três arquivos da v1 (`context.rs`, `config.rs:121-122` e o TOML) sem bump de `version`, contra `framework:268` e `AGENTS.md:142`. **Ressalvas do adversário, ambas materiais:** (i) a linha 12 da spec **já traz** a exceção do §13 na mesma frase — a acusação de "nada na v1 muda" cai; (ii) o precedente A2 é análogo (o commit `e4231f3` diz literalmente que "os valores efetivos no horário de verão não mudam — só param de deslizar em novembro"), então exigir bump para 1.0.1 contraria um precedente já aceito pelo dono. O que sobra e vale: o hotfix é **comportamentalmente um no-op até 01/11/2026** (hoje, EDT, 15:30–18:00 UTC = 11:30–14:00 ET), o que permite aplicá-lo antes da virada do DST sem mexer na amostra viva — e a decisão sobre reiniciar ou não o relógio do gate B precisa ser escrita, porque `framework:286` proíbe mudar estratégia durante teste e a própria spec aplica essa lógica ao caso simétrico (`:238`).

**H2.** A fade-v2 trata como opcional ("cabe ao dono decidir se isto exige uma ADR curta … ou basta o registro na ADR-019") uma ADR que `AGENTS.md:75` torna obrigatória para mudança estrutural — e o fallback é falso: `grep` por `neutral|allow_neutral|OutsidePhase` na ADR-019 devolve **uma** linha, de sequenciamento. A ADR-019 não cria o campo, não divide o `NoContext` e não decide nada disso.

**H3.** `balance-area-v2` e `opening-reversal-v2` pré-registram como "holdout travado" meses cujos números **já estão impressos nas próprias specs** (balance §4b: 2026 PF 0,60, após 01/12/2025 PF 0,88, blocos 6–7 negativos; openrev §13: 2026 n=45 PF 0,82, 2026Q1/Q2 PF 0,59/0,38). Um holdout cujo resultado já foi lido não falsifica nada. A openrev-v2 é honesta no §0 ("já reprova com os dados existentes") e depois re-propõe o mesmo teste como novo. **Ressalva do adversário:** a openrev condiciona tudo à abertura da Onda C (`:200`), então a crítica é de coerência, não de decisão. Some-se um critério de morte que **não dá para calcular**: "PF por dia" não é definido em nenhum documento nem existe em `metrics.rs`.

**H4.** `balance-v2:96/:207/:213` e `plano:328` tratam "≥ 5 pares" como exigência vigente do gate A. Não existe em ADR nenhuma: o ADR-010:22-24 diz "≥ 50 trades" sem qualificar pares, e o gate A de 04/09 fechou a balance e a fade com **3 pares**. Aplicada retroativamente, a regra invalidaria as duas aprovações de 04/09.

### Bloco I — Cronograma e esforço: o calendário não cabe no orçamento declarado

`plano:424`, `:16`, `:171`, `:215`, `:310` rotulam a Onda A como "2–3 semanas" na mesma frase em que citam **19–29 dias-pessoa** — que, com dedicação integral, são 3,8 a 5,8 semanas; o roadmap já registra a correção (`:41`, "é o dobro") e o plano manteve o rótulo. Pior: os marcos datados assumem ~20 h/semana (2,5 dias-pessoa por semana), e de M0 (11/09) a M3 (20/11) são 10 semanas = **25 dias-pessoa** contra **32–49 dias-pessoa** de entregas listadas — 28% a 96% acima do orçamento. Tudo que depende do M3 (relógio do gate B, "fev–mar/2027", gate C) desliza junto. Erros aritméticos menores no mesmo bloco: subtotal do Tier A é 18,0–28,5 (declarado 19–29) e não inclui o A9; a Onda B soma 26–41 dias nas linhas da §2 (declarado 28–43) e 224–344 h (declarado ~240–340 h); "46–71 dias a 20 h/semana" são 4,2–6,5 meses (declarado 5–7).

Item relacionado que muda uma decisão de fila: o EV de "≈ +9,6k/ano da Onda B" inclui `#12` (openrev short-only, +1,0k) e `#21` (escada de risco/Kelly, +2,0k), que o próprio plano classifica como **Tier C** (itens 19 e 24). Só com a Onda B propriamente dita o EV é **≈ +6,6k/ano** — e, com o item #10 medido negativo (bloco A), cai para ≈ +4,5k/ano.

---

## 4. Imprecisões menores

| # | Documento:linha | O que diz | O que é verdade | Fonte da verificação |
|---|---|---|---|---|
| 1 | `plano:5` e `:301` | "candles 15m de 24/02/2025 → 02/09/2026" | IWM, QQQ e SPY começam em **21/02/2025** (26 barras); total 136.204 candles confere | SQL `min/max` por símbolo |
| 2 | `plano:78`, `ADR-018:57`, `fade-v2:242` | fade "+6.005 / +4.567" | **+6.015 / +4.578** (o ADR-016:79 já registra 6.015; a soma dos US$ 19.380 só fecha com 4.578). *Enfraquecido pelo adversário: erro de US$ 10–11, não muda conclusão* | soma dos JSONs; `crit_eod.py` |
| 3 | `plano:28` | "41 vereditos; 12 matar; 5 manter" | por proposta: 33 revisar, **7 matar**, **1 manter**; 109 vereditos individuais (82 R / 19 X / 8 M) | `pesquisa:59-63` |
| 4 | `plano:45-46` vs `:58` | "91 OOS PF 1,96" e "92 trades OOS" sem distinção | são duas amostras: walk-forward do motor (runs 413/415/416 = **91**, PF 1,957, +13.999) e reconstrução dos críticos por corte em 12/05/2025 (**92**, PF$ 2,091, +15.957) | SQL em `backtest_runs`; `_aud11.py` |
| 5 | `plano:91` | "dias com ≥ 2 entradas: PF 2,69; com 1 entrada: 0,70" | é só da balance-area; no portfólio dos 8 pares é **2,19 vs 0,88** (fade 4,64 vs 1,46; openrev 1,92 vs 0,96) | agrupamento por data ET nos JSONs |
| 6 | `plano:93` | dias do servidor "17–21/08 … 60–100k ações e 1,6–3 bp" | **17/08 está fora**: 255.994 ações e 3,12 bp de mediana (6,03 de média) | SQL por dia em AVUV |
| 7 | `plano:97` / `ADR-020:66-69` / `ADR-018:165-166` | três valores para a mesma barra mediana e dois rótulos de janela errados | ver §5: a medição não converge e fica sem patch | SQL em 4 janelas |
| 8 | `plano:130-131` | "`strategy.rs:460-465`" | o arquivo tem **56 linhas**; a assinatura é `crates/trader-domain/src/strategy.rs:50-55`; `StrategyState` ignorado confere (`engine.rs:184`, `paper.rs:472` passam `&Default::default()`) | `find` + `wc -l` |
| 9 | `plano:137` | "entrada limit enche na hora (`simulated/broker.rs:522-575`)" | o ramo citado **nunca é alcançado** por sinal (`build_bracket_order` sempre emite `Bracket`); o caminho real é `:522-537` + `:611-640`; hoje inerte (9 TOMLs usam `stop`) | leitura de código + `grep` nos TOMLs |
| 10 | `plano:165`, `ADR-020:212`, `fade-v2:238`, `pesquisa:318` | "0,74 trade/pregão" | **0,60** (232 trades / 384 pregões, 8 pares vivos); o 0,74 era com 9 pares, incluindo a pullback desligada | contagem + `replay.py` |
| 11 | `plano:223` | "`trade_repository.rs:28-30, 329` têm match exaustivo" | mesmo erro do bloco G1 (a leitura tem catch-all) | leitura de código |
| 12 | `plano:260` | "PSR balance 0,938; fade 0,984; openrev 0,82" | corretos, mas são da régua **antiga**; com flatten viram 0,48 / 0,95 / 0,98 | PSR de Bailey/López de Prado sobre `result_in_r` |
| 13 | `plano:282` | "529–572 … 660–676 sem label; 22 cópias" | 191 runs sem label em 06/09 (ids 459–649); em 660–676 só **8** estão sem label (9 já têm `research-exit-policy-2026-09-07`); o dedupe pela chave da ADR-019 são 109 grupos / 279 linhas | SQL |
| 14 | `plano:298` | "\|close−open\| mediano 5–7 bp, ~1/4 do stop" | **6–11 bp** conforme o ativo (8,5 bp no pool; só IWV e SPY caem em 5–7); ~1/3 do stop. A conclusão do parágrafo fica **mais forte** | SQL percentil por símbolo |
| 15 | `plano:352` | "21,9% em IJS, 24% em IWV, 10% VBR, 3% AVUV" | correto, mas **mistura estratégias**: os três primeiros são da balance-area e o IWV é da range-fade | contagem nos logs |
| 16 | `plano:89` | "39–42% de todas as barras são Neutral" | 38,7–41,6% com a regra real; na janela operacional 41,6–44,1%. *Enfraquecido: é arredondamento* | réplica de `classify_trend` |
| 17 | `plano:392` | "Kraken 0,40/0,80 ou 0,25/0,40 — a confirmar"; "25–80 bp/lado" | a página salva responde: Tier 1 = **0,40/0,80** (spot 0,38/0,80); 0,25% só é taker do Tier 6; o app não-Pro é **pior** (1% a 1,5%). O piso de 25 bp não tem fonte, e `plano:15` já usa 40–80 | `www_kraken_com_features_fee-schedule.html` |
| 18 | `plano:15`, `:147`, `:391` | "12–18 bp de um review terceiro" | é fonte **primária da IBKR**, mas do site **.com (EUA)**, com **mínimo de US$ 1,75/ordem** que nenhum documento cita; a página .ca de cripto responde 410 e a .ca de comissões não publica tarifa | páginas salvas |
| 19 | `plano:66` e `:365` | Grimes "stop nunca menor que um range médio de barra" | a análise do livro no repo diz **"raramente menos de 2 ATRs, às vezes mais de 4"** (`grimes-art-science-ta.md:150`) — o piso proposto (1 × ATR14) é **mais frouxo que a fonte** | leitura do arquivo |
| 20 | `plano:146`, `:393`, `:403`, `:432` | "permissão CLP (commodity pool)" | CLP = **Complex Leveraged Products**. *Enfraquecido: "Complex or Leveraged ETPs" é a redação literal da própria IBKR* | páginas salvas |
| 21 | `plano:256`, `pesquisa:699`, `balance-v2:223` | "`trader-research/`" no presente | o diretório **não existe** e nunca foi versionado; só a ADR-019:215 o marca como novo | `ls` + `git log` |
| 22 | `pesquisa:448-450` | "`q3.sql:39-49` implementa…" | o conteúdo confere (536 linhas em q1..q6), mas o arquivo **não está no repositório** — a citação é órfã e sustenta a reprovação de 7 candidatos | `find` |
| 23 | `pesquisa:116`, `:1055`, `ADR-019:5`, `:35` | "plano §2.5" | é **§2.6** (§2.5 é o baseline de dinheiro) | leitura do plano |
| 24 | `fade-v2:244`, `pesquisa:320` | "plano §5.9" para a saída de IWV | é **§5.10** (§5.9 é o A9) | leitura do plano |
| 25 | `pesquisa:85`, `:229` | edge-existente-3 como "**B** #19" | no plano o item 19 é **Tier C** (§6.3 "movida para a Onda C") | leitura do plano e da spec |
| 26 | `pesquisa:16-17`, `:52-55` | "ADR-018/019/020 ainda não existem"; "`sql/screens/` ainda não existe" | ambos existem como untracked; o próprio relatório se contradiz em `:1493` | `git status`, `find sql` |
| 27 | `pesquisa:469-470` | controle positivo "balance ex-overnight, PF 1,43" | o plano §5.7 e a ADR-018:66-67 mandam o oposto: **com flatten, PF 1,55 / avg R ≈ 0** | leitura + `crit_eod.py` |
| 28 | `plano:447` vs `pesquisa:65-71` | "vereditos dos 2 críticos … e no relatório de pesquisa" | o relatório diz duas vezes o contrário; e §1 ("16 dos 18 rodaram") não fecha com §11 | leitura |
| 29 | `fade-v2:40` | IWV "PF 1,14" | **1,15** (o JSON dá 1,14507…), como o próprio §14 item 11 e o plano §5.10 usam | JSON do run 547 |
| 30 | `balance-v2:60-67` | "pool de 8 símbolos, 237 trades" | nenhum documento declara a composição; o estudo de saídas declara outro pool de 8 e conta **241** trades; o pool da v2 inclui IJR e SCHA, sem dado após 06/08/2026 | contagem dos JSONs |
| 31 | `roadmap:142-143` vs `:41`/`:331` | "45–70 dias / 360–560 h" e "46–71 / 370–570 h" | a tabela §5 soma **45–70,5**; o documento publica duas somas | soma das linhas |
| 32 | `roadmap:286` vs `plano:119` | cenário D "2–4k" e "2–5k" | medido: **2,6k** (ex jun–out) e **4,5k** (2026) | `replay.py` |
| 33 | `sql/screens/run.sh:4` | manda `tee docs/reports/screens-raw/` | o diretório não existe; o `tee` falha sem `mkdir -p` | `ls` |
| 34 | `pesquisa:1494` | "`ops.yml:15-27`" | as ações vão de `:21` a `:29`; o intervalo corta justamente `flatten` e `cancelar-ordens` | leitura |
| 35 | `plano:353`, `pesquisa:1147` | "`calendar_tags` … de `config/calendar.toml`" | o arquivo não existe (é "a criar", como o resto da frase marca para o `day_type.rs`) | `ls config/` |
| 36 | `plano:465` | §12 lista `sql/screens/` | falta `sql/stats/` (13 consultas que sustentam §2.2, §2.4 e a ADR-020 §3) e falta o estudo de saídas | `find sql`, `grep -rln` |
| 37 | 37 citações em 7 documentos | `entry.rs:44`, `context.rs:23-38`, `mod.rs:104-146`, `config.rs:46`… | nome-base ambíguo: `mod.rs` casa com 21 arquivos, `config.rs` com 11. Caso pior demonstrado: `ADR-019:26` e `:32` citam **dois `walkforward.rs` diferentes** com seis linhas de distância | `find` |
| 38 | `plano:87`, `fade-v2:98` | "risco real por trade 0,15–0,32%" | mediana **0,24%**, p10 0,13%, p90 0,50%; medianas por par de 0,13% (IWV) a 0,35% (IWM); 0 de 232 trades em 1%. O §2.5 do próprio plano já traz o número certo | 232 trades dos 8 JSONs |
| 39 | `plano:164`, `balance-v2:137`, `openrev-v2:100`/`:292` | "sempre stop server-side" | verdadeiro **no envio** (`broker.rs:553-556`, `:628-636`), falso na rejeição parcial: TP e SL vão fire-and-forget sem confirmação (`:665`, `:695-702`); o watchdog exige 2 detecções, não repõe sem `known_stop` e não corrige divergência de quantidade. Em `openrev-v2:100`, a citação `:605-612` aponta para a perna parent, não para o stop | leitura de código |
| 40 | `plano:275`, `ADR-020:79-83` | "o que morde é `positions.len() >= 3`" | correto, e **incompleto**: a checagem também ignora ordens pendentes (ver bloco F2) | leitura de código |

---

## 5. Afirmações que ficaram sem verificação possível

| Afirmação | Onde | Por que não deu | O que seria preciso |
|---|---|---|---|
| Slippage real do flatten (fill MKT das 15h55 × close da 15:45) | ADR-018 "a medir"; `balance-v2:14` passo 6 — é **kill pré-registrado** da v2 | o banco dev tem 3 trades e 26 fills, todos de 04/08 com comissão zero | dump do banco de produção ou ação `sql` read-only no `ops.yml`. Enquanto isso, o kill criteria da balance-v2 não é avaliável, e 55–58% das saídas dela são o flatten |
| Contagem de `NoContext` em produção | `fade-v2` §6.1 item 5; `roadmap` §6 | `signals` do dev tem 293 linhas, todas de `pullback-trend-v1` (31/07–07/08/2026), zero `NoContext` | dump de produção — e, hoje, seria inconclusivo, porque `validate()` emite um único `NoContext` para Neutral, vol High e fase ≠ Regular |
| Latência por barra, `submit_latency_ms`, custo por fill real | `plano:93`, §5.8 | não há fonte no banco dev; a query com `signals.timestamp` não serve (é `Utc::now()`) | `market_contexts`/`orders` de produção, mais o item 3 do §5.8 implementado |
| "IWM em 01/09 teve range real de 0,57%" | `plano:93` | o 0,19% gravado reproduz na barra; o 0,57% vem de fora do banco | fonte externa (TWS/PC) preservada |
| Causa raiz do feed esparso (TWS → Gateway vs tipo de market data) | `plano:93`, §5.8 | medido o **efeito** no banco; a causa exige `debug-candles` nos dois hosts na mesma barra | item 1 do §5.8 |
| "0 trades das aprovadas desde 18/08" e o ritmo real do gate B | `plano:165`, `:445` | a tabela `trades` do dev tem 3 linhas de 04/08/2026 | dump de produção |
| IC95 por bootstrap em blocos e MC de drawdown (`plano:260-262`) | §5.3, roadmap §1.2 | dependem de semente e esquema de reamostragem não documentados; só o PSR foi reproduzido (0,936 / 0,988 / 0,819 contra 0,938 / 0,984 / 0,82 publicados) | o script com semente fixa versionado (é o `trader-research/` que não existe) |
| "22 cópias em 9 grupos" (`ADR-019:48-51`, `plano:282`) | — | nenhum agrupamento testado devolve 22 e 9 juntos (109/279 pela chave da ADR; 26/26 e 9/74 por chaves alternativas), e o documento não declara o recorte | declarar chave e recorte, ou remover o número até o dedupe rodar |
| "PF por dia" como kill criteria (`balance-v2:204`, `:217`) | — | a métrica não é definida em documento nenhum nem existe em `metrics.rs` (16 campos) | escrever a fórmula e acrescentá-la ao item 4 da ADR-019 |
| Tipos de ordem da API de cripto da TWS ("MKT-IOC e LMT de 5 min, sem STP") | `plano:147`, `:391` | nenhuma das 6 páginas salvas contém "MKT", "IOC", "stop order" ou "bracket"; nenhuma página da TWS API foi salva | página de Order Types da TWS API. É a razão *load-bearing* do arquivamento de cripto |
| "Coinbase Advanced 0,40/0,60" | `plano:392` | a página salva é um shell de JS. *Enfraquecido: o taker de 0,60% aparece na tabela comparativa da IBKR salva; só o maker de 0,40% fica sem fonte* | captura da tabela de tarifas |
| Barra mediana de liquidez: qual é o número | `plano:97`, `ADR-020:66-69`, `ADR-018:165-166`, `pesquisa:795` | **três valores publicados e quatro medições**: RTH pré-07/08 (SLYV 365k / IJS 526k) reproduz o plano mas o rótulo diz 11h–13h; 11h–14h desde mar/2026 (290k/370k) reproduz a ADR-020, e nessa janela 238k **é** 82%/64%, o que derruba o achado que dizia o contrário; 11h–13h pré-07/08 dá 320k/413k (74%/58%); e uma quarta medição (mar–ago/2026) dá 345k/436k, mas com janela que na verdade termina em 06/08 | uma definição única escrita (janela horária, período, fonte, N) e uma medição republicada. **Sem isso o cap de liquidez da ADR-020 não pode ser calibrado** |
| Horas de manutenção, equity real e moeda-base da conta | `roadmap` §1.4 e Q6/Q8 | ação do dono | `trader-cli account --provider ibkr` (15 min) + registro de horas |

---

## 6. As análises que faltavam

### 6.1 Setups novos dentro da assinatura de edge — 8 candidatos desenhados e medidos (Fase 0) — **SOBREVIVE PARCIALMENTE** (4 leituras estruturais derrubadas; os números resistem)

**Veredito do adversário, que reexecutou as `.sql`:** todos os números reproduzem dígito a dígito. Caem quatro afirmações estruturais: (a) "o controle positivo do screener PASSA nos 3 pares vivos e valida o instrumento" — ele reprova **duas** das cinco pernas do critério (mesmo sinal 2025/2026 e ≥ 40 datas/ano), e o resumo cita o critério com três pernas, omitindo a que ele reprova pior; além disso restringir o controle aos pares que o gate A selecionou é circular; (b) a leitura do candidato principal — **3 de 177 fills caem depois de 07/08/2026 mas carregam +3,9R dos +10,7R** (avg R +1,288); limpos, o total cai para +6,8R (−36%) e 2026 vira PF 0,84 em 67 fills, num setup disparado por **volume**, medido numa amostra cujo volume está corrompido; (c) a "ablação" não ablaciona nada — o setup proposto nunca teve condição de barra larga, e a tag `abl_vol` é a tag `v25` renomeada; (d) "passa o critério de estabilidade ±20%" — a varredura foi 2,0/2,5/3,0×, isto é base, +25% e +50%; o sentido de baixa não foi medido. Sobrevive: o veredito de fundo ("nenhum candidato passa"), que fica mais forte, e todas as medições.

**Método.** Todo número saiu de `sql/screens/_lib.sql` + `_eval.sql` com `hold_days = 0` (flatten no fechamento da 15:45 = régua do live), `gap_cancel = true` (ADR-015), `risk_min = 0,0020`, custo 4 bp + US$ 0,01/ação, alvo 1,5R. Arquivos no scratchpad, fora do repositório. Universo da assinatura = IJS, VBR, AVUV, SLYV, IWN (os 5 small/mid value do §2.1); 384 pregões por ativo (216 em 2025, 168 em 2026) = 1,524 ano-ativo.

| # | Setup | ev/ano/ativo | fills 25/26 | PF_R 25/26 | avg R | t (por data) | stop p50 | custo em R | Veredito |
|---|---|---|---|---|---|---|---|---|---|
| 1 | **volume-climax-fade** (vol ≥ 2,0× média20) | **23,2** | 107 / 70 | **1,23 / 0,96** | **+0,060** | 0,57 | **36,3 bp** | **0,144** | reprova (t, sinal 2026, trimestre) — único acima do controle |
| 1b | mesma, vol ≥ 2,5× | 16,8 | 80 / 48 | 1,29 / 0,92 | +0,073 | 0,63 | 38,9 bp | 0,143 | idem |
| 2 | balance-failure-fade (área de 78 barras) | 8,1 | 37 / 25 | 1,33 / 0,69 | +0,023 | 0,13 | 21,6 bp | 0,226 | nasce morto por amostra (máx. 19 fills/ativo) |
| 3 | d3-failure-fade (extremo de 3 pregões) | 5,2 | 26 / 14 | 1,94 / 0,92 | +0,230 | 1,18 | 20,4 bp | 0,216 | nasce morto por amostra (máx. 10 fills/ativo) |
| 4 | stretch-mean-fade (close ≥ 1,2×ATR14 da EMA20) | 8,9 | 35 / 33 | 1,56 / 1,37 | +0,224 | 1,24 | 17,7 bp | 0,277 | estabilidade é artefato (ver abaixo) |
| 5 | pdhl-failure-fade (PDH/PDL de ontem) | 7,6 | 32 / 26 | 1,06 / **0,26** | −0,261 | **−1,66** | 21,5 bp | 0,212 | **MORTO pela medição** |
| 6 | ib-failure-fade (Initial Balance) | 13,5 | 62 / 41 | 0,98 / **0,30** | −0,244 | **−1,80** | 21,2 bp | 0,227 | **MORTO pela medição** |
| — | *controle* `range-extreme-fade-v1` (ctx_piv) | 10,6 | 49 / 32 | 1,33 / 0,61 | **0,000** | 0,00 | 21,6 bp | 0,222 | é a régua |

Mortos antes de virar proposta: **vwap-reversion** (stop p50 13,8–15,7 bp, 66–76% dos sinais abaixo de 20 bp, custo 0,30–0,34 R, 2026 PF 0,17–0,48 com t de −1,47 a −2,57; e a coluna `vwap` de `candles` é **0 de 136.204** preenchida) e **second-entry-fade** (580 sinais, 56 são 2ª tentativa e 2 são 3ª; **3 fills válidos** em 18,3 meses no universo da assinatura — o achado 8 do plano é real mas não é operacionalizável como regra de entrada).

**Calibração do screener** (o item que o adversário corrige). Controle positivo `ctx_piv` (modelo mais fiel da v1) contra a referência do plano §2.3 (69 trades, avg R 0,218, PF 1,74):

| Universo | fills | avg R líq. | WR | stop p50 | PF_R | Datas/ano |
|---|---|---|---|---|---|---|
| AVUV + SLYV + IWV (pares vivos) | 40 | +0,332 | 62,5% | 29,1 bp | 1,82 | 24 (2025) / 13 (2026) |
| 5 ETFs small/mid value | 81 | 0,000 | — | 28,3 bp | 1,33 / 0,61 | — |
| 11 ETFs do `_lib.sql` | 196 | −0,062 | — | ~21,6 bp | 1,13 / 0,60 | — |

O PF fica dentro de +5% da referência, mas isso é a soma de dois erros que se cancelam: o screener acha 40 dos 69 fills (58%) e é +52% otimista no avg R. E no universo dos 5 value o controle soma **exatamente 0,00 R** em 81 fills (2025 +8,4R, 2026 −8,4R) — a `range-extreme-fade-v1` só é positiva porque roda em AVUV/SLYV/IWV.

**Setup 1 — `volume-climax-fade`, o único que sobrevive à triagem.** Fade de barra que faz novo extremo do dia com volume ≥ 2× a média das 20 barras anteriores e fecha no terço oposto; entrada stop 1 tick além do extremo oposto da barra de sinal, validade 2 barras, `gap_cancel` 0,25, stop no extremo da barra, alvo 1,5R, flatten na última barra RTH, **uma entrada por símbolo por dia**. Frequência por símbolo (fills 2025/2026): IWN 34/17 (33,5/ano), SLYV 25/20, IJS 22/15, VBR 17/9, AVUV 9/9. Varredura do parâmetro: 2,0× → +0,114 / −0,022; 2,5× → +0,138 / −0,038; 3,0× → +0,218 / −0,144 (monotônico, sinal não inverte). Stops: p10 16,0 / p50 36,3 / p90 75,3 bp, **16,5% abaixo de 20 bp** contra 41,2% do controle; custo 12,3% do R contra 16–18%. Fora do universo da assinatura o resultado morre (IWM −0,125/−0,247, QQQ −0,019/−0,211, SPY −0,114/−0,195, MDY +0,037/−0,474, SCHA −0,419/−0,073, IJR −0,390/−0,256), exatamente como o §2.1 prevê. Por símbolo, só IJS tem o mesmo sinal nos dois anos (PF 1,46 / 1,32). Concentração: **2025Q2 sozinho soma +9,8R contra +9,3R do total** — tudo fora de abr–jun/2025 soma −0,5R. Ablações: só volume (128 fills, +0,138/−0,038, t 0,63); só barra larga (785 fills, −0,057/−0,225, PF 0,90/0,64, **t = −2,11**); as duas (114 fills, +0,208/−0,055); + dia de range (111, +0,193/−0,108); + contexto global (63, +0,129/−0,013). *Ressalva do adversário:* como o próprio texto declara que |t| = 2,00 é o máximo esperado sob a nula com ~500 leituras, o t = −2,11 da barra larga não pode ser lido como "significativamente negativo"; e há um defeito na definição — 45 de 1.272 sinais (3,5%) recebem direção invertida em relação à lógica da barra, porque o `CASE` atribui direção por `high > th_prev` independentemente de qual disjunção disparou.

**Achado transversal e acionável:** o cluster **inverte** aqui. Dias com ≥ 2 sinais no mesmo símbolo dão avg R −0,326 e PF_R 0,54, contra −0,039 e 0,93 nos dias de sinal único — o oposto do achado 8 do plano (PF 2,69 na balance). Por isso "uma entrada por símbolo por dia" é parte da spec, não trava de risco.

**Refutação útil para a v2 da range-fade:** a hipótese "o que paga é a distância da média, não o novo extremo" está **refutada** — a versão sem exigir novo extremo tem 2× mais eventos (18,4 vs 10,6/ano/ativo) e avg R agregado pior (−0,059 vs 0,000). E o `stretch-mean-fade` "estável" é artefato: short 2025 avg R −0,323 / long +0,362; short 2026 +0,246 / long −0,476 — quatro células de ~30 trades com sinais trocados; sem o filtro de contexto o efeito some (+0,003 / −0,098).

**Famílias fechadas nos dois sentidos:** os níveis de ontem (PDH/PDL) e o Initial Balance não produzem reversão vendável em 15m nesses ETFs — nem rompidos (screens 01/03/06) nem quando o rompimento falha (setups 5 e 6, com WR de 22–27% em 2026 e t de −1,66 a −1,80).

**Contagem de tentativas (obrigatória):** 35 tags novas × 4 alvos × 3 universos ≈ **500 leituras**. O maior t observado (2,00, tag `vc35` — que, observa o adversário, **não aparece no relatório da análise**, embora tenha 2025 t = 2,86 e PF 3,15) é o esperado do máximo sob a nula. **Nenhum número desta seção é fora de amostra.**

**Recomendação:** não implementar nada. Guardar como conhecimento de custo zero: o edge do clímax está no volume e não na largura da barra; o cluster não se transfere; "distância da média" não substitui "novo extremo"; níveis de ontem e IB estão fechados. Quando o feed e a ADR-018 estiverem prontos, rodar o setup 1 **uma vez** no holdout mai–set/2026 com 2,0× e universo IJS+SLYV+VBR pré-declarado. Corrigir o `sql/screens/README.md` §4 com esta calibração — e registrar que `screens-raw-extra/09-base.txt` tem 5 tags e **não contém** `ctx`/`ctx_piv` (quem ler aquele `.txt` conclui que o controle reprova com PF 1,05/0,67, que é o resultado da tag sem filtro de contexto), e que o `.sql` foi editado depois daquela saída (`ext_m20` passou de 714 para 381 sinais) — uma alteração de definição após ver resultado, que conta para `n_trials` e não está registrada.

### 6.2 A alavanca "liberar contexto Neutral" — **SOBREVIVE** (a mais sólida das quatro; duas ressalvas de leitura)

**Veredito do adversário:** confirmou o achado central por duas vias que executou (grep nos logs e execução do `aud_signals.py`), reproduziu a re-simulação e a validação do replay, e reproduziu as consultas ao banco. Enfraquece duas leituras: o corte **por par** não aparece no resumo e desmente "abandonar"; e a permutação p = 0,0066 compara o incremento contra uma base viesada por sobrevivência (os 3 pares foram selecionados pelo gate A). O conteúdo abaixo já está com as duas ressalvas embutidas.

**Origem do número.** O log de debug da medição original não foi preservado (`grep -rn NoContext` no scratchpad só acha `.md`). Os logs INFO da mesma rodada dão:

| Par | "setup … detectado" | "entrada executada" | Rejeições reais | Alegado | Razão |
|---|---|---|---|---|---|
| AVUV | 55 | 31 | **24** | 48 | 2,00× |
| SLYV | 48 | 30 | **18** | 36 | 2,00× |
| IWV | 66 | 35 | **31** | 62 | 2,00× |
| **Total** | **169** | **96** | **73** | **146** | **2,00×** |

`broker rejeitou` = 0 nos três logs. Uma réplica em Python do pipeline de sinal da v1 e do `MarketContextAnalyzer` devolve 55/48/66 sinais com 24/18/31 bloqueados por `is_tradeable` — fecha nos três pares, o que elimina qualquer outra razão de rejeição. Erros correlatos no mesmo parágrafo: "~60% dos sinais morrem no `is_tradeable`" → **43,2%**; "a contagem inclui repetições em barras consecutivas, então é um teto" → só **3 dos 73** são repetição no mesmo dia (23/18/29 dias distintos); "31/30/35 entradas" são ordens enviadas, não trades (29/21/19). E `NoContext ≡ Neutral` aqui: **zero** das 169 barras de sinal têm `VolatilityRegime::High`, e todo sinal está em 09:45–15:15 ET.

**Contexto medido com a regra real** (`close > EMA20 > SMA200` = Up; inverso = Down; resto = Neutral; EMA de janela do projeto), janela 09:45–15:15 ET, barras com índice ≥ 200:

| Ativo | Barras | Uptrend | Downtrend | **Neutral** | vol High | Bloqueadas |
|---|---|---|---|---|---|---|
| AVUV | 8.614 | 34,0% | 22,2% | **43,8%** | 0,3% | 43,9% |
| SLYV | 8.601 | 33,2% | 23,3% | **43,5%** | 0,2% | 43,7% |
| IWV | 8.614 | 38,4% | 20,0% | **41,6%** | 0,2% | 41,7% |

Barras que passam o detector real de dia de range (`check_range_day`): AVUV 68,7%, SLYV 66,5%, IWV 72,9% de **todas** as barras da janela, com Neutral em 49,4% / 48,3% / 43,1% delas — excesso de ~5 p.p. sobre a média geral. E 96,0–96,8% dos **dias** têm ao menos uma barra "de range": o detector não classifica dia, classifica uma condição local de 12 barras. A fração Neutral é estável em 37,5–48,7% nos 21 trimestres-ativo.

**Teto de trades adicionais.** Período = 555 dias = 1,520 ano. Taxa de fill observada por par: 93,5% / 70,0% / 54,3% → **14,8 + 8,3 + 11,1 = 34,1 trades/ano** nos três pares. É teto: o motor pula a análise com posição aberta, há `max_trades_per_day = 3` e as travas de conta. Contra o gate A: AVUV 26 OOS hoje → 48 no teto; SLYV 20 → 37; IWV 18 → 47 — **nenhum chega a 50**, e o agregado (64) já passava antes.

**Sensibilidade.** IC90 bootstrap do PF_R atual (69 trades): [1,11; 1,67; 2,56]. Adicionando K trades sorteados da distribuição empírica:

| K | Acerto dos novos | PF_R p5 | mediana | p95 | P(PF_R < 1,3) |
|---|---|---|---|---|---|
| 37 | igual (61%) | 1,20 | 1,67 | 2,36 | 10,5% |
| 37 | −20% → 49% | 1,01 | 1,40 | 1,96 | 35,4% |
| **73** | **igual** | 1,26 | 1,67 | 2,24 | 7,6% |
| **73** | **−20%** | 0,98 | 1,29 | 1,72 | **51,3%** |
| **73** | **−40% → 37%** | 0,76 | 1,00 | 1,32 | **94,0%** |
| 110 | −40% | 0,71 | 0,91 | 1,16 | 99,2% |

Pontos de equilíbrio a K = 73: acerto de 49,2% deixa a mediana em PF_R 1,3; 36,3% deixa em 1,0.

**O PF do incremento, medido.** Replay validado contra o backtest oficial na população **não** bloqueada: AVUV 29 / 58,6% / PF 1,64 / avg R +0,323 (backtest: idêntico, US$ 7 de diferença no net); SLYV PF 3,09 vs 3,04; IWV 20 trades contra 19. Resultado do incremento, sem flatten:

| Ativo | Bloqueados | Trades | Acerto | PF$ | PF_R | avg R | P&L | Saídas |
|---|---|---|---|---|---|---|---|---|
| AVUV | 24 | 20 | 40,0% | **1,33** | 1,31 | +0,206 | **+1.235** | 12 stop, 8 alvo, 3 expirou, 1 overshoot |
| SLYV | 18 | 14 | 35,7% | 0,51 | 0,59 | −0,301 | −1.409 | 9 stop, 5 alvo, 3 expirou, 1 overshoot |
| IWV | 31 | 20 | 25,0% | 0,28 | 0,23 | −0,862 | −3.083 | 15 stop, 5 alvo, 6 expirou, 5 overshoot |
| **Pool** | **73** | **54** | **33,3%** | **0,70** | **0,62** | **−0,321** | **−3.257** | |

IC90 do PF_R do incremento [0,31; 0,60; 1,11]; P(PF_R < 1,0) = 91,4%; P(PF_R < 1,3) = 98,3%. Com flatten (ADR-018): PF$ 0,56, avg R −0,380, −US$ 3.985, P(PF_R < 1,0) = 99,5%. Quebras: 2025 n=25 PF 0,52; 2026 n=29 PF 0,88; long n=30 PF 0,52; short n=24 PF 0,95 (base long PF 3,33, base short 1,03). **Conjunto:** base 70 trades PF$ 1,90 IC90 [1,20; 3,10] → base+incremento 124 trades PF$ **1,16** IC90 [0,80; 1,66]. Permutação da diferença de avg R: −0,631, p = 0,0066 (com a ressalva de seleção acima).

**Por que o filtro existe.** Não há evidência original: `is_tradeable` nasceu no commit `780aedd` (02/07/2026, "mvp ready") junto com o próprio módulo de contexto; nenhum dos 20 ADRs menciona `is_tradeable` ou `NoContext`; a única racionalização escrita é operacional e foi feita para a `pullback-trend-v1`, que é trend-following. Ponto de aplicação único: `risk/mod.rs:158`. **Tornar a política por estratégia não quebra nada** (só `pullback-trend-v1` e `low2-m2s-short-v1` checam `trend_state` por conta própria, e a flag teria default `false`), mas o filtro global está removendo uma população de PF 0,70 — acerta por acidente. O que continua sendo dívida de design real: o edge da v1 não é "fade em dia de range", é "fade em dia de range **dentro de uma tendência de 15m**", e isso precisa estar escrito no doc da v1.

**Teste mínimo que ainda vale (≤ 1 dia):** implementar a flag com os vetos de vol High e fase ≠ Regular explícitos, rodar com e sem em AVUV/SLYV/IWV e diffar por `entry_time`. Esperado a partir desta auditoria: delta ≈ 54 trades, PF ≈ 0,70, avg R ≈ −0,32 (com flatten, ≈ 0,56). Se o motor produzir outra coisa, o erro está no replay e a discrepância precisa ser explicada trade a trade. **Não** procurar hora, par ou direção que salve o delta.

### 6.3 Screener SQL da rodada extra — consolidação e teste de calibração — **SOBREVIVE** (a mais reproduzível; 1 ressalva)

**Veredito do adversário:** re-rodou o `09` em três configurações, as telas 01 e 05 e a correlação; cada número decisivo bate dígito a dígito, e os dois defeitos de código da tela 05 foram confirmados lendo o SQL. Enfraquece uma conclusão: "especificidade 1/1, sensibilidade 0/2" apoia-se em controles que a própria análise escreveu no scratchpad (com simplificações declaradas) — é 1 negativo e 1,5 positivos, amostra de tamanho 1 para especificidade.

**Execução.** As 9 telas rodaram sem erro (01 = 6 s, 02 = 4 s, 03 = 3 s, 04 = 3 s, 05 = 2 s, 06 = 2 s, 07 = 3 s, 08 = 1 s, 09 = 44 s). **Nenhum `docs/reports/screens-<data>.md` existe** — o relatório que o README §7 torna obrigatório por sessão nunca foi escrito.

| Tela / variante | 2025 | 2026 | Mesmo sinal? | Contra o README §8 |
|---|---|---|---|---|
| 01 Q12 PDL-break short, fill honesto, k=1 | avg R −0,034 (n=499, PF 0,90) | +0,034 (n=344, PF 1,11) | não | idêntico |
| 01 Q8 mesma, fill **no nível** | +0,063 (457, PF 1,21) | +0,134 (324, PF 1,50) | sim | idêntico — o "edge" era o gap no gatilho |
| 01 Q12b confirmada por fechamento, k=1,5 | +0,025 (310) | −0,056 (197) | não | **número novo** |
| 02 Q9 fade do squeeze, k=1,5 | PF 0,45 / 0,34 | PF 0,15 / 0,22 | sim (negativo) | idêntico |
| 03 Q10b IB<0,45×ATRd short | k=1: +0,023 / k=1,5: +0,048 | +0,019 / −0,052 | não (a k=1,5) | o par "+0,02 / −0,05" do README **mistura dois alvos** |
| 03 Q10 IB<0,5% absoluto, k=1,5 | 255 fills, ΣR +17,1 → avg +0,067 | — | — | **número novo**, mas com fill no nível |
| 04 Q1 gap-down < −1% | n=239, +0,535% | n=120, +0,466% | sim | **número novo** |
| 04 Q2b fill honesto, k=1 | +0,088 (50) | −0,041 (43) | não | idêntico |
| 05 Q16 double-top, k=1,5 | short −0,206 / long −0,125 | −0,071 / −0,313 | sim (negativo) | PF 0,47–0,86 |
| 06 Q15 toque de PDH | n=587, +0,026%, 77,9% rompidas | n=532, −0,056%, 78,0% | não, e ≈ 0 | idêntico |
| 07 Q14 contexto neutral | dias de range **44,5%** (26.686/59.976) | dias de tendência **41,1%** (3.664/8.924) | — | idêntico; variante Q14b 44,4% vs 41,9% |
| 08 Q7 dia após queda ≥2%, ex-crash | +0,333 (45) | −0,059 (25) | não | o par "+0,55 / −0,09" do README **mistura Q7 k=99 com Q7b k=1** |
| **09 `ctx_piv`, k=1,5, piso 0,20%** | +0,072 (116, PF 1,13) | **−0,261 (80, PF 0,60)** | **não** | sem número anterior |

**O achado principal é de calibração.** Aplicando a regra da Fase 0 (t ≥ 1,5 por data; mesmo sinal 2025/2026; melhor trimestre ≤ 50%; ≥ 40 fills/ano por data; sinal estável a ±20%):

| Candidato | t ≥ 1,5 | mesmo sinal | melhor trim | ≥ 40 datas/ano | Veredito |
|---|---|---|---|---|---|
| 01–08 (todas as 19 regras de livro) | n/d | **não** (ou negativo nos dois anos) | n/d | n/d | **reprovados** |
| **09 controle POSITIVO** (range-fade, 11 ETFs, piso 0,20%) | não (−0,54) | não | n/a | não (51/30) | **REPROVADO** |
| **09 idem, sem piso** (a v1 não tem piso) | **não (−1,80)** | não (−0,096 → −0,695) | n/a | não em 2026 | **REPROVADO** |
| **controle POSITIVO 2** (balance com flatten, k=1) | não (1,22) | **sim** (+0,104 / +0,206) | **sim** (0,39) | não (33/22) | reprovado por potência |
| **controle NEGATIVO** (pullback-trend, sem piso) | não (**−3,37**) | sim (negativo) | n/a | 70/61 | **reprovado — correto** |

Isto é: **o instrumento reprova a melhor estratégia viva do projeto**. Duas causas isoláveis, ambas medidas: (i) a **agregação sobre 11 ETFs** — restringindo `ctx_piv` aos 3 pares do gate A, t = **+1,79**, 2025 avg R +0,518 com PF 2,53, melhor trimestre 47%, e por símbolo AVUV PF 2,34 / SLYV 1,25 / IWV 1,61 contra o gate A OOS 1,95 / 2,95 / 1,31; os pares onde a v1 nunca foi aprovada (IJS 0,30, VB 0,43, IWM 0,52, VBR 0,57) puxam tudo para baixo; (ii) o **piso de stop de 0,20%**, que remove 120 dos 333 fills (36%) e **inverte** o sinal de 2025 (de −0,096 sem piso para +0,072 com piso) — um parâmetro livre não calibrado que decide o veredito, e que a v1 não tem. No controle negativo o piso remove 347 de 551 fills (63%).

**Dois filtros candidatos que o screener entrega para a v2 da fade** (mais concretos que o `allow_neutral`): a faixa das **12h ET** é a pior de longe (27 fills, avg R −0,635, PF 0,27) mesmo com o veto de meio-dia modelado, contra 11h (43 fills, +0,271, PF 1,60); e dias com ≥ 2 sinais no mesmo símbolo dão avg R −0,318 / PF 0,58 contra −0,043 / 0,93 nos de sinal único. Custo de replicar a ADR-015: 8 cancelados em 204 fills (3,3% em 2025, 4,8% em 2026), que teriam rendido +0,815 / +0,352 R.

**Defeitos concretos das telas** (arquivo:linha):

| # | Onde | Problema | Impacto medido |
|---|---|---|---|
| 1 | `02:61` e `05:61` | trade que não bate stop nem alvo recebe **0 R** em vez do fechamento do dia | 22,3% / 27,1% da amostra da 05 a k=1,5 (30,1% / 34,4% a k=2); 7,8–11,1% da 02 |
| 2 | `05:53-56` | sem deduplicação por (símbolo, dia) | 2.311 fills em 1.460 dias-símbolo (1,58/dia) em 2025; 1.591 em 1.056 em 2026 |
| 3 | `05:69-72` | `group by … symbol` mas seleciona `null, null` | o bloco "por símbolo" sai com 11 linhas anônimas |
| 4 | `02:10,23` vs `:57-62` | cabeçalho afirma "fill honesto"; o código não modela preço de fill | veredito não muda (PF 0,15–0,52), mas não é instância do modelo de referência |
| 5 | `03:17` vs `:66-78` | cabeçalho afirma fill honesto no Q10; o Q10 enche exatamente no nível | o único número novo do Q10 é otimista por construção e dominado por IWV (96 de 255 fills, avg −0,070) |
| 6 | `README` §2 | não registra que 03 e 04 **não filtram risco** nem o terceiro piso (`_lib.sql:36`) | três pisos coexistem: 0,30% (01), 0,15% (05), 0,20% (`_lib`), mais duas telas sem piso |
| 7 | `02:63-70` | `avg(r_net)` e `wr` publicados sob os nomes `fwd8_dir_pct`/`fwd16_dir_pct` | um leitor lê "−0,421" como retorno em % |
| 8–9 | README §8 (telas 03 e 08) | pares de números que comparam **variantes diferentes** | comparação inválida entre anos |
| 10 | `screens-raw-extra/09-base.txt` | não é reproduzível com o `.sql` atual (5 tags contra 7; `ext_m20` 714 vs 381) | só `09-nofloor.txt` bate |
| 11 | README §4 item 5 | "11 ETFs a ~0,97 de correlação" | medido: **mediana 0,940** dos 55 pares (mín 0,816, máx 0,997, média 0,928) |

**Limites do instrumento:** trade único por evento, sem gestão de posição (a família inteira do estudo de saídas é invisível); sem dinheiro (nem sizing, nem cap de notional, nem 3 posições, nem liquidez — `fills` não são trades); viés de sobrevivência e correlação (os 11 ETFs são todos small-cap/value; a agregação por data reduz 196 fills para 108 datas, e nem essas são independentes); um regime só; e os controles rodados são modelos, não o código Rust.

**Sobreposição entre famílias, medida:** range-fade 410 dias-símbolo, balance 371, pullback 743; interseções 57 (rf∩ba, 14%), 106 (rf∩pb, 26%), 100 (ba∩pb, 27%); datas distintas 188 / 119 / 174 de 385. **O risco de correlação está no dia, não no gatilho.** Entre os candidatos de livro, a única sobreposição é 03∩01: 795 dias de IB estreito, 550 de rompimento de PDL, **135 em comum**.

**Contagem desta rodada:** 19 regras (telas 01–08) + 7 tags (09) + 3 dos controles + 2 recortes de universo = **31 variantes**, sem contar os 4 alvos por variante.

**O que precisa acontecer antes de o screener virar gate:** (1) medir **por par**, não sobre os 11 ETFs — a agregação sobre símbolos correlacionados é o que reprova o controle positivo; (2) baixar o limiar de frequência (40 datas/ano por par não existe neste projeto: `ctx_piv` em AVUV+SLYV tem 30 datas em 18 meses); (3) calibrar ou remover o piso de stop; (4) reescrever 01–08 sobre `_lib`/`_eval`, sem o que a regra §5 é inaplicável a elas; (5) corrigir os defeitos acima.

### 6.4 Completude: o que falta para o dono decidir — **SOBREVIVE PARCIALMENTE** (2 das 5 lacunas com atribuição causal errada)

**Veredito do adversário:** todos os números derivados dos JSONs são exatos (reproduzidos com script próprio, sem um único desvio). Caem duas atribuições causais, ambas envolvendo comparação com documentos do repo: (a) a queda "de PF 1,55 para PF_R 0,94" é **troca de métrica**, não efeito de comissão (nos mesmos trades sem flatten, PF$ 1,92 e PF_R 1,32) — a conclusão de que a balance reprova o PF_R ≥ 1,2 sobrevive, mas não pela razão dada; (b) a "quarta medição" de liquidez usa janela que na verdade termina em ~06/08/2026 mas foi rotulada "mar–ago/2026", e **a ADR-020 está certa para a janela que ela própria declara** (medido: SLYV 305k / IJS 370k contra os 290k/370k dela). Enfraquecida: "4 a 15 sessões esparsas" descreve o mecanismo errado (as 26 barras estão lá; o que colapsa é o range intrabar e o volume). Achado novo do adversário que corre **contra** o plano: os números de liquidez do `plano:97` (SLYV 366k p10 114k / IJS 527k p10 164k) não reproduzem em nenhuma das seis janelas testadas com a definição declarada, e com as medianas medidas o bot usa **78% e 64%** de uma barra mediana, não 65% e 45% — o plano subestima o problema de impacto que ele mesmo levanta.

**Análises prometidas ou implicadas e não feitas** (as fechadas nesta auditoria estão marcadas):

| # | Lacuna | Onde é prometida | Fechada aqui? | Se der A | Se der B |
|---|---|---|---|---|---|
| A1 | **Potência do holdout travado**. Medido: 6 m = **33 / 16 / 18** trades em 19/14/16 datas; 4 m = 28/10/11; SE(avg R) 0,26–0,38 | ADR-019 §7; plano §3.4; as 3 specs | **sim** | — | o critério é **inexequível como escrito**; ou vira "só mata", ou o holdout passa a ser o paper forward |
| A2 | **Reconciliação da comissão**. Medido: delta −668 / −528 / −282 US$, ≈ US$ 7/trade | plano §5.6; ADR-019 §4 | **sim** | ≤ 5% mantém | é 10% e 11,6% do net: publicar o gate A com flatten antes de corrigir cria um baseline oficial já errado |
| A3 | **PF_R com flatten das três v1** | ADR-019 §7 propõe PF_R ≥ 1,2 e nunca o calcula | **sim** (está no estudo de saídas, que ninguém cita) | ≥ 1,2 segue | balance **0,94** → reprova o segundo critério, não só o avg R |
| A4 | **Tempo até o gate B de cada v2**: balance-v2 no limiar honesto (1,5) rende ~4,6 trades/ano/símbolo → 3 pares ≈ 14/ano → **~17 meses** até 20 trades por par | balance-v2 §13/§14; roadmap §3.4 faz a conta só para as v1 | sim (aritmética) | limiar 1,0 ≈ 8 meses | o item #11 fica sem caminho de validação e a escolha 1,5 vs 1,0 deixa de ser estatística |
| A5 | **Contaminação do banco dev pelo feed** | o plano trata só produção | **sim** | irrelevante | o holdout e o re-run do gate A incluem sessões erradas — reingerir vira pré-requisito da ADR-018 |
| A6 | **Tercis de stop nas outras duas estratégias** | plano §2.2, §6.9, §10.6 | **sim** | regra transversal justificada | sinal invertido em 2 de 3 (ver bloco C2) |
| A7 | **Custo do piso de stop nas aprovadas** — o §6.9 manda medir e não mede | plano §6.9 | **sim** | — | o piso só pode ser parâmetro da v2 |
| A8 | **PF do delta Neutral** | plano §6.1, §9; roadmap #10 | **sim** (aproximado pelo screener: 42 fills, −9,1R nos pares vivos; −19,3R no pool de 11) | gasta os 5–8 dias | rodar antes do plumbing; o item #10 morre |
| A9 | **Controle positivo do screener** | README §4 diz que "nunca rodou" | **sim** | Fase 0 entra no framework | o controle **reprova**: a Fase 0 não pode ser aprovada agora |
| A10 | **Mediana da barra que calibra o cap** | plano §2.4; ADR-020 §7 "a medir" | parcial | convergem | quatro medições, três valores publicados: ver §5 |
| A11 | Slippage real do flatten | ADR-018; balance-v2 §14 passo 6 (**kill**) | não | ≤ 4 bp segue | > 4 bp mata a balance-v2 antes de qualquer código |
| A12 | Rejeições por contexto em produção | fade-v2 §6.1 item 5 | não | confirma o backtest | o número que sustenta #10 é artefato |
| A13 | Bloqueio mútuo por símbolo em IWN | plano §6.5 ("medir antes") | não | troca segue | a troca entrega menos amostra do que promete |
| A14 | Largura mediana de 78 barras nos setoriais | plano §6.5 | não | braço balance entra | o screening tem metade dos braços |
| A15 | Horas, energia, equity/moeda-base | roadmap §1.4, Q6 | não | — | se a conta for CAD, todo US$ está 1,37× errado |

**Decisões propostas com evidência fraca** (além das já tratadas no §3): `#16` — entrar `balance-area-breakout-v1` em **IWN "imediato"** no M1: o par foi escolhido depois de ver a tabela dos 8 símbolos, sem walk-forward com flatten, para uma estratégia que o próprio §2.5 declara "bloqueada para dinheiro real" — é a seleção post-hoc que o plano condena em outros itens; `#19` — o roadmap ainda atribui EV +1,0k e Tier B a um item que o plano põe na Onda C; `§5.7` — adotar a Fase 0 com o controle positivo reprovando; `§2.5` — aplicar o "±30% do ADR-010" (critério da estratégia) a um replay de portfólio que ainda não existe no motor.

**Riscos não cobertos por nenhum documento:** (1) **não há monitor de qualidade de dado** — o feed degradou em 07/08 e só foi percebido em 06/09 (30 dias), e o plano propõe um *diagnóstico*, não um alarme permanente; (2) fonte única de dados (IBKR), sem segunda fonte para detectar o mesmo modo de falha; (3) risco de pessoa única — todo o calendário depende do dono e não há contingência; (4) **comportamento em bear / alta volatilidade**: `is_tradeable` bloqueia vol `High`, então num crash o bot simplesmente para, e ninguém mediu quantos dias seriam bloqueados nem o efeito sobre a amostra do gate B; (5) o que acontece com o edge se o fator small-cap value entrar em tendência longa (a fade morre, o breakout inverte); (6) custo de oportunidade: 46–71 dias-pessoa para um EV declarado em paper, sobre um baseline de dinheiro real de 2,6–4,5k/ano — a relação horas × retorno aparece no roadmap §3.4 mas não vira critério de parada; (7) nada sobre tributação/natureza da conta.

**Opinião apresentada como fato:** "não é acidente de sizing: é **stop estreito perde**" (vira regra, item #13 e critério de triagem — refutado no bloco C2); "o detector coincide com Neutral por construção" (o próprio anexo diz que é falso); "o que mudou em 07/08 foi TWS → IB Gateway" (correlação temporal apresentada como causa, enquanto o §5.8 põe a causa como item a diagnosticar); "a maior alavanca … a hipótese de maior potencial com menor custo" (o PF era declarado desconhecido na frase seguinte; medido, é negativo); "Onda A ~2–3 semanas"; "≈ +US$ 9,6k/ano" (probabilidades declaradas subjetivas no roadmap §7, citadas sem a ressalva no plano); "o único OOS verdadeiro é o paper forward" (correto como princípio, mas o mesmo conjunto usa "holdout travado" como se fosse OOS, e a balance-v2 admite que já o leu).

**Perguntas ao dono — problemas de forma:** sobreposição (plano §10.1 ≈ roadmap Q10; §10.3 ≈ Q6+Q8; §10.7 é ponteiro); tarefas vendidas como decisões (Q6 e Q8 são execuções de 15 min); Q2 (tolerância a drawdown) pede um número ancorado em DD de 3,8% / 1,3% que a ADR-020 diz que fica **3× mais leniente** com `capital_fraction` 1/3 se a base não for fracionada junto — os dois números oferecidos não são comparáveis; e o §4 fixa 18/12/2026 e 31/03/2027 **antes** de Q4 perguntar o prazo do dono. **Dez decisões do dono estão embutidas em texto técnico e fora das duas listas:** D1 `capital_fraction` 1 ou 1/3 (corta o P&L ~60%); D2 `max_daily_loss_pct` sobre a conta ou sobre a fatia (3×); D3 limiar da balance-v2 1,5 ou 1,0; D4 reescrever ou não linhas de produção na migração 0004; D5 tocar os `config.rs` das v1 com `deny_unknown_fields`; D6 se o split de `NoContext` exige ADR própria; D7 nota ou bump de versão no hotfix; D8 aceitar gate B **operacional** se o feed não for resolvido; D9 piso de stop como ADR global ou parâmetro de v2; D10 abrir uma 4ª instância da balance-v1 enquanto ela reprova o gate A. **Faltam três perguntas:** o dono aceita que o bot **pare de operar** enquanto o feed não for corrigido (hoje ele opera com barras erradas)? Qual o plano se as horas caírem a zero por um mês? Os documentos devem ser commitados como estão, com a ADR-018 substituindo formalmente o gate A de 04/09, antes de qualquer código?

**Lacunas de forma:** o estudo de saídas é **órfão** (`grep -rn "estudo-politicas|politicas-de-saida" docs/ README.md` = zero fora dele mesmo) e contém o único desenho pareado da pesquisa e o PF_R com flatten que o gate proposto exige; ele também contradiz `balance-v2:8` ("as ablações de saída não foram re-simuladas") — foram, e a política D (stop além da barra) dá PF_R 1,53 contra 0,94 do baseline nos pares vivos (t 1,7; t 2,4 no pool), reprovada só pelo critério estrito de t > 2 nos dois anos. O **roadmap não está no README nem no HANDOFF** — é o documento que contém os critérios de encerramento e as 11 perguntas. E `sql/screens/README.md` §6 diz "cada arquivo é autocontido"; os screens 09+ falham assim (`ERROR: relation "bs" does not exist`).

---

## 7. O que ainda falta (pendências abertas, priorizadas)

| Ordem | Ação | Custo | Por que nesta posição |
|---|---|---|---|
| 1 | **Reingerir 08/08→02/09 dos 7 pares vivos pelo PC/TWS** e ligar um check diário de qualidade de barra (volume e range contra a mediana de 60 pregões) | 2 h + 1 h | sem isso, o baseline oficial que substitui o gate A de 04/09 nasce sobre 4–15 sessões com ¼ do range real, e o stop das três estratégias sai do range da barra |
| 2 | **Adotar a comissão por ação no mesmo commit da ADR-018** e publicar a reconciliação com o estudo de saídas | 4 h | a régua muda 10–12% do net; hoje existem dois números oficiais para o mesmo baseline e nenhum documento admite |
| 3 | **Rodar o A/B do delta Neutral no motor** (ou o `09-*.sql` calibrado) antes de qualquer plumbing do `RiskConfig` | 1 h (SQL) / 1 dia (motor) | decide o item de maior EV da Onda B e a adoção da Fase 0 sem gastar 5–8 dias. Esperado: delta ≈ 54 trades, PF ≈ 0,70 |
| 4 | **Corrigir o critério de holdout na ADR-019** com o n medido (33/16/18) e escolher entre "só mata" e "holdout = paper forward" | 2 h | senão o M2 aprova um critério inexequível e ele será afrouxado em silêncio |
| 5 | **A9 + teste operacional de short** (3 sell stops fora de setup) | 1 dia | bloqueia #11 (tem shorts), #19 (100% short) e a leitura do 10/10/2025, que carrega 64% do P&L da balance-v2 |
| 6 | **Publicar os tercis de stop das 3 estratégias** e rebaixar o #13 de regra global para parâmetro de v2 | 2 h | a regra, como está, apagaria metade da amostra da única estratégia que passa o gate |
| 7 | **Fixar uma definição única da mediana de liquidez** (janela, horário, período, fonte, N) e recalcular o cap da ADR-020 | 2 h | quatro medições e três valores publicados; o cap depende disso e a decisão D1 também |
| 8 | **Escrever o caminho de dados do cap de liquidez** (`median_bar_notional` no `MarketContext`, não no `RiskManager`) e trocar `f64` por `Decimal` em `max_notional_usd` | 4 h | é a única peça da ADR-020 que não é plumbing e hoje não é implementável |
| 9 | **Reescrever o item 2 da ADR-018** (decidir a reclassificação, não oferecer duas opções), acrescentar o braço `"end_of_day"` na leitura com teste de round-trip, e tratar meio pregão nos dois lados (backtest e flatten do live) | 4 h | uma ADR é registro de decisão; e sem o braço, todo trade de flatten volta do banco como alvo |
| 10 | **Corrigir a migração 0005** (`NULLS NOT DISTINCT` ou `label NOT NULL DEFAULT ''`), declarar a chave e publicar o dedupe medido (109 grupos / 279 linhas) | 2 h | 82% dos runs continuariam duplicáveis |
| 11 | **Estender `--label/--set/--holdout-from/--no-flatten/experimental` ao comando `backtest`** e gravar toda divergência do `[risk]` no jsonb `metrics` | 4 h | metade do protocolo das v2 não roda, e o override por env é invisível ao `config_hash` |
| 12 | **Abrir a ADR-021 "Política de contexto por estratégia"** em vez de deixar a escolha "se registra ADR" para o dono | 2 h | `AGENTS.md:75`; e a ADR-019 não cobre o assunto |
| 13 | **Declarar o tempo até o gate B de cada v2** no pré-registro (balance-v2 no limiar honesto ≈ 17 meses) e o pool de 8 símbolos por extenso | 1 h | reclassifica o item #11 antes de qualquer código |
| 14 | **Declarar que não há holdout travado** para as três estratégias atuais (a janela foi lida e está impressa nas specs) e que o `--holdout-from` só vale para dado ingerido depois desta data | 1 h | um holdout já visto não falsifica nada |
| 15 | **Refazer o cronograma** a 2,5 dias-pessoa/semana (ou declarar 40 h/semana), com a conta "dias-pessoa por marco × semanas disponíveis" visível | 2 h | M0→M3 tem 25 dias-pessoa contra 32–49 de entregas |
| 16 | **Indexar o roadmap e o estudo de saídas** no README e no HANDOFF, e incorporar o PF_R com flatten ao plano §2.3 e à balance-v2 §8 | 1 h | o dono não tem como achar os critérios de encerramento nem o único desenho pareado |
| 17 | **Somar às listas de decisão as 10 decisões embutidas (D1–D10)** e separar tarefas (Q6, Q8) de decisões | 2 h | metade das escolhas que mudam o resultado está escondida em ADRs e specs |
| 18 | **Calibrar o screener** antes de levá-lo ao dono como Fase 0: medir por par (não sobre 11 ETFs correlacionados), rever o limiar de 40 datas/ano, calibrar ou remover o piso de stop, corrigir os 11 defeitos das telas e criar `docs/reports/screens-2026-09-07.md` com as 31 variantes | 1 dia | o controle positivo reprova; um filtro que reprova a melhor estratégia viva não pode reprovar candidatos |
| 19 | **Registrar as divergências live×backtest ausentes** na tabela §2.6 e no §Contexto da ADR-018 (flatten só de posição rastreada; barras abandonadas; warmup 600; exposição ignorando ordens pendentes; guardas de overshoot assimétricas) e somar as ordens abertas ao `exposure_limit_hit` | 4 h | são as maiores fontes de divergência depois do flatten e não estão onde o leitor procura |
| 20 | **Adiar IWN-balance** para depois do gate A com flatten em IWN e do replay de portfólio; **cortar #14 e #15 da Onda B** (o próprio plano dá p ≈ 0,21 e "marginal") | 0 h (decisão) | coerência com os princípios §3.3, §3.6 e §3.8; e gastar N em hipóteses declaradas ruído contamina o contador que a ADR-019 existe para proteger |

---

## 8. Nota de método

**Quem rodou o quê.** Seis verificadores independentes, cada um com uma dimensão e proibidos de escrever no repositório: (1) **motor Rust** — leu `trader-backtest` inteiro, `trader-core` (context, risk, execution, session, `range_extreme_fade_v1` e amostragem das outras 8), `trader-adapters` (simulated e ibkr), `trader-cli` (paper, backtest, walkforward, risk_config) e `trader-infra`, para checar as 8 afirmações A–H e as citações `arquivo:linha`; (2) **números empíricos** — reproduziu 35 afirmações com SQL no banco dev e com scripts próprios sobre os JSONs do binário de 06/09, incluindo a reconstrução do cenário B do zero; (3) **consistência cruzada** — cruzou números, datas, referências de seção, nomes de arquivo e vereditos entre os 10 documentos, resolvendo cada divergência contra evidência primária; (4) **conformidade** — conferiu as 3 specs v2 e as 3 ADRs contra AGENTS.md, o framework, o ADR-010 e as ADRs 013–017, e testou se o que está escrito **roda**; (5) **aritmética** — refez baseline, IC95, EV, esforço, limiares e coerência PF/WR/avgR/n; (6) **referências** — resolveu 339 citações `arquivo:linha` (299 únicas por documento) contra a árvore do repo, abriu ~100 para comparar com a afirmação, verificou 126 caminhos e 9 páginas web salvas. Quatro analistas produziram as medições que faltavam (setups novos, alavanca Neutral, screener, completude). Dois adversários revisaram 128 achados e 32 afirmações das análises, **reexecutando** os artefatos decisivos (screener em três configurações, telas 01/05, os scripts de replay, ~24 consultas próprias ao banco).

**Limites desta auditoria.** (a) Ninguém rodou `cargo build/test/run`: "compila" e "o código faz X" são inferência de leitura, não medição. (b) Nada foi escrito no repositório nem no banco — em particular, o A/B do `allow_neutral_context` no motor **não** foi executado (exigiria alterar código e gravaria em `backtest_runs`), então o delta medido é re-simulação validada contra o backtest oficial, não o motor. (c) Só o banco **dev** estava acessível: tudo que depende de produção (fills e comissão reais, latência, `signals.rejection_reason`, slippage do flatten, ritmo do gate B) continua não verificável — o dev tem 3 trades e 26 fills de 04/08. (d) O log de debug que produziu os 48/36/62 não foi preservado; a correção para 24/18/31 é reconstrução por duas rotas que fecham exatamente, não leitura direta. (e) As medições das análises novas são **todas in-sample**: nenhuma produziu um único número fora de amostra, e as ~500 leituras da análise de setups mais as 31 variantes do screener entram em `n_trials` sem deflação — com o maior t de toda a busca em 2,00, a leitura honesta é que a rodada não encontrou nada, e que os defeitos de instrumento que ela encontrou valem mais que qualquer candidato. (f) Os controles do screener (balance positivo, pullback negativo) foram escritos no scratchpad pelo próprio analista, com simplificações declaradas: a especificidade de "1/1" é amostra de tamanho 1. (g) A contaminação do feed atravessa **todas** as quatro análises no trecho pós-07/08/2026; foi quantificada só no caso mais grave (o candidato de setup novo, que perde 36% do resultado). (h) Não foram auditados `docs/PRD.md`, `ARCHITECTURE.md`, `DATA-MODEL.md`, `SECURITY.md`, `trader-web`, o pipeline de ingest, nem ~80 das 106 citações do relatório de pesquisa (essas ficaram só na verificação automática de existência e intervalo de linhas).