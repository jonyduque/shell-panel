---
name: shell-panel-review
description: Revisao geral do shell-panel por subagentes - particoes, lentes com uma pergunta cada, refutacao de todo achado bloqueante e conferencia mecanica das ancoras. Use quando pedirem revisao geral/auditoria do projeto, ao revisar uma lente ou particao isolada, ou ao aceitar achados de revisao vindos de subagente. Traz o gate, a prova de mutacao, a busca de consumidores e o verificador de ancoras.
---

# Revisao geral do shell-panel

A revisao roda pelo workflow `.claude/workflows/shell-panel-review.js` com a configuracao
`.claude/review/review.json`. O orquestrador NAO revisa: despacha, audita o que volta e consolida.

## Por que este formato (o que escapou da ultima rodada)

As 18 tasks do plano `docs/superpowers/plans/2026-09-21-review-fixes.md` fecharam com gate verde e
revisao aprovada. Depois disso, **cinco defeitos** foram achados pelo usuario usando o binario:

| Commit | Defeito | Classe |
|--------|---------|--------|
| 3fb09f2 | CR/LF num match (historico multi-linha, relatorio forjado) executava a linha | dado nao confiavel do PTY escrito como digitacao |
| 5dc77fe | tecla apos Tab antes do dropdown perdia o Tab; restos de dropdown ao paginar | sequencia de teclas com relatorio pendente; estado de tela entre redesenhos |
| f590ff7 | ListView do PSReadLine colidia com o dropdown | interacao com configuracao do usuario |
| 6f4e060 | Enter com dropdown aberto executava a linha | tabela de teclas do README nunca dirigida contra o binario |
| c345dd9 | `[System.IO.Fi`+Tab perdia o `]` | semantica do ResultType do PowerShell |

Nenhum foi pego por gate nem por revisao de codigo: **todos aparecem so dirigindo o binario real**.
Por isso existe a lente `interativo`, que escreve testes de rascunho sobre o harness
`tests/common/mod.rs` (Terminal::shell_panel) e roda o binario no ConPTY.

## Ferramentas (provadas antes do uso)

Caminhos absolutos: agentes em worktree nao veem estes arquivos nao versionados.

| Script | Uso | Exit |
|--------|-----|------|
| `scripts/verify.ps1` | fmt + clippy `-D warnings` + `cargo test` | 0 verde, 1 vermelho |
| `scripts/mutate.ps1` | muta 1 trecho de producao, roda 1 teste, restaura byte a byte | 0 MORTA (coberto), 1 SOBREVIVEU, 2 inconclusivo, 3 falha ao restaurar |
| `scripts/find_consumers.ps1` | consumidores de um simbolo fora do arquivo de origem | 0 ha consumidor, 1 so na origem, 2 so testes |
| `scripts/check_anchors.ps1` | confere que o trecho citado existe perto da linha | 1 se ha ancora MORTA |
| `scripts/user_state.ps1` | `-Save`/`-Compare` (pwsh 7): PATH do usuario (tipo + hash), linhas de teste no historico do PSReadLine, instalacao real, fragmento do Terminal, config | 0 igual, 1 mudou (pare e relate) |

`mutate.ps1` serve tambem para comandos que nao sao cargo: ele reconhece a execucao pela linha
`test result:` (o `scripts/test-installer.ps1` imprime uma no formato do cargo). Mutacao em arquivo
`.ps1` funciona igual. `user_state.ps1` foi provado com snapshot adulterado (saida 1) e repetido
(saida 0); nao rode `-Compare` durante um teste do instalador em andamento (ele poe uma entrada
temporaria no PATH). Ele le o caminho do historico do PSReadLine sob `-NoProfile`: um
`HistorySavePath` mudado no perfil do usuario nao e conferido (so o caminho padrao).
So um teste do instalador por vez na maquina: o `scripts/test-installer.ps1` segura o mutex
`Global\shell-panel-installer-test`; um segundo espera ate 600 s e desiste sem tocar o PATH.

Provas feitas em 2026-10-01 (base 562e3e0): mutacao `contains('-')`->`contains('_')` em
`src/engine/aggregate.rs:69` MORTA por `test_shell_suggestions_kinds_and_descriptions`;
a mesma contra `color_test` SOBREVIVEU; mutacao que nao compila e filtro sem teste deram 2.
A primeira versao do mutate.ps1 classificava `error: test failed` como erro de build e
devolvia 2 para toda mutacao morta - por isso as provas existem.

## Regras para todo agente da revisao

- **READ-ONLY no checkout principal.** Quem precisa executar roda em worktree propria e apaga
  os arquivos de rascunho antes de terminar (worktree sem mudanca e removida sozinha).
- **Nunca** `CARGO_TARGET_DIR` compartilhado: o `target/debug/shell-panel.exe` e unico por
  target e os e2e de um worktree rodariam o binario de outro. Build limpo leva ~90 s; aceite.
- Todo achado tem `arquivo:linha` e **trecho literal copiado**. Trecho reescrito vira ancora
  morta e o achado inteiro e descartado pelo `check_anchors.ps1`.
- Evidencia no passado e com saida colada. "Se removessemos X o teste falharia" nao e prova.
- Achado de delecao exige `find_consumers.ps1` colado. Achado de teste fraco exige
  `mutate.ps1` colado com exit 1 (sobreviveu).
- Zero achados e resultado legitimo. Achado marginal inventado custa uma refutacao.
- Nao revise o que `decisionsNotToRelitigate` em review.json fecha.

## Licoes da rodada 1 (2026-10-01, run wf_6a3690b0-aeb)

- **e2e oscilam sob carga.** Com 10 worktrees compilando e rodando ConPTY ao mesmo tempo,
  `test_completion_uses_the_real_line_and_the_real_session` falhou 2 de 3 vezes na base e e2e
  em paralelo cairam por timeout. Logo, MORTA vindo de e2e nesta maquina pode ser ruido: so
  aceite MORTA de e2e com a base rodada logo antes e verde. SOBREVIVEU continua valendo.
- **Testes e2e e rascunhos gravam no historico PSReadLine REAL do usuario**
  (`ConsoleHost_history.txt`: ~110 linhas de teste encontradas). `-NoProfile` nao impede isso.
  Rascunho interativo deve enviar primeiro `Set-PSReadLineOption -HistorySaveStyle SaveNothing`.
  Desde 2026-10-03 toda sessao iniciada pelo cargo grava em `target/test-history.txt`
  (`SHELL_PANEL_TEST_HISTORY` em `.cargo/config.toml`, honrado pelo script de integracao). Antes
  disso cada gate completo gravava 10 linhas no historico real; o `user_state.ps1` so via 1 delas,
  porque conta apenas linhas com marcadores. Rascunho fora do cargo continua gravando no real.
- **Zero derrubados em 26** nao prova refutador fraco: aqui toda evidencia tinha saida de
  execucao e o refutador reproduziu na propria worktree. Mesmo assim confira os Criticos.
- Uma worktree ficou registrada sem mudancas (`.claude/worktrees/...-13`): ao fim, rode
  `git worktree list` e remova as limpas com `git worktree remove` + `git branch -D`.
- O `meta` do workflow precisa ser literal puro: concatenar strings com `+` e rejeitado.

## Licoes das correcoes (2026-10-01 a 2026-10-02: criticos, importantes, menores, modo transparente)

- **Seguranca de subagentes.** Duas vezes um subagente mexeu na maquina do usuario: rodou o
  `install.ps1` com os locais padrao (reinstalou a instalacao real) e rodou o teste do instalador
  sob o Windows PowerShell 5.1, que morreu antes de restaurar e deixou `;%SP_INSTALLER_TEST%\bin`
  no PATH. Toda instrucao para agente leva as regras de seguranca do `CLAUDE.md` e o
  `user_state.ps1 -Save/-Compare`. Uma mudanca acusada e relatada, nunca "consertada" pelo agente.
- **Afirmacao na spec sem medicao vira defeito do plano.** A spec prometia F13-F24 para programas;
  o proprio ConPTY so entrega F1-F12. Medir antes de prometer (um probe descartavel de 10 minutos
  teria evitado a tarefa bloqueada).
- **Correcao que introduz regressao pior que o defeito.** Guardar o ReadLine original num closure
  criado no escopo global congelou `$PWD`: depois de `cd` a completion rodava no diretorio errado.
  A conferencia manual do coordenador nao fez um `cd`. Teste de comportamento (cwd apos
  `Set-Location`) primeiro, sempre.
- **Mutacao proposta pelo plano pode ser equivalente** (remover `"icons"` de `TOP_LEVEL_KEYS` nao
  muda nada porque o braco do `match` vem antes). Sobreviveu = teste fraco OU mutacao equivalente;
  descobrir qual e dizer.
- **O host do teste pode mascarar o defeito.** Um teste de restauracao do modo do console hospedado
  em `cmd /c` passava sem a restauracao, porque o cmd reseta o modo a cada comando.
- **Temporizador do Windows ~15,6 ms.** `timeout(5 ms)` (e ate timeout zero) dura ~16 ms. Medir a
  latencia antes de aceitar um numero do codigo.
- **Testes de ambiente.** O runner do GitHub tem `%TEMP%` em nome curto 8.3 e um console que engole
  sequencias indecodificaveis; comparar o ultimo componente do caminho e nao exigir o que o host
  pode descartar. Carga de sessoes ConPTY em paralelo estourou timeouts duas vezes, em medidas
  diferentes: num CI anterior, 19 sessoes fizeram o primeiro comando passar da espera de 15 s do
  `quiet_session` numa de 8 rodadas; no run da release v0.2.0, o primeiro relatorio de completion
  depois de `git ` + Tab passou dos 3 s do timeout do relatorio. Por isso o CI roda com 2 threads.
- **Revisor tambem erra.** Achados de revisao foram rebaixados ou ampliados por efeito (um Minor que
  apagava o `)` do usuario virou correcao); conferir na fonte antes de despachar a correcao.

## Severidade

- **Critico**: comando executado sem o usuario querer, texto do usuario perdido/corrompido,
  terminal deixado quebrado, travamento, injecao a partir de dado nao confiavel.
- **Importante**: divergencia do comportamento documentado (README, wiki, --help) num caso
  realista; panic alcancavel; regra documentada sem teste capaz de falhar.
- **Menor**: o resto (legibilidade, duplicacao, morto sem custo de runtime).

## Fluxo

1. `verify.ps1` no checkout principal: gate verde e a base. Gate vermelho para a revisao.
2. Workflow com `args: { config: <conteudo de review.json> }`.
3. Salvar `result.confirmados` em JSON e rodar `check_anchors.ps1` nele. Ancora morta =
   achado descartado, nao corrigido a mao.
4. Relatorio em `docs/reviews/<data>-revisao-geral.md` com: o que foi coberto, achados por
   severidade, derrubados com motivo, o que NAO foi coberto.

As perguntas e armadilhas de cada lente estao em `references/lentes.md`.
