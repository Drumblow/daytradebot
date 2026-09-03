# Incidente 03/09/2026 — três ordens de venda para a mesma posição

**Gravidade:** alta (conta paper; teria virado posição vendida de 1.654 ações)
**Duração da exposição ao risco:** ~10 minutos (22h30 → 22h40 UTC)
**Resolvido:** sim, na mesma noite. Causa raiz corrigida.

---

## Resumo

Ao encerrar manualmente as 827 ações órfãs de IWM (ver
`pregoes-2026-08-31_a_09-02.md` §4.1), o comando de flatten enviou **três**
ordens de venda de 827 ações em vez de uma. As três ficaram enfileiradas para a
abertura de 04/09. Se tivessem executado juntas, a conta sairia de +827 para
**−1.654 ações** de IWM (~$487 mil de notional vendido).

O erro foi detectado imediatamente pela ação `exposicao`, as três ordens foram
canceladas, a causa raiz foi corrigida e uma única ordem foi reenviada.

## Linha do tempo (UTC)

| Hora | Evento |
|---|---|
| 22:30:07 | `flatten IWM --confirm` envia a 1ª ordem. IBKR **aceita** e devolve o aviso 399: *"Your order will not be placed at the exchange until 2026-09-04 09:30:00 US/Eastern"* |
| 22:30:09 | `confirm_order` interpreta o fim do stream como falha; o comando repete |
| 22:30:11 | 2ª ordem enviada, mesmo aviso, mesma leitura errada |
| 22:30:13 | 3ª ordem enviada; o comando termina com erro |
| 22:33 | `exposicao` mostra **3 ordens** de venda de 827 abertas |
| 22:40 | `cancelar-ordens IWM --confirm` cancela as três; broker confirma 0 restantes |
| 22:41 | Nova tentativa de flatten: a ordem 4 é enviada **e cancelada em menos de 1s** — segundo defeito, ver §Causas |
| 22:50 | Com as duas correções no ar, ordem 5 enviada e preservada |
| 22:52 | Estado final verificado: 1 posição, 1 ordem |

## Causas

### 1. Ordem transmitida sem confirmação era reportada como falha

`confirm_order` (`ibkr/broker.rs`) devolvia
`Err("stream de confirmação encerrado sem status")` quando o stream terminava
sem um `OrderStatus`. Mas o submit já tinha acontecido: **a ordem foi
transmitida**. O caminho de *timeout*, logo abaixo, já fazia a coisa certa
("assumindo aceita") — o caminho de fim de stream não.

Uma ordem enviada fora do pregão cai exatamente aí: a IBKR aceita, responde com
o aviso 399 e não emite status até a abertura.

**Consequência mais grave que a duplicação:** este é, quase certamente, o
mecanismo que criou a posição órfã de IWM em 07/08. Sinal aceito → ordem
enviada → adapter reporta falha → ninguém rastreia → a ordem executa e a
posição fica órfã, travando o símbolo por 18 pregões.

### 2. Retentativa cega em ordem a mercado

`close_at_market` repetia o envio ao receber erro, sem verificar se a tentativa
anterior tinha chegado ao broker. Numa ordem a mercado, "não sei se foi" nunca
autoriza mandar de novo.

### 3. O cancelamento derrubava a própria ordem de fechamento

A ordem que zera a posição é, ela mesma, uma ordem do **lado da saída**. O passo
de limpeza, que cancela pernas de proteção filtrando por símbolo + lado,
cancelava exatamente o que tinha acabado de ser enviado (ordem 4, 22:41).

O padrão do erro é o mesmo do flatten que fechava posição de outra instância,
corrigido horas antes: **agir sobre ordens por símbolo + lado, sem olhar
identidade**.

## Correções

| # | Correção | Commit |
|---|---|---|
| 1 | Fim de stream sem status vira aviso e `Ok` — rejeição de verdade continua erro (Notice 2xx, status `Inactive`/`Cancelled`) | `82671e9` |
| 2 | Retentativa só depois de **provar** no broker que nenhuma ordem de saída está trabalhando; se a consulta falhar, o fechamento é reportado como incerto | `82671e9`, `4fec07d` |
| 3 | `close_position_at_market` e `close_at_market` devolvem o id da ordem enviada; o cancelamento pula esse id | `4fec07d` |
| 4 | Comando novo `cancel-orders` + ação `cancelar-ordens` no `ops.yml`, para limpar ordens que não deveriam existir | `82671e9` |

A correção 2 entrou também no laço do live (`close_position_at_market`), que
tinha a mesma retentativa cega na saída por tempo e no flatten de fim de sessão.

## O que funcionou

- **A ação `exposicao` pegou o problema em três minutos.** Ela tinha sido criada
  poucas horas antes, no mesmo dia, para diagnosticar a posição órfã. Sem ela, as
  três ordens só apareceriam na abertura.
- O modo de ensaio (`confirmar=false`) mostrou corretamente o que seria feito
  antes do primeiro envio.
- `cancel-orders` reconsulta o broker no fim e só sai com sucesso se sobrar zero
  — não deixou ninguém adivinhando.

## Lições

1. **Toda ação sobre ordem precisa carregar identidade.** Símbolo + lado não
   identifica uma ordem. Os três defeitos desta noite e o do flatten entre
   instâncias são o mesmo erro.
2. **Ausência de confirmação não é rejeição.** Em protocolo assíncrono de ordem,
   o estado desconhecido tem que ser tratado como "possivelmente enviada", nunca
   como "não enviada".
3. **Reenvio de ordem a mercado exige prova, não suposição.**
4. Ferramenta de diagnóstico paga o custo dela na primeira vez que é usada.

## Pendências que este incidente deixa

- Fechar o ciclo: confirmar na abertura de 04/09 que a ordem 5 executou e que
  IWM saiu do bloqueio (as duas instâncias devem voltar a gravar
  `market_context` no primeiro candle).
- O fill dessa venda vai aparecer como "fill sem ordem rastreada" no log das
  instâncias de IWM — esperado, é uma posição que o bot nunca rastreou.
