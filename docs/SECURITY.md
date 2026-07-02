# Segurança — HumanStyle Trader Bot

**Versão:** 1.0  
**Status:** Aprovado para implementação  
**Última atualização:** 2026-07-02  
**Classificação:** Confidencial — controle financeiro  

---

## 1. Propósito

Este documento estabelece as práticas de segurança para proteger credenciais, capital e integridade operacional do sistema *HumanStyle Trader Bot*.

A segurança financeira é prioridade absoluta. Toda decisão técnica que conflitar com a segurança deve ser rejeitada.

---

## 2. Princípios de segurança

1. **Zero hardcoded secrets:** nenhuma credencial, token, senha ou chave API fica no código.
2. **Defense in depth:** múltiplas camadas de proteção (rede, sistema, aplicação, dados).
3. **Fail-safe:** em dúvida, o sistema não opera.
4. **Mínimo privilégio:** o bot tem apenas as permissões estritamente necessárias.
5. **Auditoria total:** toda ação sensível é registrada e rastreável.
6. **Isolamento de ambientes:** paper e real nunca compartilham credenciais ou banco.

---

## 3. Gestão de credenciais

### 3.1 O que é secreto

- Senhas de conta da corretora.
- API keys e tokens.
- Client IDs da IBKR.
- Dados de autenticação do PostgreSQL.
- Chaves de criptografia.
- Qualquer informação que permita movimentar dinheiro ou acessar contas.

### 3.2 Onde armazenar

| Ambiente | Mecanismo |
|----------|-----------|
| Local/dev | Arquivo `.env` fora do repo, permissão `600` |
| CI/CD | Secrets do GitHub/GitLab |
| VPS/paper | Arquivo `.env` com permissão `600` ou secret manager |
| Futuro production | AWS Secrets Manager, HashiCorp Vault ou similar |

### 3.3 O que NUNCA fazer

- Commitar `.env` ou arquivos de secrets.
- Enviar credenciais por chat ou email.
- Logar secrets, mesmo parcialmente.
- Usar credenciais de produção em desenvolvimento.
- Compartilhar sessão da IBKR entre ambientes.

---

## 4. Segurança da conta na corretora

### 4.1 Conta de paper trading

- Usar conta de paper trading separada da conta real.
- Verificar que paper trading está ativo e configurado corretamente.
- Nunca misturar credenciais de paper e real.

### 4.2 Permissões da API

- Conceder apenas permissões de leitura e trading para a conta de paper.
- Revisar permissões trimestralmente.
- Desativar acesso API quando não estiver em uso por longos períodos.

### 4.3 Autenticação

- Habilitar 2FA na conta da corretora.
- Para automação, usar IB Gateway com "Trust this device" em ambiente seguro.
- Rotacionar senhas anualmente ou após qualquer incidente.

---

## 5. Controles financeiros no software

### 5.1 Regras imutáveis

O software deve implementar e respeitar rigorosamente:

```text
Nunca operar sem stop.
Nunca operar fora do horário configurado.
Nunca operar após perda máxima diária.
Nunca dobrar lote após perda.
Nunca abrir nova posição se já existir posição ativa no mesmo ativo.
Nunca operar dinheiro real no MVP.
```

### 5.2 Parâmetros de risco

| Parâmetro | Valor inicial (conservador) | Onde configurar |
|-----------|----------------------------|-----------------|
| Risco por trade | 1% do capital | `risk_limits` no banco + `config/risk.toml` |
| Perda máxima diária | 2% do capital | `risk_limits` |
| Máximo de trades por dia | 3 | `risk_limits` |
| Risco-retorno mínimo | 1:2 | config da estratégia |
| Spread máximo | 0.05% | config da estratégia |
| Volatilidade máxima (ATR%) | 1.5% | config da estratégia |
| Perdas consecutivas até parar | 3 | `risk_limits` |

### 5.3 Validação de ordens

Antes de enviar qualquer ordem, o `RiskManager` deve validar:

1. Modo de operação é `paper`.
2. Horário está dentro do permitido.
3. Perda diária não foi atingida.
4. Número de trades do dia não excedeu o limite.
5. Não há posição aberta no mesmo ativo.
6. Stop está definido e tecnicamente válido.
7. Risco/retorno atende o mínimo.
8. Spread e volatilidade estão dentro dos limites.
9. Tamanho da posição é compatível com capital e risco.

Se qualquer validação falhar, a ordem é rejeitada e registrada.

---

## 6. Segurança do banco de dados

### 6.1 Credenciais

- Usuário do aplicativo com privilégios mínimos (SELECT, INSERT, UPDATE em tabelas específicas).
- Nunca usar usuário `postgres` ou superuser no aplicativo.
- Senha forte e rotacionada.

### 6.2 Acesso

- PostgreSQL escutando apenas em `localhost` ou interface de VPN.
- Firewall bloqueando porta 5432 externamente.
- Conexões criptografadas (SSL/TLS) quando via rede.

### 6.3 Dados sensíveis

- Não armazenar senhas ou tokens no banco.
- Dados de conta (cash, equity) são sensíveis e devem ter acesso restrito.

---

## 7. Segurança da aplicação

### 7.1 Input validation

- Validar todos os símbolos, timeframes, quantidades e preços antes de usar.
- Rejeitar ordens com valores negativos, zero ou absurdamente altos.
- Sanitizar parâmetros de configuração.

### 7.2 Logging seguro

- Nunca logar credenciais, tokens, senhas ou dados pessoais.
- Mascarar parcialmente IDs de conta quando necessário.
- Logs de ordem podem incluir preços e quantidades, mas não informações de conta.

### 7.3 Comunicação com broker

- Usar HTTPS para APIs REST.
- Validar certificados TLS (não desabilitar verificação).
- Para TWS API local, restringir acesso ao localhost.

---

## 8. Segurança de infraestrutura

### 8.1 Servidor

- Sistema operacional atualizado.
- Firewall ativo (iptables/ufw/cloud firewall).
- SSH apenas por chave, sem senha.
- Usuário dedicado `trader` para executar o bot.
- Fail2ban ou equivalente.

### 8.2 Rede

- VPS em região próxima ao broker (reduz latência e risco de interceptação).
- VPN para acesso administrativo.
- Não expor portas desnecessárias.

### 8.3 Containerização (futuro)

- Docker para desenvolvimento e deploy.
- Imagens mínimas (distroless ou alpine).
- Não rodar container como root.
- Secrets injetados em runtime, não buildados na imagem.

---

## 9. Prevenção de ameaças específicas

| Ameaça | Mitigação |
|--------|-----------|
| Vazamento de credenciais | .env fora do repo, secret manager, rotação. |
| Acesso não autorizado ao servidor | SSH por chave, firewall, usuário dedicado. |
| Injeção de ordens maliciosas | Validação rigorosa no RiskManager; apenas estratégias aprovadas. |
| Overtrading emocional | Limites automáticos de trades diários e perda. |
| Execução acidental em real | Hard check de `TRADER_MODE=paper`; avisos visuais. |
| Bug causando loop de ordens | Rate limiting, circuit breaker, validação de duplicatas. |
| Manipulação de dados históricos | Imutabilidade, hashes, controle de acesso ao banco. |
| Perda de conexão durante operação | Reconexão com backoff; suspensão de novas entradas. |

---

## 10. Checklist de segurança pré-deploy

```text
[ ] Nenhum secret no repositório (verificar com git-secrets ou grep).
[ ] .env.example criado sem valores reais.
[ ] Permissões de .env configuradas como 600.
[ ] Modo de operação confirmado como paper.
[ ] Limites de risco configurados e testados.
[ ] Stop obrigatório em todas as estratégias.
[ ] Usuário do banco com privilégios mínimos.
[ ] Firewall configurado.
[ ] Backups automáticos ativos.
[ ] Logs não contêm credenciais.
[ ] Documentação de incidentes definida.
```

---

## 11. Resposta a incidentes

### 11.1 Descoberta de vazamento de credencial

1. Revogar imediatamente a credencial no broker.
2. Rotacionar todas as senhas e tokens relacionados.
3. Revisar logs das últimas 24h.
4. Notificar responsável.

### 11.2 Execução inesperada em modo real

1. Parar o bot imediatamente.
2. Verificar posições e ordens no broker.
3. Fechar posições não autorizadas manualmente se necessário.
4. Investigar causa (env, config, bug).
5. Documentar e corrigir.

### 11.3 Perda superior ao limite diário

1. Parar novas entradas.
2. Manter stops/alvos das posições abertas.
3. Alertar operador.
4. Investigar se houve bug, slippage ou condição de mercado.
5. Só retomar após análise.

---

## 12. Conformidade e auditoria

- Manter logs de todas as ordens, fills e decisões por no mínimo 5 anos.
- Permitir reconstrução completa de qualquer dia de trading a partir do banco.
- Revisar trimestralmente permissões de API e acessos ao servidor.

---

## 13. Referências

- `docs/ARCHITECTURE.md`
- `docs/OPERATIONS.md`
- `docs/PRD.md`
