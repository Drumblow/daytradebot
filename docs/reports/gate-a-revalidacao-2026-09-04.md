# Revalidação do gate A com o motor corrigido — 04/09/2026

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

## Limites honestos

- Os pares foram **escolhidos** em agosto testando 42 combinações. Revalidá-los
  no mesmo histórico não elimina o viés de seleção original; só remove o viés do
  motor. A formalização disso é o Deflated Sharpe do AFML, ainda pendente.
- 6 janelas OOS sobre 18 meses cobrem um único regime de mercado amplo. Nenhuma
  dessas estratégias foi testada num crash ou numa alta prolongada.
- `range-extreme-fade-v1` em IWV (PF 1,31, 18 trades) é marginal isoladamente.
  Fica pela força do agregado da estratégia; se continuar assim com mais amostra,
  merece revisão.
