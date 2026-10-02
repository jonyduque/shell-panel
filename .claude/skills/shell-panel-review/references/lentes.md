# Lentes da revisao do shell-panel

Cada lente e UMA pergunta. Um agente pedido a "revisar tudo" acha o defeito mais chamativo e
para; o resto do arquivo vira contexto lido e nao interrogado.

## correcao (por particao, worktree)

**Pergunta:** que entrada, sequencia de eventos ou estado produz resultado errado, panic,
travamento ou estado inconsistente nesta particao?

Onde procurar neste projeto:
- Fatiar `&str` por indice vindo de fora (relatorio CMP, lexer): fronteira de char UTF-8,
  indice alem do fim, unidades UTF-16 tratadas como bytes. Emoji e acentos sao o caso de teste.
- `unwrap`/`expect`/indexacao em dado que vem do PTY, do JSON do relatorio, de config ou spec.
- Reator async (`App::run`): await que bloqueia o laco de entrada; canal que enche; timeout
  que nao cancela o trabalho; estado (Tab pendente, dropdown aberto, relatorio em voo) que
  fica inconsistente quando dois eventos chegam na ordem "errada".
- Processos filhos (carapace, zoxide): timeout que nao mata, pipe que enche, PATH ausente.

Armadilhas ja pagas:
- Um cenario "construido" na cabeca e plausivel, nao confirmado. Escreva o teste de rascunho
  na worktree e cole a saida; sem saida, marque o achado como "nao construido".
- O lexer e o merge ja tem 30+ testes: antes de dizer que um caso nao e tratado, procure o
  teste que o trata.

### Armadilhas acrescentadas em 2026-10-02 (valem para correcao)
- PowerShell: `GetNewClosure()` executado no escopo global copia todas as variaveis globais
  (inclusive `$PWD` e preferencias) para o closure. Um closure que precisa ver a sessao viva tem de
  ser criado num escopo filho. Teste o comportamento (cwd apos `Set-Location`), nao a presenca.
- Reator: dois modos de entrada (`reading_line`). Procure teclas lidas num modo e processadas no
  outro (troca de modo), e escritas que terminam num ESC solto (ConPTY le como Escape).
- Windows: espera curta (`timeout`, `sleep`) dura no minimo ~16 ms; "so o que ja esta na fila" e
  um poll sem espera.

## teste (por particao, worktree)

**Pergunta:** qual teste desta particao nao consegue falhar, e qual regra declarada (README,
comentario, mensagem de commit) nao tem teste que falhe quando ela quebra?

Metodo obrigatorio: para cada regra que voce acusar, rode `mutate.ps1` no arquivo de PRODUCAO
com uma perturbacao pequena (nao um valor degenerado) e o teste que deveria pega-la. Exit 1
(SOBREVIVEU) colado e a evidencia. Exit 2 nao e evidencia de nada.

Armadilhas ja pagas:
- Mutar para o extremo (`0`, `""`, `return`) prova que a regra existe, nao que esta certa.
  `30.min(w)` -> `31.min(w)` e melhor que -> `0`.
- Assercao garantida pela linha de cima; teste cuja unica assercao e "nao deu erro";
  `wait_for_text` que casa com texto antigo ainda na tela (o e2e de Esc ja tropecou nisso).
- Teste e2e que passa porque o PowerShell fez sozinho o que o shell-panel deveria fazer
  (Tab caiu para o PSReadLine e o texto apareceu igual).

## seguranca (transversal, worktree)

**Pergunta:** que dado nao confiavel chega a uma acao com efeito (texto digitado no PTY,
processo iniciado, arquivo escrito, sequencia de terminal emitida) sem ser validado?

Fronteiras deste projeto:
- Saida do PTY: qualquer programa (`Get-Content arquivo.txt`, `curl`) pode emitir
  `ESC ] 6973;CMP;...`. O relatorio so deveria valer quando um Tab esta pendente e o
  PSReadLine esta lendo. Matches viram digitacao: controle, ESC, CSI, OSC dentro do texto.
- Descricoes/tooltips e nomes de spec desenhados no dropdown: sequencias de escape vindas
  de spec JSON ou tooltip pintam/movem o terminal do usuario.
- Argumentos passados a carapace/zoxide: a linha do usuario vira argv.
- `--verbose`: o log pode gravar linhas digitadas (senhas em `Read-Host` nao, mas linhas sim).
- `-EncodedCommand` e o script embutido; variaveis de ambiente lidas.

Armadilha: o fix 3fb09f2 ja filtra CR/LF em `shell_suggestions` e `merge_suggestions`. Leia
antes de reacusar; procure o que ele NAO cobre (ESC? outros C0? C1 0x80-0x9f? descricoes?).

## interativo (transversal, worktree)

**Pergunta:** onde o binario real, dirigido por teclas, diverge da tabela "Keys" do README,
da secao "How it works" ou do que o usuario espera?

Metodo: escreva testes de rascunho em `tests/zz_review_scratch.rs` usando `mod common;` e
`common::Terminal::shell_panel(&dir)` (veja `tests/e2e_binary_test.rs` para o padrao: espere
o prompt `PS `, envie bytes, `wait_for_text`, `screen()`). Rode com
`cargo test -q --test zz_review_scratch -- --test-threads=1`. Apague o arquivo no fim.
Primeira linha enviada a cada sessao: `Set-PSReadLineOption -HistorySaveStyle SaveNothing\r` -
sem isso o rascunho grava no historico real do usuario.

Padroes ja prontos no `tests/e2e_binary_test.rs`: `read_one_key` / `got_line` (um programa le uma
tecla com `[Console]::ReadKey` e imprime `GOT-<tecla>-<modificadores>`), `answer_queries_until`
(responde a toda pergunta DA1 como um terminal real), `warm_completion()` antes de um Tab
cronometrado, `TempDir`, `drain(quiet, max)`. No modo programa so F1-F12 chegam (limite do
ConPTY, medido).

Cenarios minimos (a classe dos 5 defeitos que escaparam):
- cada linha da tabela Keys, com dropdown aberto e fechado;
- tecla digitada entre o Tab e a chegada do relatorio (rapido, sem esperar);
- Tab no meio da linha e em linha de continuacao; texto com acento/emoji antes do cursor;
- paginar alem da ultima pagina e voltar; dropdown acima do cursor (prompt perto do fim da tela);
- Tab com zero, um e muitos matches; Esc depois de inserir;
- sair com `exit` com dropdown aberto; codigo de saida propagado;
- programa em execucao (`Start-Sleep 2`) recebendo Tab: deve ir para o programa.

Armadilhas ja pagas:
- `wait_for_text` casa com texto antigo. Espere a tela mudar antes de checar o proximo estado.
- Timeouts: o prompt leva ate 40 s para subir sob carga; use os mesmos START/STEP do e2e.
- Divergencia so e achado com a tela colada (antes/depois) do teste que rodou.

## manutencao (transversal, read-only)

**Pergunta:** o que existe e ninguem consome, o que esta escrito duas vezes sem motivo, e onde
o nome ou o comentario mente sobre o que o codigo faz?

- Morto: `find_consumers.ps1` obrigatorio. Exit 2 (so testes consomem) e candidato forte; exit 0
  nao e morto. Itens `pub` de lib usados so por testes que testam a si mesmos sao morto real.
- Restos do design antigo (worker PowerShell, screen scraping, prompt wrapper, debounce) que o
  plano mandou apagar: procure nomes, comentarios e constantes sobreviventes.
- Duplicacao: so acuse se as duas copias ja divergiram ou vao divergir (ex.: constantes de
  prefixo em theme.rs vs config.rs).

## docs (transversal, read-only)

**Pergunta:** que afirmacao do README, da wiki (`docs/wiki/`), do `--help` ou de comentario
de modulo e falsa sobre o codigo atual?

Para cada afirmacao verificavel (opcao, chave, default, tecla, caminho, limite, timeout,
mensagem de erro), abra o codigo e confira. Cite a frase literal do doc como trecho e o
`arquivo:linha` do codigo que a contradiz. Planos em `docs/superpowers/` sao historicos: nao
revise.

## Particao installer (vale para as lentes por particao)

- Nunca rode `install.ps1`/`uninstall.ps1` com os locais padrao. O teste e
  `scripts/test-installer.ps1`, sempre lancado a partir do pwsh 7, com `-Shell pwsh` e
  `-Shell powershell`; ele usa pastas temporarias e restaura o PATH. Rode `user_state.ps1 -Save`
  antes e `-Compare` depois de cada execucao.
- So UM teste do instalador por vez na maquina. O proprio `scripts/test-installer.ps1` garante
  isso com o mutex `Global\shell-panel-installer-test` (um segundo espera e desiste sem tocar o
  PATH); mesmo assim, nunca o rode a partir de agentes em paralelo.
- Prova de mutacao: `mutate.ps1 -Path install.ps1 ... -TestCommand 'pwsh -NoProfile -File
  scripts/test-installer.ps1 -Shell pwsh'` (o teste imprime `test result:`).
- Verifique os dois modos de execucao: arquivo (`-File`, parametros por nome, `-Switch:$false`) e
  `irm | iex` (nada pode sobrar na sessao: variavel, funcao, `$ErrorActionPreference`, StrictMode).
- Os .ps1 sao ASCII puro; emoji e ESC sao montados em tempo de execucao.
- Workflows: `actionlint`; o publish tem de ser repetivel (release ja existente).
