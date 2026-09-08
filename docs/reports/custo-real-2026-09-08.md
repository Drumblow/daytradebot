# Custo real de execução — a comissão era 10× menor, e o segundo custo é maior que ela — 08/09/2026

**Por que existe:** o §5.6 do plano de lucratividade manda trocar a comissão do
simulador — US$ 0,35 fixos por perna — pela tabela real da IBKR, que cobra
**por ação**, e cobrar alguma coisa no fill do alvo, que até então enchia de
graça.

**O veredito:** com o custo certo, **nenhum recorte passa o gate**. Nem por
par, nem por estratégia, nem no portfólio dos oito juntos — que era o único
que passava tudo em `estatistica-gate-a-2026-09-08.md`, e agora reprova no
avg R (0,107 contra o piso de 0,15).

**Reprodução:**

```bash
cargo build --release -p trader-cli
./trader-research/regenera-runs.ps1          # custo real  -> out/adr018/wf56_*.json
./trader-research/regenera-runs.ps1 -Legacy  # custo antigo -> out/adr018/wf19_*.json
cd trader-research
uv run trader-research ../out/adr018/wf56_*.json --por par
```

O `--legacy-cost` reproduz os números publicados até 08/09 **dígito a dígito**
(balance·IJS: 23 trades, PF 2,54, avg R 0,383, net US$ 3.384,2038926054444…) —
é o teste de paridade que o §5.6 pede. O cenário intermediário (comissão real,
desconto zero) sai com `--limit-haircut-bps 0`.

---

## 0. Correção: a primeira versão deste relatório errou o tamanho do efeito

A versão de algumas horas atrás dizia **"1/18 da comissão"** e **"posições de
1.000 a 2.500 ações"**, e atribuía a virada dos vereditos à comissão. Uma
rodada de refutação derrubou as três coisas, e eu confirmei medindo:

| afirmação anterior | medido nos 214 trades OOS |
|---|---|
| comissão era 1/18 da real | **1/9,94** (US$ 149,80 → US$ 1.489,36) |
| posições de 1.000 a 2.500 ações | **234 a 1.311**, mediana 619; **zero** trades acima de 2.500 |
| US$ 0,70 → US$ 10 a 25 por trade | US$ 0,70 → **US$ 2,34 a 13,11**, mediana US$ 6,19 |
| a virada vem da comissão | comissão explica **42,9%** da queda de P&L; o desconto no alvo, **56,4%** |

O 18× saiu de **um único trade** de 1.262 ações que eu vi num JSON e
generalizei. O teste que deveria ter guardado o número usava essas mesmas 1.262
ações fixas e afirmava "> 17×" — passava sem tocar em dado nenhum. Ele foi
reescrito sobre a faixa medida (`a_diferenca_entre_os_dois_modelos_na_amostra_REAL`).

Isso não muda o veredito de manchete, e a §3 mostra por quê. Muda um dos quatro
vereditos por par, e muda a explicação de todos eles.

## 1. O que mudou no simulador

| | antes | agora |
|---|---|---|
| comissão | US$ 0,35 fixos por perna | **US$ 0,005 por ação**, mínimo US$ 1,00/ordem, teto 1% do valor (IBKR US Fixed) |
| fill do alvo (ordem limite) | grátis — bastava o candle tocar o nível | **2 bp de desconto** |
| comissão por trade, na amostra | US$ 0,70 | US$ 2,34 a **13,11** (mediana 6,19) |

Nenhum dos dois limites da tabela morde na amostra — verificado, não suposto:
**zero acionamentos em 428 pernas**. Mas a margem é menor do que parece: a
ordem mínima real tem **234 ações**, que a US$ 0,005 dá US$ 1,17 contra o piso
de US$ 1,00 — 17% acima, não uma ordem de grandeza. O cap de liquidez do
ADR-020 vai reduzir tamanho e pode fazer o piso morder.

**Sobre o desconto no alvo, duas ressalvas.** A primeira é de mecanismo: o
§5.6 o chama de "spread cobrado no fill do alvo", e não é isso — uma ordem
limite parada no book é o lado **passivo** e não paga spread. O que o desconto
corrige é que **tocar não é encher**: o high do candle no seu preço quase
sempre significa que poucos lotes negociaram ali. Por isso o campo se chama
`limit_fill_haircut_pct`.

A segunda é de peso: **este parâmetro foi escolhido a mão, não calibrado**, e
responde por mais da metade do efeito medido. Ele agora é ajustável
(`--limit-haircut-bps`) exatamente para que essa dependência seja mensurável
em vez de assumida.

## 2. O efeito, decomposto em duas etapas

Mesmos trades, mesma janela, mesmas 6 janelas OOS. A coluna do meio isola a
comissão; a da direita acrescenta o desconto no alvo.

| estratégia · par | antes (US$ 0,35 fixo) | só comissão real | + desconto de 2 bp |
|---|---|---|---|
| balance · IJS | PF 2,54 · avgR 0,384 · US$ 3.384 | 2,41 · 0,347 · 3.200 | **2,33 · 0,305 · 2.996** |
| balance · VBR | 1,52 · −0,014 · 1.851 | 1,46 · −0,033 · 1.717 | **1,41 · −0,068 · 1.516** |
| balance · AVUV | 1,43 · −0,174 · 2.250 | 1,36 · −0,212 · 1.941 | **1,33 · −0,226 · 1.760** |
| openrev · IWM | 1,72 · 0,451 · 3.773 | 1,69 · 0,439 · 3.659 | **1,64 · 0,406 · 3.347** |
| openrev · IWN | 1,57 · 0,284 · 2.337 | 1,53 · 0,267 · 2.210 | **1,48 · 0,240 · 2.004** |
| fade · AVUV | 1,48 · 0,159 · 1.262 | 1,36 · 0,116 · 1.024 | **1,28 · 0,073 · 783** |
| fade · SLYV | 2,64 · 0,424 · 2.417 | 2,43 · 0,377 · 2.208 | **2,29 · 0,320 · 1.982** |
| fade · IWV | 1,32 · 0,043 · 422 | 1,28 · 0,026 · 387 | **1,14 · −0,076 · 187** |

Os dois custos, medidos por par:

| par | comissão (antes → depois) | desconto no alvo |
|---|---|---|
| balance · IJS | 16 → 198 | 202 |
| balance · VBR | 24 → 158 | 202 |
| balance · AVUV | 25 → 332 | 183 |
| openrev · IWM | 22 → 134 | 308 |
| openrev · IWN | 18 → 145 | 203 |
| fade · AVUV | 18 → 255 | 240 |
| fade · SLYV | 14 → 221 | 222 |
| fade · IWV | 13 → 47 | 200 |
| **total** | **150 → 1.489** | **1.760** |

**O desconto no alvo é maior que toda a comissão nova.** Somando as duas
etapas, a queda de P&L do conjunto é US$ 3.122, dos quais a comissão explica
US$ 1.340 (42,9%) e o desconto, US$ 1.760 (56,4%).

**Quatro vereditos viram — mas não pelo mesmo motivo:**

1. **`fade` em AVUV** cai de PF 1,48 para 1,28 e passa a reprovar o profit
   factor. **Este veredito é do desconto, não da comissão**: só com a comissão
   real o PF fica em 1,36, acima do piso de 1,30. A 1 bp em vez de 2, volta a
   passar. É o veredito mais frágil do conjunto.
2. **`fade` em IWV** cai para PF 1,14 e reprova o PF. Aqui a comissão já
   basta (1,28 sem desconto); o avg R, esse sim, só fica negativo com o
   desconto (0,026 → −0,076).
3. **`balance` em VBR e AVUV** afundam em unidades de risco: PF_R 0,88 e 0,68.
   Efeito majoritariamente da comissão.
4. **`fade` em SLYV deixa de passar o critério do IC95** (1,003 → 0,867) — era
   a única combinação viva que passava.

A explicação de IWV: ele paga a **menor** comissão do conjunto (US$ 47) porque
é o ETF mais caro dos oito e a mesma fatia de capital compra menos ações. Mesmo
assim é o que mais se degrada em termos relativos — perde 56% do net — porque
partia da menor base (US$ 422 em 18 meses). O desconto no alvo, ao contrário
da comissão, é praticamente uniforme entre os pares (~US$ 20 por saída no
alvo): ele não distingue IWV.

## 3. O efeito no gate

Critérios do ADR-010 (seis) mais os três do ADR-019 §7 que dão para medir num
recorte agregado.

| recorte | n | PF em $ | PF em R | avg R | IC95 inf. (L=5) | 2 melh. meses | reprova em |
|---|---|---|---|---|---|---|---|
| balance (3 pares) | 92 | 1,56 | **0,94** | **−0,035** | 0,703 | 102% | avg R, IC95, PF_R, concentração |
| openrev (2 pares) | 58 | 1,57 | 1,70 | 0,332 | 0,794 | 101% | IC95, concentração |
| fade (3 pares) | 64 | 1,52 | 1,25 | 0,108 | 0,827 | 81% | avg R, IC95, concentração |
| portfólio (8 pares) | 214 | 1,55 | 1,21 | **0,107** | 1,038 | 55% | **avg R** |

**Nada passa.** Os 8 pares isolados reprovam todos; as 3 estratégias — a
unidade que o ADR-019 §7 propõe — reprovam todas; e o portfólio, único recorte
que passava tudo com a régua anterior, reprova no avg R.

**Esse veredito não depende do parâmetro escolhido a mão.** Com o desconto
zerado e só a comissão real, o avg R do portfólio fica em **0,147** — ainda
abaixo de 0,15 — e o IC95 em 1,089. A tabela por cenário:

| recorte | antes | só comissão real | + desconto de 2 bp |
|---|---|---|---|
| balance (pool) | avgR 0,025 · IC 0,768 | −0,006 · 0,731 | −0,035 · 0,703 |
| openrev (pool) | 0,376 · 0,842 | 0,362 · 0,826 | 0,332 · 0,794 |
| fade (pool) | 0,209 · 0,959 | 0,173 · 0,896 | 0,108 · 0,827 |
| **portfólio** | **0,175 · 1,135** | **0,147 · 1,089** | **0,107 · 1,038** |

A `balance-area-breakout-v1` agregada fica com **PF em R abaixo de 1** (0,94) e
**avg R negativo**: em unidades de risco, a estratégia inteira perde dinheiro
depois de pagar a corretora. O PF em dólares acima de 1 continua vindo da
correlação entre tamanho e resultado (0,59 em AVUV).

## 4. P&L por ano civil

| recorte | 2025 | 2026 |
|---|---|---|
| balance (3 pares) | +6.992 | **−721** |
| openrev (2 pares) | +2.033 | +3.318 |
| fade (3 pares) | +3.130 | **−179** |
| portfólio | +12.155 | +2.419 |

Duas das três estratégias estão **negativas em 2026** com o custo real. A
`opening-reversal-v1` — reprovada no gate A de 04/09 — é a única que cresce.

## 5. `cost_total` mostra menos da metade do custo

O campo `cost_total` do `metrics.rs`, que o `walkforward` imprimia sob o rótulo
"custo", soma **só o caixa pago à corretora**: comissão + taxas. O desconto no
alvo fica embutido no preço de saída e não entra em métrica nenhuma — nem o
slippage, que já era assim.

Na amostra: comissão US$ 1.489 contra desconto US$ 1.760. Quem lia "custo:
198,10" no IJS via 50% do que o motor cobrou; em IWV, 19%.

Corrigido pelo lado do rótulo: o `walkforward` agora imprime **"comissão"**, e
o doc do campo diz o que ele não contém. Separar o desconto numa métrica
própria exige o simulador registrá-lo no momento do fill — **item aberto**.

## 6. O resto do §5.6

| item | estado |
|---|---|
| Comissão por ação da IBKR no simulador | ✅ feito, com teste de paridade |
| Desconto no fill do alvo limite | ✅ feito, e ajustável (`--limit-haircut-bps`); calibração por ativo continua pendente |
| `tick_size` gravado a partir de f64 | ✅ corrigido (`Decimal::new(1, 2)`) e os 14 ativos do banco dev limpos (`sql/maintenance/0005`) |
| Divergência `assets.tick_size` × TOML em log | ✅ o `paper` avisa na largada |
| Runs sem `label` | ✅ causa corrigida (o `backtest` ganhou `--label` e nunca mais grava NULL) e os 586 do banco dev rotulados por data (`sql/maintenance/0006`) |
| ⛔ Dedupe de `backtest_runs` | revogado em 07/09 — não são cópias |
| Métrica separada para o desconto no alvo | ⏳ aberto (§5 acima) |
| `CommissionReport` que chega em poll posterior | ⚠️ **parcial** — ver abaixo |
| Reingerir IJR, MDY, QQQ, SCHA, SPY, VB | ⏳ depende do servidor |

### O que ficou parcial, e por quê

O `ibkr/broker.rs` casa o `CommissionReport` com o fill **dentro do mesmo
lote**. Se o stream terminar (ou o drain expirar) entre o `ExecutionData` e o
`CommissionReport`, o fill sai com comissão **zero** e o relatório, chegando
num poll seguinte, é descartado porque a execução já está em `seen`. O trade
gravado no banco fica com `commissions = 0` e `net_pnl` otimista — e é esse
número que o gate B compara com um backtest que agora cobra a tabela real.

Fechar o buraco exige **adiar a emissão do fill** até a comissão chegar. Isso
muda o comportamento do caminho ao vivo, e o plano é explícito (§5.9) sobre
esse bloco precisar de smoke test de 2 pregões numa instância — que não dá
para fazer daqui. O que entrou foi o meio-termo: o adapter agora **conta e
loga** as execuções que ficaram sem `CommissionReport`, com os `exec_id`. O
buraco passa a ser mensurável em produção antes de alguém decidir mexer no
protocolo.

## 7. O defeito que esta própria mudança criou — e o que já estava aberto

Trocar o custo reabriu, por outra porta, o buraco que o ADR-019 §3 tinha
fechado. O `analyze` escolhe o backtest de referência do gate B com
`latest_for`, e o banco passou a ter, para o **mesmo** (estratégia, par,
`config_hash`), runs com os dois custos — sendo o mais recente um
`--legacy-cost` rodado para o teste de paridade:

```
id  | par | criado em           | comissão      | label
777 | IJS | 2026-09-08 15:55:28 | fixed-0.35    | gate-a-adr019     <- seria o baseline
769 | IJS | 2026-09-08 15:52:43 | ibkr-fixed-us | gate-a-custo-real
```

Ao investigar isso, apareceu que **o mesmo buraco já estava aberto no eixo do
slippage, e não por causa desta mudança**: `--slippage-bps` nunca marcou o run
como `experimental`, e o ADR-018 manda rodar `--slippage-bps 4` em IJS e SLYV
para sensibilidade. Esse run entraria não-experimental e mais recente. Medido
sobre os trades reais, 2 bp a mais derrubam o avg R de IJS de 0,305 para 0,160
— deslocamento **maior que a banda inteira de ±30%** do gate B.

`latest_for` agora exige os três eixos da régua: `commission_model`,
`slippage_bps` e `limit_fill_haircut_bps`. Run que não declara um deles não
casa com nada, como já acontecia com `experimental`. É a mesma régua que o
`trader-research` usa do lado Python. Coberto por
`crates/trader-infra/tests/backtest_run_baseline_test.rs` (6 testes).

## 8. Um achado de paridade que não estava no plano

O `paper` no modo simulado usava **10 bp** de slippage; o backtest usa 2 bp. O
comentário logo acima da configuração diz, desde sempre, *"Compartilhado com o
backtest para garantir paridade de validação"*. Era cinco vezes mais caro no
caminho que existe justamente para comparar com o backtest. Os dois agora vêm
do mesmo `Default`.

## 9. O que este relatório não decide

- **Não muda o gate.** Os critérios do ADR-019 §7 continuam propostos.
- **Não tira instância do ar.** A fade em IWV tem PF 1,14, avg R negativo e os
  2 melhores meses em 437% do net — o pior conjunto do portfólio, e mais um
  argumento para o §5.10 do plano. Continua sendo decisão do dono.
- **Não substitui o gate A.** `gate-a-com-flatten-2026-09-07.md` continua sendo
  o veredito com a régua do live; este relatório mostra que essa régua ainda
  estava barata.
- **Não diz que a IBKR cobra exatamente isto.** A tabela usada é a **US
  Fixed**; a conta do projeto é IBKR Canada, e o plano cita US$ 0,005/ação com
  mínimo de US$ 1,00 para ela. Se a conta estiver na tabela **Tiered**, o custo
  muda (e há taxas de bolsa e regulatórias que nenhum dos dois modelos inclui).
  O número certo sai do extrato — e o `CommissionReport` da IBKR, quando
  casado, é a fonte.

## 10. Limites honestos

- **O desconto de 2 bp é declarado, não medido**, e responde por 56% do efeito.
  A calibração por ativo (§2.2 do plano tem os dados) não foi feita. O único
  veredito que depende inteiramente dele é o do PF da fade em AVUV; o veredito
  de manchete sobrevive com ele zerado (§3).
- Taxas de bolsa, SEC/FINRA e câmbio não entram em nenhum dos dois modelos.
  O custo real é maior que o deste relatório, não menor.
- Os oito pares continuam sendo os escolhidos entre 42 combinações. Baratear ou
  encarecer o custo não corrige viés de seleção.
- Os cenários da §2 e da §3 são **três runs independentes**, não uma
  decomposição aditiva: a equity diverge entre eles e o sizing dos trades
  seguintes muda um pouco. O resíduo medido é US$ 23 em US$ 3.122 (0,7%).
