# Estatística do gate A — o critério que não dava para avaliar — 08/09/2026

**Por que existe:** o ADR-019 §7 propõe acrescentar ao gate A o critério
"limite inferior do IC95 do profit factor por bootstrap em blocos ≥ 1,0", e a
própria lista de pendências do ADR registrava que ele **não tinha como ser
avaliado** — faltava o relatório em Python do §5.3 do plano (item 1 da lista).
Agora ele existe (`trader-research/`) e o critério tem número.

> ## ⚠️ A RÉGUA MUDOU DE NOVO NO MESMO DIA — leia com o custo em mente
>
> Os números deste relatório usam a comissão de **US$ 0,35 fixos por perna** e
> um alvo limite que enche de graça — o modelo do simulador quando ele foi
> escrito. Horas depois, o §5.6 do plano trocou os dois: comissão **por ação**
> da IBKR (9,9× a antiga, medido nos 214 trades) e um desconto de 2 bp no fill
> do alvo, que sozinho responde por 56% do efeito.
>
> O que muda: **o portfólio dos oito pares deixa de passar tudo** — reprova no
> avg R (0,107 contra o piso de 0,15) — e o limite inferior do IC95 cai de
> 1,135 para 1,038. Nenhum recorte passa mais o gate, e isso vale mesmo com o
> desconto zerado (avg R 0,147, IC95 1,089).
>
> O que **não** muda: o achado central deste relatório, de que a **unidade** em
> que o gate é lido decide o veredito, e a crítica ao esquema do bootstrap
> (§3). Esses são sobre método, não sobre o nível dos números.
>
> Números com o custo real em `docs/reports/custo-real-2026-09-08.md`.

**O que mudou de fato:** nada nas estratégias, nada no live. O motor de
backtest ganhou um campo no JSON (`oos_sessions`, o calendário de pregões da
amostra OOS) e o `initial_capital`; o resto é análise sobre os mesmos oito
walk-forwards de `gate-a-com-flatten-2026-09-07.md`, com a mesma régua
(2 bp/lado, flatten às 15h45 ET, hotfix v1.0.1 da range-fade).

**Reprodução:**

```bash
./trader-research/regenera-runs.ps1
cd trader-research && uv sync
uv run trader-research ../out/adr018/wf19_*.json --por par
uv run trader-research ../out/adr018/wf19_*.json --por estrategia
uv run trader-research ../out/adr018/wf19_*.json --por portfolio --capital 238000
```

Seed fixa (`20260907`), 10.000 reamostras: rodar duas vezes dá o mesmo número.

---

## 1. O achado: a **unidade** do gate decide o veredito

Os mesmos 214 trades, os mesmos oito pares, a mesma régua. Só muda o recorte
em que os critérios são lidos:

| recorte | reprova em |
|---|---|
| 8 pares isolados | **todos os 8** |
| **3 estratégias (pool dos pares)** — a unidade que o ADR-019 §7 propõe | **todas as 3** |
| portfólio dos 8 pares junto — recorte **novo, que nenhum ADR propõe** | nada, nos nove critérios que dão para medir |

**Comece pela linha do meio.** O ADR-019 §7 diz, com estas palavras, "por
estratégia". Nessa unidade — a proposta — **as três reprovam**, todas no
critério do IC95 e todas na concentração. Essa é a resposta à pergunta que o
ADR fez.

A terceira linha é um recorte que eu acrescentei; ela não está proposta em
lugar nenhum e **não deve ser lida como aprovação**. Ela está aqui porque a
diferença entre as três linhas é o achado, e escondê-la seria escolher o
recorte depois de ver o resultado — que é exatamente o modo de falha que este
relatório existe para vigiar. Os nove critérios são os seis do ADR-010 mais os
três do §7 que dão para medir num recorte agregado (IC95 em blocos, PF em R,
concentração); os outros dois do §7 — holdout travado e sensibilidade a 4–5 bp
— **não foram rodados** para o portfólio.

Por que os números divergem tanto entre recortes não é paradoxo estatístico, é
agregação. Um par isolado tem 18–35 trades e nunca chega aos 50 do ADR-010;
três pares da mesma estratégia chegam à amostra mas diluem o ponto central (a
balance-area pooled cai para PF 1,68 porque IJS 2,54 é somado a AVUV 1,43); os
oito juntos ganham amostra **e** diversificação, e o intervalo aperta o
suficiente para o limite inferior cruzar 1,0 — carregando junto todo o viés de
seleção das 42 combinações (§5).

**Consequência prática:** adotar os critérios do ADR-019 §7 sem decidir a
unidade não resolve nada — só muda o lugar da discussão. A nota do ADR-010 de
07/09 já tinha movido a leitura de "por par" para "por estratégia"; este
relatório mostra que essa escolha, sozinha, decide aprovação e reprovação.

## 2. O critério proposto, medido

Bootstrap estacionário (Politis–Romano) sobre o P&L diário de **todos os
pregões, zeros incluídos**, blocos médios de 5 e 10 pregões, 10.000
reamostras — o esquema que o ADR-019 §8 pré-registrou.

### Por par

| estratégia · par | n | PF em $ | IC95 inf. (L=5) | reprova em |
|---|---|---|---|---|
| balance · AVUV | 35 | 1,43 | **0,524** | amostra, WR, avg R, IC, PF_R, concentração |
| balance · IJS | 23 | 2,54 | **0,848** | amostra, IC, concentração |
| balance · VBR | 34 | 1,52 | **0,563** | amostra, avg R, IC, PF_R, concentração |
| openrev · IWM | 32 | 1,72 | **0,749** | amostra, IC, concentração |
| openrev · IWN | 26 | 1,57 | **0,702** | amostra, IC, concentração |
| fade · AVUV | 26 | 1,48 | **0,555** | amostra, IC, concentração |
| fade · IWV | 18 | 1,32 | **0,328** | amostra, avg R, IC, PF_R, concentração |
| fade · SLYV | 20 | 2,64 | **1,003** | **só a amostra** |

`range-extreme-fade-v1` em SLYV é a única combinação viva que passa o critério
proposto, e por 0,003. É também a única que reprova o gate por um único item —
os 50 trades, que nenhum par deste projeto alcança em 18 meses.

### Por estratégia e no portfólio

| recorte | n | PF em $ | PF em R | avg R | IC95 inf. (L=5) | 2 melhores meses |
|---|---|---|---|---|---|---|
| balance (3 pares) | 92 | 1,68 | 1,04 | 0,025 | 0,768 | 90% |
| openrev (2 pares) | 58 | 1,65 | 1,81 | 0,376 | 0,842 | 91% |
| fade (3 pares) | 64 | 1,75 | 1,50 | 0,209 | 0,959 | 65% |
| **portfólio (8 pares)** | **214** | **1,69** | **1,35** | **0,175** | **1,135** | **49%** |

## 3. O esquema do bootstrap mudou um veredito — e por isso está pré-registrado

A primeira implementação deste pacote reamostrava **só os pregões com trade**.
Parece equivalente e não é: estas estratégias operam em 5% a 10% dos pregões,
então isso reamostra 17–32 pontos onde o pré-registro manda reamostrar ~330.
Pior, com n dessa ordem um bloco médio de 5 vira 16% a 29% da série — faixa em
que o bootstrap circular vira **rotação** da série original e o "IC95" perde
cobertura, ficando mais **permissivo** exatamente onde se anuncia conservador.

O efeito medido, com os mesmos trades:

| par | esquema errado (só dias ativos) | esquema pré-registrado |
|---|---|---|
| balance · IJS | 1,337 → **PASSA** | 0,848 → **NÃO passa** |
| fade · SLYV | 1,216 → PASSA | 1,003 → PASSA (por 0,003) |
| portfólio | 1,108 → PASSA | 1,135 → PASSA |

Um veredito virou. O relatório agora imprime o esquema alternativo ao lado, e
o código **recusa** um bloco maior que 10% da série em vez de devolver um
intervalo bonito e errado (`BlocoLongoDemais`). Nos recortes por par o esquema
alternativo já nem é calculável — é o próprio motivo de o pré-registro existir.

## 4. Onde o número **não** ajuda

**Concentração continua reprovando quase tudo.** Sete dos oito pares têm os
2 melhores meses somando 74% a 225% do net — acima de 100% significa que a
soma dos demais meses é negativa. Só SLYV (59%) e o portfólio (49%) ficam
abaixo do teto proposto de 60%.

**O P&L por ano civil** (item 7 das pendências do ADR-019, que o `metrics.rs`
ainda não calcula) mostra de onde vem o dinheiro:

| recorte | 2025 | 2026 |
|---|---|---|
| balance (3 pares) | +7.734 | **−248** |
| openrev (2 pares) | +2.463 | +3.647 |
| fade (3 pares) | +3.956 | +144 |
| portfólio | +14.152 | +3.543 |

Das três, só a `opening-reversal-v1` — a que o gate A de 04/09 **reprovou** —
está ganhando dinheiro em 2026. A `balance-area-breakout-v1`, aprovada "com
folga" naquele relatório, está negativa no ano.

**PSR e DSR não aprovam nada.** No portfólio, PSR(0) = 0,998 sobre o P&L
diário, mas o DSR cai para **0,766** com N=42 tentativas (o número de
combinações testadas na seleção de pares de agosto) e para **0,448** quando
calculado sobre o R por trade. Abaixo do 0,95 convencional nos dois casos.

**A direção do erro do DSR é desconhecida**, e uma versão anterior deste
pacote afirmava o contrário. Sem `n_trials` registrado (item 3 das pendências
do ADR-019), a variância entre tentativas é substituída pela variância do
estimador de uma série. Se as tentativas forem correlacionadas — variações de
parâmetro da mesma regra, que é o caso registrado —, o DSR impresso é
**pessimista**. Só se cobrissem estratégias genuinamente diferentes ele seria
otimista.

## 5. O que o resultado do portfólio **não** é

O único recorte que passa tudo é o que mais precisa de ressalva:

1. **Os 8 pares foram escolhidos entre 42 combinações** testadas em agosto. O
   portfólio herda o viés de seleção inteiro; é exatamente o que o DSR de
   0,448 está dizendo.
2. **8 pares são 7 ativos.** AVUV aparece duas vezes (balance e fade): as duas
   posições caem no mesmo papel, e a diversificação é menor do que a contagem
   sugere. Todos são ETFs small-cap value americanos — correlacionados por
   construção.
3. **O drawdown do portfólio não descreve conta nenhuma.** O P&L vem de 8
   backtests independentes, cada um dimensionando sobre os próprios US$ 100k
   (soma US$ 800k), e o percentual é medido contra US$ 238k. Numa conta
   compartilhada de verdade o sizing de cada trade seria menor e o P&L cairia
   junto. Refazer isso é o replay do §6.4 do plano, que este pacote **não**
   faz.
4. **80% do P&L é de 2025.** US$ 14.152 contra US$ 3.543.
5. **A amostra continua sendo o gargalo.** 214 trades OOS em 18 meses, sobre
   um único regime de mercado amplo.

## 6. O que este relatório não decide

- **Não muda o gate.** Os critérios do ADR-019 §7 continuam propostos; o gate
  vigente é o do ADR-010. Adotá-los — e escolher a unidade em que são lidos —
  é decisão do dono (§10.2 do plano).
- **Não move nem tira instância nenhuma do ar.** A `range-fade` em IWV reprova
  em cinco critérios (amostra, avg R, IC95, PF em R, concentração), tem o pior
  limite inferior do conjunto (0,328) e os 2 melhores meses em 225% do net —
  o que reforça o §5.10 do plano (retirá-la), mas isso é mudança de produção e
  continua sendo decisão do dono.
- **Não substitui o `gate-a-com-flatten-2026-09-07.md`.** Aquele relatório
  continua sendo o veredito do gate A com a régua do live; este acrescenta as
  colunas que faltavam e a leitura por portfólio.
- **Não corrige o Sharpe do motor.** O `metrics.rs` continua anualizando pelo
  intervalo mediano da série (item 5 das pendências do ADR-019); a linha de
  Sharpe de qualquer run deste harness continua sem significado. O Sharpe
  usado aqui é o da própria frequência da série, calculado no Python.

## 7. Como este número foi criticado antes de ser publicado

O pacote passou por uma rodada adversarial de refutação (08/09/2026, seis
lentes independentes, três céticos por achado): **63 achados levantados, 20
sobreviveram**. Os cinco críticos, todos corrigidos antes deste relatório:

1. O bootstrap não seguia o esquema pré-registrado (§3 acima) — **mudou
   número e veredito**.
2. O bloco de 5 sobre 17–32 pregões degenerava em rotação, com cobertura real
   de 84–86% num intervalo rotulado como 95%.
3. Nenhum teste prendia a fórmula do PSR: quatro mutações do núcleo (sinal da
   assimetria, curtose excedente no lugar da bruta, termo em SR² ausente, n em
   vez de n−1) passavam a suíte inteira. A fórmula estava certa; nada a
   mantinha certa. Hoje sete mutantes morrem.
4. A linha do veredito podia ser invertida (`.inferior` → `.superior`) sem
   quebrar um teste — não havia nenhum teste de `cli.py`.
5. A suíte inteira tinha escore de mutação de 32%: 38 de 56 bugs plausíveis
   injetados passavam com tudo verde.

A suíte foi de 47 para **97 testes**, e os mutantes nomeados nos achados 1 a 4
morrem — os sete do núcleo do PSR/DSR foram reexecutados um a um e todos
quebram a suíte. **O escore de mutação global não foi medido de novo**: dizer
que "subiu" sem rodar os 56 mutantes seria a mesma classe de afirmação que
este ciclo passou o dia derrubando.

Três achados eram texto meu afirmando mais do que media: o "DSR é limite
superior" (direção não medida e provavelmente invertida), o exemplo de
dezembro que justificava a regra de fuso (16h05 ET em dezembro é 21h05 UTC —
**o mesmo dia**; a conta estava errada), e a convenção `exit_time` × `entry_time`,
que é indemonstrável nesta amostra porque nenhum dos 214 trades atravessa a
noite. Os três estão corrigidos, e a conversão de fuso agora tem testes que
fabricam os casos que discriminam.

## 8. Limites honestos

- O bootstrap trata os pregões como a unidade de dependência. Correlação entre
  **ativos** no mesmo dia é preservada (o dia inteiro é reamostrado junto),
  mas correlação entre dias além do bloco não é.
- O IC é percentil simples, não BCa. Para uma razão enviesada como o profit
  factor, com 330 pregões dos quais 5–38% têm trade, o percentil é o
  compromisso declarado — não o estimador ótimo.
- `n_trials` continua sendo estimativa declarada em relatório, não registro
  (item 3 das pendências do ADR-019). Os N usados aqui (2, 6, 42) são
  escolhidos, não medidos.
- Os modos de sizing B/B'/C do Monte Carlo ignoram o teto de notional, que é o
  que hoje realmente prende. São teto, não previsão — e dependem do ADR-020,
  que continua não implementado.
