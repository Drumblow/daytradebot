# Revalidação do gate A com o motor corrigido — 04/09/2026

> ## ⚠️ SUBSTITUÍDO em 07/09/2026 — não use os números desta página
>
> Este relatório mediu o gate A com um backtest que **não fazia flatten de fim
> de pregão**, enquanto o live encerra tudo a mercado às 15h55 ET. Comparou o
> live com um motor que ganha dinheiro dormindo posicionado (ADR-018).
>
> A releitura está em **`docs/reports/gate-a-com-flatten-2026-09-07.md`** e
> muda o veredito: a `balance-area-breakout-v1` **reprova** o gate A pelo avg R
> (VBR −0,013; AVUV −0,173 com WR 31%) — só IJS passa sozinha. A
> `range-extreme-fade-v1` reprova em IWV. A `opening-reversal-v1` sobe e passa
> em IWM/IWN.
>
> Dois motivos a mais para não reaproveitar nada daqui: o hotfix v1.0.1 da
> range-fade mudou o `config_hash` dela (`49ee6f04…` → `818b5339…`), e o PF em
> **R** — que este relatório não tinha — põe a balance-area **abaixo de 1** em
> AVUV (0,74) e VBR (0,97).
>
> Mesmo precedente do ADR-015 §4: nenhum run anterior ao flatten é comparável
> com os novos. O texto abaixo fica como registro do que se sabia em 04/09.

**Por que existe:** todo backtest anterior a 03/09/2026 mediu um mundo mais
generoso que a realidade. Três correções mudaram as regras, e o carimbo de
aprovação do gate A (ADR-010) foi emitido com a régua antiga:

| Correção | O que o backtest antigo assumia | O que a realidade faz |
|---|---|---|
| **ADR-015** | entrada stop enche no preço do gatilho, mesmo com a barra abrindo muito além | enche num preço pior, ou a ordem é inválida e não enche |
| **A3** | **dois** candles para o rompimento acontecer | o live dá **um** |
| **A4** | slippage 100× menor que a config, e **zero** custo nas saídas | ~2 bp na entrada e na saída, mais gap no stop |

A primeira vítima já foi a `pullback-trend-v1`: com custo realista ficou com
profit factor **abaixo de 1 nos três ativos** e saiu do ar em 04/09 (ADR-016).
Este documento aplica a mesma régua às três estratégias restantes.

**Método:** `trader-cli walkforward`, 6 janelas out-of-sample, 24/02/2025 →
02/09/2026 (~9.936 candles de 15 min por ativo), código de `01aab02`. Mesma
metodologia da validação original de agosto — só o motor mudou.

---

## Resultado por par

| Estratégia | Ativo | Trades OOS | Acerto | PF | avg R | DD% | P&L | Falha em |
|---|---|---|---|---|---|---|---|---|
| balance-area-breakout-v1 | IJS | 23 | 60,9% | 2,35 | 0,749 | 1,58 | +5.268 | amostra |
| balance-area-breakout-v1 | VBR | 33 | 42,4% | 1,76 | 0,087 | 1,28 | +3.777 | amostra, avg R |
| balance-area-breakout-v1 | AVUV | 35 | 40,0% | 1,85 | 0,047 | 3,38 | +4.953 | amostra, avg R |
| opening-reversal-v1 | IWM | 32 | 46,9% | **1,09** | 0,217 | 3,84 | +742 | amostra, **PF** |
| opening-reversal-v1 | IWN | 26 | 46,2% | **1,11** | 0,133 | 1,72 | +683 | amostra, **PF**, avg R |
| range-extreme-fade-v1 | AVUV | 26 | 61,5% | 1,95 | 0,394 | 0,89 | +2.720 | amostra |
| range-extreme-fade-v1 | SLYV | 20 | 70,0% | 2,95 | 0,515 | 0,56 | +3.356 | amostra |
| range-extreme-fade-v1 | IWV | 18 | 55,6% | 1,31 | 0,043 | 0,78 | +422 | amostra, avg R |

**Nenhum par isolado fecha o gate**, e sempre pelo mesmo motivo: a amostra. Em
18 meses essas estratégias produzem de 18 a 35 trades OOS por par. Exigir 50 por
par pede uns 3 anos de histórico.

## Resultado por estratégia

O critério de amostra faz sentido na **estratégia**, não no par: o símbolo é
parâmetro, não uma estratégia diferente. Agregando:

| Estratégia | Pares | Trades OOS | Acerto | PF | P&L | Veredito |
|---|---|---|---|---|---|---|
| **range-extreme-fade-v1** | 3 | 64 | 62,5% | **2,10** | +6.498 | **PASSA** |
| **balance-area-breakout-v1** | 3 | 91 | 46,2% | **1,96** | +13.999 | **PASSA** |
| **opening-reversal-v1** | 2 | 58 | 46,6% | **1,11** | +1.425 | **NÃO PASSA** |

## Leitura

**As duas que passam, passam com folga.** PF de 1,96 e 2,10 depois do custo real
de execução, com drawdown abaixo de 3,4% em todos os pares e P&L positivo em
todos os 6. Não é resultado marginal.

**A `opening-reversal-v1` repete a história da pullback.** PF de 1,11 no
agregado, com os dois pares consistentes (1,09 e 1,11) — não é ruído de um par
azarado, é o padrão da estratégia. Com 58 trades, um PF de 1,11 **não é
distinguível de 1,0**: pelo que a amostra permite afirmar, ela não tem vantagem
depois de pagar a corretora. É a mesma assinatura da pullback, só que um degrau
acima do prejuízo em vez de um abaixo.

**O avg R baixo do balance em VBR e AVUV (0,087 e 0,047) merece atenção**, e não
é contradição com o PF de 1,8: o profit factor compara dinheiro ganho com
dinheiro perdido, o avg R compara o resultado com o risco **orçado** de cada
trade. Os dois juntos dizem que a estratégia ganha dinheiro tomando bem mais
risco por trade do que colhe em retorno. Isso importa para dimensionamento, não
para a decisão de manter no ar.

## Decisão proposta

**Manter as três no ar em paper, com status diferente:**

- `balance-area-breakout-v1` e `range-extreme-fade-v1` — **gate A fechado** com o
  motor corrigido. Elegíveis para o gate B e, cumprido ele, para dinheiro real.
- `opening-reversal-v1` — **gate A reprovado**. Continua em paper porque aporta
  amostra para o gate B e custa nada em conta simulada, mas **fica bloqueada
  para dinheiro real** até apresentar edge com o custo realista.

Por que não cortar como se fez com a pullback: a pullback **perdia** dinheiro
(PF 0,68–1,09), esta **ganha pouco**. Cortar tudo que é marginal na mesma base
de dados que serviu para escolher os pares é overfitting — e o portfólio já caiu
de 11 para 8 instâncias esta semana. Reduzir para 6 tornaria a amostra do gate B
lenta demais para ser útil.

## O que isto muda no projeto

1. **O gate A do ADR-010 passa a ser lido por estratégia**, com a amostra
   agregada entre os pares. A leitura por par continua registrada acima, mas
   exigir 50 trades OOS por par é inalcançável no histórico disponível.
2. **A validação de agosto está formalmente substituída por esta.** Os números
   antigos (PF 1,70 na opening-reversal em IWM, por exemplo) não são
   comparáveis: mediam sem custo de execução.
3. **Só agora existe base para decidir sobre estratégia nova ou cripto.** O motor
   mede certo, duas estratégias estão comprovadas e uma está reprovada — e sabe-se
   qual é qual.

## As cinco arquivadas: vale reabrir?

O motor mudou depois que elas foram reprovadas — entao valia conferir se alguma
reprovacao tinha sido injusta. Havia uma hipotese concreta: o **C5** corrigiu a
contabilidade de caixa do short, que corrompia a curva de equity, e a equity
dimensiona os trades seguintes. Uma estrategia de venda a descoberto podia ter
sido julgada com numeros distorcidos.

Backtest de 24/02/2025 a 02/09/2026 sobre 14 ativos, motor atual:

| Estratégia | Veredito original | Amostra agora | PF agora | P&L |
|---|---|---|---|---|
| breakout-first-pullback-v1 | 9 trades — rara demais | 21 trades / 12 ativos | 0,92 | −611 |
| **low2-m2s-short-v1** | 962 trades, net −55k | 920 trades / 14 ativos | **0,66** | **−83.485** |
| trendline-break-test-v1 | PF 1,04 (nota: "promissora") | 64 trades / 14 ativos | 0,91 | −839 |
| value-area-reentry-v1 | PF 0,76 / 0,84 / 0,89 | 107 trades / 14 ativos | 0,65 | −7.559 |
| failure-test-long-v1 | amostra insuficiente | 200 trades / 14 ativos | 0,61 | −13.191 |

**Nenhuma se salva, e todas pioraram.** A hipotese do C5 foi refutada da forma
mais util possivel: a `low2-m2s-short-v1` foi de −55k para **−83,5k**. A
contabilidade de short corrompida estava *inflando* o resultado dela, nao
escondendo uma vantagem.

Dois subprodutos que valem mais que o veredito:

1. **A `failure-test-long-v1` deixou de ser "inconclusiva".** Ela tinha sido
   arquivada por amostra insuficiente (6–9 trades/ano/ativo). Com o universo de
   14 ativos sao 200 trades — poder estatistico de sobra para dizer que o
   problema nunca foi amostra: PF 0,61.
2. **O "PF 1,38 promissor" da `trendline-break-test-v1` era artefato de custo
   zero.** Com custo realista vira 0,91.

Vale notar o padrao dos "melhores pares" de cada uma: PF de 3 a 7 sobre 2 a 5
trades. E exatamente a armadilha de testes multiplos que a auditoria apontou —
testar 14 ativos × 5 estrategias produz combinacoes espetaculares por acaso.
Nenhuma delas significa nada.

**Conclusao: as cinco continuam arquivadas, agora com veredito emitido pela
regua certa.** A questao esta fechada e nao precisa ser reaberta a cada mudanca
do motor — a menos que a mudanca torne o motor mais PERMISSIVO, o que nao e o
caso de nenhuma correcao feita ate aqui.

## Limites honestos

- Os pares foram **escolhidos** em agosto testando 42 combinações. Revalidá-los
  no mesmo histórico não elimina o viés de seleção original; só remove o viés do
  motor. A formalização disso é o Deflated Sharpe do AFML, ainda pendente.
- 6 janelas OOS sobre 18 meses cobrem um único regime de mercado amplo. Nenhuma
  dessas estratégias foi testada num crash ou numa alta prolongada.
- `range-extreme-fade-v1` em IWV (PF 1,31, 18 trades) é marginal isoladamente.
  Fica pela força do agregado da estratégia; se continuar assim com mais amostra,
  merece revisão.
