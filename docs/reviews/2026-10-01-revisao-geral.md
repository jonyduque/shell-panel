# Revisão geral do shell-panel: relatório consolidado

> **Execução:** workflow `.claude/workflows/shell-panel-review.js`, run `wf_6a3690b0-aeb`, 2026-10-01, base `562e3e0`.
> 12 lentes (correção e teste por partição; segurança, interativo, manutenção e docs transversais), 26 refutadores e 1 síntese: 39 agentes, nenhum com erro.
>
> **Auditoria do orquestrador, antes de aceitar:**
> - `check_anchors.ps1` sobre os 46 achados brutos: 46 âncoras vivas e 0 mortas. Uma estava deslocada: o trecho de I9 também aparece em `app.rs:172`, mas o achado é o de `:244`.
> - Nenhum refutador derrubou um achado (26 de 26 confirmados). Conferi a evidência dos 4 Críticos e de I5 e I9: todas têm saída colada de execução real, o refutador reproduziu cada uma de forma independente na própria worktree, e C1 e I9 têm controle negativo.
> - Os 19 Menores não passaram por refutação. Só a âncora deles foi conferida.
> - Os achados brutos em JSON estão em `2026-10-01-revisao-geral.json`.


Base: `C:/Users/jonyd/Projetos/shell-panel`, commit `562e3e0`. Os caminhos abaixo são relativos à raiz do repositório.

## Resumo

| | Quantidade |
|---|---|
| Achados que chegaram ao consolidador | 46 (26 bloqueantes refutados + 20 Menores sem refutação) |
| Fundidos por duplicata | 4 (relatório atrasado; panic de cor; `ConPtySession::resize`; delimitadores do lexer) |
| **Achados únicos** | **42** |
| Críticos (todos CONFIRMADOS) | 4 |
| Importantes (todos CONFIRMADOS) | 19 |
| Menores | 19 (1 confirmado na refutação e rebaixado para Menor; 18 sem refutação, com âncora a conferir mecanicamente) |
| Não verificados (limite de refutação atingido) | 0 |
| Derrubados na refutação | 0 |
| Unidades sem resultado | 0 |

Por tipo, entre os Críticos e Importantes:
- **Defeito em produção:** 4 Críticos e 7 Importantes. Os Importantes são o Tab no meio da palavra, a quebra de linha no lexer, o carapace sem aspas, Alt+Enter, o panic de cor, o escape no tooltip e a sequência ESC no dropdown.
- **Regra documentada sem teste capaz de falhar:** 12 Importantes.

Achados que atravessam partições estão marcados com **⇄**.

---

## Críticos

### C1. Um relatório CMP atrasado, pedido por um Tab anterior, é aceito como resposta do Tab atual (texto corrompido, Tab duplicado, comando roda alterado) ⇄
- **Local:** `src/core/app.rs:213`
- **Lentes:** correcao:core, interativo. São duas descobertas independentes, fundidas aqui.
- **Atravessa:** core (reator) ↔ shell (`assets/shellIntegration.ps1:39-47`, onde o acorde é tratado em fila pelo PSReadLine).
```rust
                        // Only the answer to the latest Tab is wanted.
                        if report_deadline.take().is_some() {
```
- **Cenário:** o reator não sabe a qual pedido um relatório responde. Qualquer relatório que chega com deadline pendente é tomado como a resposta do Tab mais recente. O `generation` só é capturado quando o relatório chega (linha 220), então o filtro da linha 227 não descarta o relatório velho. Basta um completer lento, coisa que o README admite (até 3 s):
  - `f -p al`, Tab, 500 ms, `x`, 300 ms, Tab (completer de 1,5 s) → `f -p alphaxpha`. O Enter roda com `$p = alphaxpha`; o pwsh puro roda `alphax`.
  - `zzf z`, Tab, `q`, Tab (completer de 2,5 s) → `zzf zzalphaqzalpha`.
  - Tab, timeout de 3 s, Tab de novo (completer de 4 s) → `zzf zzalphazalpha`: a completion entra duas vezes.

  O relatório certo chega depois e é descartado, porque o deadline já foi consumido.
- **Evidência:** quatro reproduções com rascunhos e2e (já apagados). Linhas decisivas:
  ```
  PS ...sp_zz_slow0_32340> f -p alphaxpha
  GOT=alphaxpha
  PS ...sp_zz_pwsh0_32340> f -p alphax          (pwsh puro)
  FINAL LINE B: "...> zzf zzalphazalpha   "     panicked: stale report applied
  ```
  Controle: com N=4000 na variante A, o deadline do Tab2 vence antes e não há corrupção. Isso confirma que o defeito depende da janela de tempo. Rajadas `\tq\t` e `\t\x7f\t` reproduziram 3 vezes em 6 execuções.
- **Proposta:** casar pedido e resposta com um contador `reports_outstanding`: +1 a cada `REPORT_REQUEST_KEY`, −1 (saturando) a cada `OscEvent::Report`. O relatório só é aceito com o contador em 0 e deadline pendente. O contador é zerado em `ReadLineStarted` e no timeout. Adicionar um e2e com completer lento (Tab, tecla, Tab) esperando `alphax`.
- **Relacionado:** C3 e I8 tocam a mesma guarda (`app.rs:211-213`).

### C2. As mensagens OSC 6973 saem na code page OEM do console: fora dela, os caracteres viram `?` e o Tab insere um curinga no lugar do nome real ⇄
- **Local:** `assets/shellIntegration.ps1:5`
- **Lente:** correcao:shell-pty
- **Atravessa:** shell (script) → core (`app.rs:235`, que aplica o match único) → engine (as linhas dos providers chegam corrompidas).
```powershell
    [regex]::Replace($value, '[\x00-\x1f\x7f\\;]', { param($match)
```
- **Cenário:** Windows pt-BR (OEM 850), configuração de fábrica. Num diretório que só tem `日本.txt`, digitar `Get-Item .\` e Tab insere `.\??.txt`. O `__SP-Escape` só escapa C0, DEL, `\` e `;`. O resto passa cru por `[Console]::Write` (linha 11) em `OutputEncoding` (CP=850, medido dentro da sessão), e o .NET troca por `?` o que a code page não tem. O `?` é curinga: `Remove-Item .\` + Tab + Enter apagaria todo `.txt` de 2 caracteres. A mesma corrupção atinge a linha do relatório (o texto vai `????` para specs, carapace e zoxide) e o cwd do RS. Afeta pwsh e powershell.exe. O e2e atual usa `ação`, que cabe na CP850. Na OEM 437 (en-US) até `ação` cai fora da code page.
- **Evidência:**
  ```
  SHELL=Pwsh CP=850   line="echo '????' > $sp_rep_"
  pwsh.exe: OSC CMP;{...,"matches":[[".\x5c\x5c??.txt","??.txt",...
  PS ...sp_zz_cjk2_32428> Get-Item .\??.txt
  ```
  A lente testou a correção proposta (regex trocada e depois revertida): o Tab deu `Get-Item .\日本.txt`, e shell_report_test, e2e_pty_test e osc_test continuaram verdes. O refutador não testou a correção. Ressalva: um Tab enviado sem pausa caiu no caminho do Tab simples.
- **Proposta:** payload 100% ASCII, sem mexer na `OutputEncoding` do usuário. Regex `'[\ud800-\udbff][\udc00-\udfff]|[^\x20-\x3a\x3c-\x5b\x5d-\x7e]'`, com escape `\xHH` dos bytes UTF-8 (o par substituto vai inteiro); o `unescape_value` do Rust já decodifica. Adicionar um e2e com nome CJK ou emoji.

### C3. Um relatório CMP forjado na saída de um programa vira texto digitado: um RS falso liga `reading_line`, e o próximo Tab aceita o primeiro CMP que chegar ⇄
- **Local:** `src/core/app.rs:211`
- **Lente:** seguranca
- **Atravessa:** shell (`src/shell/command_state.rs:18-19` e `osc.rs`, sem autenticação) ↔ core.
```rust
if let Some(report) = command_state.report.take() {
                        // Only the answer to the latest Tab is wanted.
                        if report_deadline.take().is_some() {
```
- **Cenário:** um comando comum imprime dado não confiável, por exemplo `1..250 | % { Get-Content forged.txt; Start-Sleep -Milliseconds 40 }`. O arquivo contém `ESC]6973;RS;BEL` + `ESC]6973;CMP;{...matches:[["echo INJECTED-FROM-FILE",...]]}BEL`. O RS forjado liga `reading_line` no meio do comando. O usuário aperta Tab: o shell-panel manda o acorde e arma o deadline, e o CMP forjado seguinte é aceito. Com um match único, ele é escrito no PTY junto com `replacementLength` Backspaces. O Tab do usuário se perde, o texto do atacante aparece no próximo prompt e o texto que o usuário digitou adiantado é apagado. O `is_insertable` bloqueia CR/LF, então o comando não roda sozinho, mas fica pronto para um Enter. O commit 3fb09f2 já trata "a spoofed CMP report" como ameaça e só filtrou CR/LF.
- **Evidência:**
  ```
  LOOPDONE
  PS ...sp_zz_file_19396> echo INJECTED-FROM-FILE
  INJECTED ON PROMPT: true
  CONTROL INJECTED ON PROMPT: false      (mesmo arquivo, sem Tab)
  ```
  A lente também reproduziu a variante com processo em background, que apagou o `Get-ChildIt` digitado. O refutador não reproduziu essa variante.
- **Proposta:** token aleatório por execução, embutido no script antes do `-EncodedCommand` e nunca em variável de ambiente. As mensagens passam a ter a forma `6973;<token>;RS|RE|CMP;...`, e `parse_osc_sequence` descarta as que vêm sem token. Teste em `tests/stream_test.rs`: um CMP sem token, com Tab pendente, não gera `CompletionOutcome`.
- **Relacionado:** C1 (mesma guarda) e I8 (a guarda não tem teste).

### C4. Caracteres fora do BMP (😀, 🐛), digitados ou colados, somem antes de chegar ao PowerShell, e a linha roda sem eles ⇄
- **Local:** `src/core/app.rs:163`
- **Lente:** interativo
- **Atravessa:** io (entrada pelo crossterm) → core.
```rust
        let mut event_stream = crossterm::event::EventStream::new();
```
- **Cenário:** `git commit -m "🐛 fix"` grava a mensagem sem o emoji, sem nenhum erro. Caracteres do BMP (`ação`) passam intactos. **Causa provável, não verificada:** no crossterm 0.28.1 (o 0.29.0 é igual), `src/event/sys/windows/parse.rs:54-58` junta surrogates sem olhar `key_down`, e o ramo Surrogate (linhas 271-272) vem antes do teste de `key_down`. Se a ordem for down(alta), up(alta), down(baixa), up(baixa), os dois pares saem inválidos e nenhum `KeyEvent` é gerado. O `encode_key_event` codificaria o char certo se ele chegasse.
- **Evidência:**
  ```
  shell-panel: PS ...> 'XY'.Length; 'MARK' + 'B'    → 2
  pwsh puro:   PS ...> 'X😀Y'.Length; 'MARK' + 'B'  → 4
  shell-panel: 'A😀😀B' chega como 'AB' → 2   |  pwsh puro → 6
  ```
- **Proposta:** juntar surrogates só a partir de registros `key_down`, com um `[patch.crates-io]` do crossterm ou lendo os `INPUT_RECORD` via `crossterm_winapi` só para esse caso. Antes de corrigir, confirmar a ordem dos registros com um log de `KeyEventRecord`. Adicionar um e2e em que `'X😀Y'.Length` imprime 4.

---

## Importantes

### I1. Tab com o cursor no meio de uma palavra, aceitando uma sugestão de spec, carapace ou zoxide, duplica o resto da palavra ⇄
- **Local:** `src/engine/aggregate.rs:177`
- **Lente:** correcao:engine
- **Atravessa:** engine ↔ core (`app.rs:235` aplica o match único sem confirmação).
```rust
        _ => calculate_replacement(active_token_raw(&report.line[..cursor]), &suggestion.name),
```
- **Cenário:** na linha `git checkout`, com o cursor depois de `chec`, o Tab produz `git checkout kout`. Sugestões sem `uses_shell_range` só olham `line[..cursor]` e usam `delete_count: 0`. O mesmo acontece com zoxide (`cd proj|etos`) e carapace. Isso contradiz README.md:63 ("also in the middle of a line").
- **Evidência:** `action: ReplacementAction { backspace_count: 0, delete_count: 0, insert_text: "kout " }` / `line after: "git checkout kout"`. Reproduzido também em e2e no binário.
- **Proposta:** apagar também a cauda do token depois do cursor, usando `replace_range` quando houver range, ou achando o fim do token com os mesmos delimitadores de `active_token_raw`. Teste em aggregate_test: `report("git checkout", 8, 4, 8, [])` com a sugestão externa `checkout` deve dar `delete_count` 4.

### I2. O lexer não trata quebra de linha como espaço nem como separador de comando, e quebra specs e zoxide em linhas de continuação ⇄
- **Local:** `src/engine/lexer.rs:201` (também `:271`; o lado divergente está em `:321`)
- **Lentes:** correcao:engine e manutencao (o Menor "delimitadores duplicados em split_segments e active_token_raw já divergiram" foi fundido aqui).
- **Atravessa:** engine ↔ docs (README.md:63).
```rust
        while i < len && (chars[i] == ' ' || chars[i] == '\t') {
```
- **Cenário:** no buffer `if ($true) {\ngit sta`, os tokens viram `["\ngit","sta"]`, `can_handle("\ngit")` dá falso e o Tab fica sem a completion do git. Em `Get-Location\ngit sta`, a raiz vira `Get-Location\ngit`. Em `cd C:\\\ngit `, o zoxide responde na linha do git. Enquanto isso, `active_token_raw` (`:321`) já trata `\n`/`\r` como delimitador. Casos a mais trazidos pela lente manutencao, sem refutação: `git status & git ch`, onde o lexer mantém `&` como token e o json_spec devolve `[]`; e o doc de `split_segments` (`lexer.rs:104`), que não menciona `(` e `{`, separados na linha 149.
- **Evidência:**
  ```
  "if ($x) {\ngit sta" -> lex ["\ngit", "sta"]
  "if ($x) {\n  git sta" -> engine []
  continuation completed=false   (e2e no binário; na mesma linha única: git status)
  ```
- **Proposta:** em `split_segments`, tratar `\n`/`\r` fora de aspas como separador de segmento. `|` e o backtick já são tratados. Em `lex_segment`, tratar `\r`/`\n` como espaço. Unificar os delimitadores numa só constante usada pelas duas funções e corrigir o doc. Testes: `texts("if ($x) {\n  git sta") == ["git","sta"]` e `texts("Get-Location\ngit sta") == ["git","sta"]`.

### I3. Os valores do carapace entram sem aspas: um caminho com espaço vira dois argumentos
- **Local:** `src/engine/providers/carapace.rs:91`
- **Lente:** correcao:engine
```rust
            Suggestion::new(item.value, display, description, 70).with_kind(kind)
```
- **Cenário:** `git -C My` + Tab produz `git -C My Dir/`. Pior: o carapace marca o valor como Subcommand, o que esconde a sugestão do PowerShell (`'.\My Dir\'`, prioridade 60), e o `dedupe_key` funde as duas. Sobra só a versão sem aspas. `;`, `$`, `(` e o backtick passam do mesmo jeito.
- **Evidência:** saída real do carapace: `"values":[{"value":"My Dir/",...}]`. Depois do merge: `merged: "My Dir/" prio 70 kind Subcommand`, `line after accept: "git -C My Dir/"`. Ressalva: nesta máquina o carapace levou de 2,4 a 10,7 s e sempre passou do timeout de 200 ms, então não houve reprodução end-to-end no binário.
- **Proposta:** levar `zoxide::quote_for_powershell` para um módulo comum e aplicar em `name`, mantendo `display` sem aspas. O `trailing_space` já remove as aspas antes de olhar a `/`. Teste: `"My Dir/"` deve virar `"'My Dir/'"`.
- **Relacionado:** M9 (dedupe de nome entre aspas).

### I4. Alt+Enter vira um CR puro e executa a linha; no PowerShell nativo nada acontece ⇄
- **Local:** `src/io/key_event.rs:115`
- **Lente:** correcao:io-ui-vt
- **Atravessa:** io ↔ core (`handle_key`, `app.rs:334-337` e `:352`).
```rust
                (false, false) => vec![b'\r'],
```
- **Cenário:** o `match (shift, ctrl)` ignora ALT. Num terminal que repassa o acorde (VS Code, ou WT sem o atalho de tela cheia), Alt+Enter roda a linha. Com o dropdown aberto, o Passthrough fecha o dropdown e manda o `\r`. Isso contradiz a tabela Keys do README ("Passed to PowerShell").
- **Evidência:** com o registro win32 `\x1b[13;28;13;1;2;1_`: `[pwsh win32] executed = false` / `[shell-panel win32] executed = true` → `zz2mark`. A parte da evidência original com `\x1b\r` **não** reproduziu no refutador (`shell-panel esc-cr executed = false`) e não deve ser citada.
- **Proposta:** incluir ALT no registro win32 do Enter, com `(false,false,false) => \r` e +0x02 (LEFT_ALT_PRESSED) no estado. Adicionar um assert em io_test: `encode_key_event(Enter, ALT) != b"\r"`.

### I5. Uma cor hex com caractere não ASCII no config derruba o startup com panic, ao contrário da wiki, que promete fallback silencioso ⇄
- **Local:** `src/ui/color.rs:44` (e `docs/wiki/Configuration-Colors.md:85`)
- **Lentes:** correcao:io-ui-vt e docs. São duas descobertas independentes, fundidas aqui.
- **Atravessa:** ui ↔ core (`Theme::from_config` ← `App::new`, `app.rs:101`) ↔ docs.
```rust
            let r = u8::from_str_radix(&hex_part[0..1], 16).ok()?;
            let g = u8::from_str_radix(&hex_part[1..2], 16).ok()?;
```
```
A color value shell-panel cannot understand — a misspelt name, an index above 255, a hex string of
the wrong length — produces **no warning**. The key behaves as if it were empty:
```
- **Cenário:** com `selected_bg = "#aé"` ou `"#éa"`, o `hex_part.len()` conta bytes e o fatiamento cai no meio do `é`, gerando um panic antes do raw mode. O terminal não fica quebrado: o `?25h` é emitido. No ramo de 6 bytes, o panic só acontece quando a fronteira de um caractere cai no índice 2 ou 4 (`#aaaéa`).
- **Evidência:** `panicked at src\ui\color.rs:44:49: end byte index 1 is not a char boundary; it is inside 'é'` / `exit=101`. A outra reprodução deu o mesmo erro em `:45:49`.
- **Proposta:** antes de fatiar, `if !hex_part.bytes().all(|b| b.is_ascii_hexdigit()) { return None; }`. Opcionalmente, um aviso de cor inválida em `Config::load`. Casos em color_test: `"#a\u{e9}"`, `"#éa"` e `"#a\u{e9}aaa"`.

### I6. Tooltip, ListItemText e descrições de carapace e spec chegam crus ao terminal: ESC e OSC no dado pintam e movem a tela, e o efeito fica depois do Esc ⇄
- **Local:** `src/ui/theme.rs:160`
- **Lente:** seguranca
- **Atravessa:** origem no engine (`aggregate.rs:79`, `carapace.rs:81-85`) → destino na ui (`theme.rs:160/167`, `renderer.rs:74-83`).
```rust
    let label = format!("{}{}{}", prefix, icon, sug.display);
```
- **Cenário:** `$j = ... | ConvertFrom-Json` com um valor que contém `ESC[1;1HPWNED`, seguido de `$j.a<Tab>`. O ToolTip leva o valor cru, o `truncate_to_width` dá largura 0 ao ESC e o mantém, e o renderer escreve tudo no stdout. Um OSC 52 vindo do carapace pode gravar no clipboard. O `clear_dropdown` só restaura as linhas do dropdown. A limitação do README sobre "raw control bytes" não se aplica, porque o ConvertTo-Json escapa o ESC como `\u001b` e o serde o decodifica de volta.
- **Evidência:** `row0 after tab: "PWNED Users\\jonyd\\..."` / `row0 after esc: "PWNED Users..."`. Na formatação: `"\u{1b}[7m> 🔹 main  \u{1b}]52;c;aWV4\u{7}fix ..."`. O refutador manteve Importante e não Crítico: o dado hostil precisa estar carregado na sessão, e o PowerShell exporia os mesmos bytes ao imprimir `$j`.
- **Proposta:** neutralizar em display e description todo char < 0x20, 0x7f e C1 0x80-0x9f, ao construir o `Suggestion` ou ao formatar. Teste em theme_test: `\x1b` e `\x07` não podem aparecer na linha formatada.

### I7. Nenhum teste falha se o RawModeGuard parar de mostrar o cursor na saída
- **Local:** `src/io/raw_mode.rs:20`
- **Lente:** teste:io-ui-vt
```rust
            let _ = std::io::Write::write_all(&mut out, b"\x1b[?25h");
```
- **Cenário:** com `?25h` trocado por `?25l`, sair com `exit` sem dropdown deixa o cursor escondido. O único teste do guard (`tests/io_test.rs:136-140`) não tem asserção. A regra do projeto diz que "Raw mode e cursor precisam voltar ao normal em toda saida".
- **Evidência:** `mutate.ps1` contra a suíte inteira em série: `[!] SOBREVIVEU ... (20 binarios, todos ok)` / `EXIT=1`.
- **Proposta:** no e2e de saída, depois de `wait_exit`, conferir que a última ocorrência de `\x1b[?25` em `term.raw` é `?25h`. Outra opção: Drop com um `W: Write` injetável e um teste unitário dos bytes.

### I8. Nenhum teste falha se um relatório CMP chegar sem Tab pendente
- **Local:** `src/core/app.rs:213`
- **Lente:** teste:core
```rust
                        if report_deadline.take().is_some() {
```
- **Cenário:** com a guarda desligada (`|| true`), um CMP impresso por qualquer programa vira digitação. Essa guarda é a única entre o PTY e o `engine.complete`.
- **Evidência:** a mutação sobrevive à suíte inteira em série: `[!] SOBREVIVEU ... e2e_binary_test 5 passed (59.81s)`. Contra o rascunho proposto: `[OK] MORTA ... assertion failed: !typed`. Na base o rascunho passa (`typed=false`).
- **Proposta:** e2e que imprime `ESC]6973;CMP;{...ZZFOR+GED...}BEL` sem Tab, espera `DONE-MARK` mais 3 s e confirma que `ZZFORGED` não aparece fora do eco do comando. Validado na base e contra a mutação.
- **Relacionado:** C1 e C3. A correção de qualquer um deles muda essa guarda.

### I9. O fallback de 3 s para o Tab do PowerShell não tem teste: remover o `\t` do timeout passa em todos os e2e
- **Local:** `src/core/app.rs:245`
- **Lente:** teste:core
```rust
                    tab_pending = false;
                    write_to_pty(&mut pty_writer, b"\t");
```
- **Cenário:** README:14 e Known limitations:126. Sem o write, o Tab se perde quando o acorde não responde.
- **Evidência:** a mutação sobrevive a e2e_binary_test e à suíte inteira (`EXIT=1`). O rascunho completa na base em 3,32 s e mata a mutação: `assertion failed: ok`.
- **Proposta:** e2e com `Remove-PSReadLineKeyHandler -Chord 'Ctrl+Alt+Shift+F12'`, `echo zz_uni` + Tab, esperando `zz_unique_file.txt` (opcionalmente com tempo abaixo de 6 s).

### I10. Repor o Tab retido quando uma tecla chega antes do relatório não tem teste ligado ao reator (a regressão do 5dc77fe passaria)
- **Local:** `src/core/app.rs:270`
- **Lente:** teste:core
```rust
                            write_to_pty(&mut pty_writer, withheld_tab_bytes(tab_pending, key_event.code));
```
- **Cenário:** com `withheld_tab_bytes(false, ...)`, `echo zz_uni` + `\tQ` vira `echo zz_uniQ`. Isso viola a regra "Tab nunca pode ser perdido". Só a função pura tem teste (`io_test.rs:101-109`).
- **Evidência:** a mutação sobrevive a e2e_binary_test e à suíte inteira. O rascunho dá `echo .\zz_unique_file.txtQ` na base e mata a mutação.
- **Proposta:** e2e que envia `b"\tQ"` numa única escrita e espera `zz_unique_file.txtQ`.

### I11. `test_tab_outside_psreadline_is_a_plain_tab` não detecta o acorde chegando ao Read-Host
- **Local:** `tests/e2e_binary_test.rs:200`
- **Lente:** teste:core
```rust
    // Read-Host may keep or drop the Tab, but the reserved chord (`[24;8~`) must never reach it.
```
- **Cenário:** com `(command_state.reading_line || true)` em `app.rs:344`, o acorde chega ao Read-Host, que ignora tecla sem caractere. O `$v` ainda casa com `^a\s*b$` e o teste passa (README:14).
- **Evidência:** a mutação sobrevive ao teste isolado e à suíte inteira em série. Com o rascunho que usa `[Console]::ReadKey`, a mutação morre. **Ressalva do refutador:** na base, o rascunho falhou uma vez logo depois de um build frio (`GOT-F12-Alt, Shift, Control`), porque depende do sleep fixo de 2 s.
- **Proposta:** trocar o Read-Host por `[Console]::ReadKey($true)` e esperar `GOT-Tab-None`, usando como sinal um marcador impresso antes do ReadKey, não um sleep fixo.

### I12. O exit 2 para shell não suportado (`--shell` ou chave `shell`) não tem teste
- **Local:** `src/main.rs:92`
- **Lente:** teste:core
```rust
            std::process::exit(2);
```
- **Cenário:** a wiki (Configuration.md:117, Troubleshooting:78) documenta o exit 2. Trocar para 1 passa despercebido.
- **Evidência:** a mutação `exit(2)` → `exit(1)` sobrevive a `cargo test -q` inteiro: `[!] SOBREVIVEU ... (20 binarios, todos ok)`.
- **Proposta:** em cli_test, com `run_with_session_env`, testar `--shell bash` e um `--config` com `shell = "bash"`: `code()==Some(2)` e o stderr contendo `unsupported shell "bash"`.

### I13. O padrão documentado "pwsh.exe quando está no PATH" não tem teste capaz de falhar: `test_detect_shell_auto` é tautológico
- **Local:** `tests/pty_test.rs:41`
- **Lente:** teste:shell-pty
```rust
    assert!(shell == ShellType::Pwsh || shell == ShellType::Powershell);
```
- **Cenário:** uma regressão em `src/pty/shell.rs:45` abre o powershell.exe 5.1 sem aviso (README.md:70, Configuration.md:112).
- **Evidência:** a mutação `pwsh.exe` → `pwsh.com` sobrevive a pty_test e ao e2e_pty_test isolado. Na suíte inteira em paralelo ela "morre" só por timeout de carga do 5.1. Esse resultado não conta como cobertura.
- **Proposta:** extrair `detect_shell_in(path_var)` e testar com um diretório temporário que contém um `pwsh.exe` vazio (espera Pwsh) e com um PATH sem ele (espera Powershell).

### I14. Nenhum teste vê `SHELL_PANEL_SESSION=1` dentro da sessão: a recusa de sessão aninhada (README:50) pode quebrar sem nenhum teste falhar
- **Local:** `src/pty/conpty.rs:39`
- **Lente:** teste:shell-pty
```rust
        cmd.env(SESSION_ENV, "1");
```
- **Cenário:** com `"0"`, `shell-panel` e `--check` dentro de uma sessão deixam de ser recusados. O cli_test injeta a variável à mão.
- **Evidência:** a mutação sobrevive à suíte inteira em série, sem `--skip`: `[!] SOBREVIVEU ... EXIT=1`.
- **Proposta:** num teste que use `ConPtySession::spawn` ou `Terminal::shell_panel` (o helper de shell_report_test tem um `CommandBuilder` próprio e não cobre a linha 39), enviar `"SPS=$env:SHELL_PANEL_SESSION"` e esperar `SPS=1`. Outra opção: rodar `shell-panel --check` dentro do binário e esperar `RC=0`.

### I15. Metade da regra "o intervalo contém o cursor" de `replacement_range` não tem teste, e é essa metade que evita um panic em `plan_replacement` ⇄
- **Local:** `src/shell/report.rs:43`
- **Lente:** teste:shell-pty
- **Atravessa:** shell ↔ engine (`aggregate.rs:173`).
```rust
        (start <= cursor && cursor <= end).then_some((start, end))
```
- **Cenário:** sem `start <= cursor`, um relatório com o intervalo depois do cursor (forjado ou inesperado) faz `plan_replacement` fatiar `[4..3]` e entrar em panic.
- **Evidência:** a mutação sobrevive a osc, aggregate, shell_report, engine e a todos os 15 binários não-e2e. Com o rascunho, `panicked at src\engine\aggregate.rs:173:25`, e a mutação morre.
- **Proposta:** em `test_report_ranges`, um intervalo que começa depois do cursor deve dar `None`. Opcionalmente, o mesmo caso via `plan_replacement` em aggregate_test.

### I16. A regra do README "spec do usuário com o mesmo name substitui o embutido" não tem teste que falhe
- **Local:** `src/engine/providers/json_spec.rs:120`
- **Lente:** teste:engine
```rust
        self.specs.insert(spec.name.clone(), spec);
```
- **Cenário:** com `entry().or_insert`, o `git.json` do usuário seria ignorado (README.md:101, doc em `json_spec.rs:133`).
- **Evidência:** `[!] SOBREVIVEU ... test result: ok. 1/8/13/15/6 passed` / `EXIT=1`.
- **Proposta:** em `test_load_dir_adds_user_specs_and_reports_bad_files`, gravar um `git.json` com `onlymine` e afirmar que `complete("git ", "") == ["onlymine"]`.

### I17. Nas aspas de caminhos do zoxide só espaço e apóstrofo são testados; `;` pode sair sem aspas sem nenhum teste falhar
- **Local:** `src/engine/providers/zoxide.rs:39`
- **Lente:** teste:engine
```rust
        .any(|c| c.is_whitespace() || "'\"`$(){};,&@#|<>".contains(c));
```
- **Cenário:** sem o `;`, `C:\a;b` entra cru, e com Enter roda `cd C:\a` seguido de `b`. Nenhum dos 14 metacaracteres restantes tem teste.
- **Evidência:** a mutação sobrevive aos testes do engine (`EXIT=1`). As "mortes" no e2e trocam de teste a cada execução (falhas de inicialização) e são ruído.
- **Proposta:** afirmar aspas para `C:\a;b`, `C:\x$y`, `C:\a&b` e `C:\p(1)`.

### I18. A regra documentada "`max_suggestions = 0` vale 5" não tem teste; sem a guarda, o dropdown fica aberto e invisível ⇄
- **Local:** `src/ui/suggestion_state.rs:30`
- **Lente:** teste:io-ui-vt
- **Atravessa:** ui ↔ core (`AcceptSuggestion` usa `is_open()`).
```rust
            max_rows: if max_rows == 0 { 5 } else { max_rows },
```
- **Cenário:** wiki Configuration.md:95 e Reference.md:10. Com a guarda trocada para `{ 0 }`, ficam `visible=true` e `layout=None`, e Enter ou Tab inserem uma sugestão que o usuário não viu.
- **Evidência:** `{ 0 }` sobrevive a `cargo test -q` inteiro. O rascunho imprimiu `RAW0 visible=true layout=None active=Some("s0")` e mata a mutação.
- **Proposta:** em renderer_test, `SuggestionState::new(0).max_rows == 5` e `render_dropdown` com `new(0)` e 3 itens devolvendo `row_count` 3.

### I19. O teste de borda direita só usa o ícone ASCII de Other; a largura de emoji (ícones padrão) em `truncate_to_width` não é testada
- **Local:** `src/ui/theme.rs:254`
- **Lente:** teste:io-ui-vt
```rust
        let w = c.width().unwrap_or(0);
```
- **Cenário:** os ícones padrão (📁 📄 ⚡ 🔹) têm largura 2. Contados como 1, a linha passa da borda, quebra e escreve sobre a tela do usuário (Configuration-Colors.md:36).
- **Evidência:** `.min(1)` sobrevive a `--lib` e aos 15 alvos não-e2e. Os dois rascunhos (Directory com `cursor_x=70`, e `truncate_to_width("📁ab",3)=="📁a"`) passam na base e matam a mutação.
- **Proposta:** duplicar o teste de borda com `.with_kind(SuggestionKind::Directory)` e adicionar o teste direto de `truncate_to_width`.
- **Relacionado:** M1 (mesma função, defeito diferente).

---

## Menores

Exceto M19, que foi confirmado e rebaixado, os Menores não passaram pela refutação. A âncora de cada um ainda será conferida mecanicamente.

### M1. `truncate_to_width` mede caractere a caractere e deixa a linha de opção selecionada uma coluna além de `max_width`
- **Local:** `src/ui/theme.rs:174`
- **Lente:** correcao:io-ui-vt
```rust
        let truncated = truncate_to_width(&plain, max_width);
```
- **Cenário:** o ícone de opção padrão `🏷️  ` (U+1F3F7 + VS16) mede 2 por `str.width()` e 1 pela soma char a char. As linhas de Option truncadas saem com `max_width+1`. Num WT com grapheme clustering haveria autowrap e sobra na linha de baixo. O efeito no terminal real não foi construído.
- **Evidência:** `max=40 selected=true visible_width=41`.
- **Proposta:** cortar pela largura de `str` (acumulando por grapheme) e adicionar uma asserção de largura `<= max_width` em theme_test.
- **Relacionado:** I19.

### M2. A asserção `git status` do e2e é satisfeita pela predição inline do PSReadLine, e o harness grava no histórico real do usuário
- **Local:** `tests/e2e_binary_test.rs:45`
- **Lente:** teste:core
```rust
        term.wait_for_text("git status", STEP),
```
- **Evidência:** sem Tab, `contains 'git status' = true`. A mutação do match único sobrevive ao teste isolado. Há 58 cópias de `$sp_e2e_var_zz = 1` em `ConsoleHost_history.txt`.
- **Proposta:** no harness, `Set-PSReadLineOption -PredictionSource None -HistorySaveStyle SaveNothing` e esperar uma mudança que a predição não produz.

### M3. `test_default_sample_toml_validity` não detecta amostra divergente dos defaults nem chave com erro de digitação
- **Local:** `tests/config_test.rs:84`
- **Lente:** teste:core
```rust
    let parsed: Result<Config, _> = toml::from_str(sample);
```
- **Evidência:** `selected_gb` e `max_suggestions = 6` sobrevivem (`EXIT=1`).
- **Proposta:** `Config::load` sobre a amostra, com `config == Config::default()` e `warnings.is_empty()`.
- **Relacionado:** M4 (a mesma correção cobre os dois).

### M4. As listas de chaves conhecidas de `shell` e `[icons]` não têm teste: uma config válida passaria a gerar aviso
- **Local:** `src/core/config.rs:208`
- **Lente:** teste:core
```rust
const TOP_LEVEL_KEYS: &[&str] = &["max_suggestions", "shell", "colors", "icons"];
```
- **Evidência:** a remoção de `shell` e a troca `"icons" => COLOR_KEYS` sobrevivem.
- **Proposta:** a mesma de M3.

### M5. O terminador ST dividido exatamente no ESC entre chunks não tem teste
- **Local:** `src/shell/stream.rs:61`
- **Lente:** teste:shell-pty
```rust
                if offset == body.len() - 1 {
```
- **Evidência:** `body.len()` sobrevive a mutação. O rascunho com os chunks `a\x1b]6973;RE\x1b` | `\\b` mata a mutação. O impacto é pequeno porque o script termina com BEL.
- **Proposta:** caso em stream_test com `first == b"a"`, `second == b"b"` e `!reading_line`.

### M6. `ConPtySession::resize` só é usado por teste: o reator reimplementa o resize inline, e o teste só confere `is_ok` ⇄
- **Local:** `src/pty/conpty.rs:60` / `src/core/app.rs:262`
- **Lentes:** teste:shell-pty e manutencao (fundidos).
- **Atravessa:** pty ↔ core.
```rust
    pub fn resize(&self, cols: u16, rows: u16) -> Result<()> {
```
```rust
                            let _ = pair.master.resize(PtySize { rows: new_rows, cols: new_cols, pixel_width: 0, pixel_height: 0 });
```
- **Evidência:** rows e cols trocados dentro do método sobrevivem a pty_test. A única chamada é `tests/pty_test.rs:49`.
- **Proposta:** usar `session.resize` no reator, ou apagar o método e o teste. Testar uma sessão real esperando `SZ=120x40`.

### M7. O delimitador `&&`, declarado no doc do lexer, não tem teste fora de aspas
- **Local:** `src/engine/lexer.rs:144`
- **Lente:** teste:engine
```rust
                } else if ch == '&' && i + 1 < len && chars[i + 1].1 == '&' {
```
- **Evidência:** `&` → `%` sobrevive. Efeito: em `cargo build && git sta` a raiz vira `cargo`.
- **Proposta:** `texts("cargo build && git sta") == ["git","sta"]`.

### M8. O desempate "fonte externa vence com prioridade e nome iguais" do merge não é testado
- **Local:** `src/engine/aggregate.rs:126`
- **Lente:** teste:engine
```rust
            .then_with(|| a.name.cmp(&b.name))
```
- **Evidência:** a mutação que favorece o shell sobrevive.
- **Proposta:** `main` do carapace contra `main` do PowerShell, ambos com prioridade 70: espera 1 item, com `description` "carapace" e `uses_shell_range == false`.

### M9. O dedupe de nome entre aspas (`'.\src\'`), declarado no doc, não tem teste
- **Local:** `src/engine/aggregate.rs:99`
- **Lente:** teste:engine
```rust
    let unquoted = name.trim_matches(['\'', '"']);
```
- **Evidência:** a mutação que remove o apóstrofo sobrevive.
- **Proposta:** `'.\my dir\'` (shell) contra `my dir/` (externo) deve resultar num só item.
- **Relacionado:** I3.

### M10. A regra "sem espaço depois de diretório entre aspas" só é testada com apóstrofo, não com aspas duplas
- **Local:** `src/engine/replacement.rs:26`
- **Lente:** teste:engine
```rust
    let unquoted = suggestion.trim_end_matches(['\'', '"']);
```
- **Evidência:** a mutação que aceita só o apóstrofo sobrevive.
- **Proposta:** `calculate_replacement("\"My", "\".\My Documents\\\"")` deve resultar em `insert_text` sem espaço final.

### M11. O kill e o timeout dos auxiliares (carapace e zoxide) não têm teste
- **Local:** `src/engine/providers/carapace.rs:113`
- **Lente:** teste:engine
```rust
        cmd.args(&args).kill_on_drop(true);
```
- **Evidência:** `kill_on_drop(false)` e 200 → 2000 ms sobrevivem. O commit e379bdd declara "kill timed-out helpers".
- **Proposta:** teste async com `with_binary` apontando para um auxiliar lento: deve voltar vazio em menos de ~500 ms e sem processo filho vivo.
- **Atenção à triagem:** M17 propõe apagar `with_binary`, e este teste depende dele. As propostas conflitam.

### M12. A fronteira "abre abaixo quando as linhas cabem" (`wanted == below`) não tem teste
- **Local:** `src/ui/renderer.rs:38`
- **Lente:** teste:io-ui-vt
```rust
        let (place_below, page_rows) = if wanted <= below {
```
- **Evidência:** `<` sobrevive. O rascunho com `cursor_y=18`, 24 linhas e 5 itens mata a mutação.
- **Proposta:** esse caso em renderer_test, esperando `start_row` 19 e `row_count` 5.

### M13. Ctrl+Space (NUL, o MenuComplete do PSReadLine) não tem teste de codificação
- **Local:** `src/io/key_event.rs:73`
- **Lente:** teste:io-ui-vt
```rust
        '@' | ' ' => Some(0),
```
- **Evidência:** a remoção de `' '` sobrevive.
- **Proposta:** `encode_key_event(Char(' '), CONTROL) == [0x00]`, mais Ctrl+[ e Ctrl+].

### M14. `Theme::default()` já diverge do tema padrão documentado, e os testes exercitam um tema que a produção nunca constrói
- **Local:** `src/ui/theme.rs:29`
- **Lente:** manutencao
```rust
            selected_start: "\x1b[7m".to_string(),
```
- **Evidência:** `Theme::default().selected_start="\u{1b}[7m"` contra `from_config(default)="\u{1b}[46m\u{1b}[30m"` (`equal=false`). README.md:91 documenta cyan/black. `SELECTED_PREFIX` está duplicado em `config.rs:17`.
- **Proposta:** `Theme::default() = from_config(&Config::default())` e derivar os prefixos de `config.rs`.

### M15. Helpers de formatação de Theme sem consumidor em produção (dois sem consumidor algum)
- **Local:** `src/ui/theme.rs:110`
- **Lente:** manutencao
```rust
    pub fn format_selected_line(&self, text: &str) -> String {
```
- **Evidência:** `find_consumers.ps1`: `format_selected_line` e `format_description_text` só aparecem em theme.rs (exit=1); `format_selected` e `format_description` só em tests/ (exit=2).
- **Proposta:** apagar os quatro métodos e os asserts que os usam.

### M16. O comando do README para criar o config corrompe os ícones emoji no PowerShell 5.1 e no pwsh com code page OEM
- **Local:** `README.md:84`
- **Lente:** docs
```
shell-panel --print-default-config | Set-Content "$HOME\.config\shell-panel.toml"
```
- **Evidência:** no 5.1, `directory = "?? "`. No pwsh com CP850, mojibake (`C2-AD-C6-92...`). Com `>`, os bytes ficam corretos (`F0-9F-93-81`). A wiki (Configuration.md:57-65) já usa `>` e avisa sobre o 5.1.
- **Proposta:** alinhar o README com a wiki.

### M17. `with_binary` de CarapaceProvider e de ZoxideProvider não tem consumidor
- **Local:** `src/engine/providers/carapace.rs:52`
- **Lente:** manutencao
```rust
    pub fn with_binary(binary_path: impl Into<String>) -> Self {
```
- **Evidência:** o único "consumidor" é a definição gêmea em `zoxide.rs:28`.
- **Proposta:** apagar os dois. **Conflita com M11**, que usaria esse construtor.

### M18. `CommandToken.width` é calculado para todo token e lido só por testes
- **Local:** `src/engine/lexer.rs:9`
- **Lente:** manutencao
```rust
    pub width: usize,
```
- **Evidência:** o único leitor é `tests/lexer_test.rs:177`.
- **Proposta:** remover o campo.

### M19. vt_test só confere os campos que a própria linha de cima atribui; o resize do parser VT não é testado (CONFIRMADO, rebaixado de Importante para Menor)
- **Local:** `src/vt/emulator.rs:42`
- **Lente:** teste:io-ui-vt
```rust
        self.parser.set_size(rows, cols);
```
- **Cenário:** com `set_size(cols, rows)`, depois de um `Event::Resize` (`app.rs:263`) o espelho fica 120x40 invertido.
- **Evidência:** `[!] SOBREVIVEU` nos 14 alvos não-e2e (a primeira rodada deu MORTA por um binário intermitente). O rascunho `screen().size()==(40,120)` mata a mutação. Foi rebaixado porque o resize não é documentado fora do doc comment.
- **Proposta:** duas asserções em `test_vt_headless_basic_processing_and_cursor`.

---

## Não verificados

Nenhum. Todos os bloqueantes passaram pela refutação.

## Derrubados na refutação

Nenhum.

---

## Lacunas de cobertura

### Infraestrutura de teste e confiabilidade das provas
- **Os e2e do binário oscilam com a máquina carregada.**
  - `test_completion_uses_the_real_line_and_the_real_session` falhou na linha 95 em 2 de 3 execuções isoladas na base. A causa não foi encontrada; uma cópia com dump de tela passou 6 de 6.
  - Rodando em paralelo, todos os e2e caíram por timeout de START.
  - Consequência: todo MORTA vindo de e2e nesta máquina pode ser ruído, e só SOBREVIVEU valeu como evidência. As mutações de `renderer.rs:38` e `theme.rs:254` contra o e2e ficaram inconclusivas.
- **Nenhuma unidade rodou o gate `verify.ps1`.**
- **Efeito colateral não limpo:** e2e e rascunhos gravaram comandos no histórico PSReadLine real do usuário (ver M2).
- **Mutações do engine:** e2e_binary_test, e2e_pty_test e shell_report_test não entraram no conjunto. A cobertura deles foi conferida só por grep.

### Ciclo de vida, saída e terminal
- A restauração de raw mode e cursor em panic, erro e shell morto não foi verificada no binário. O `disable_raw_mode` do Drop não é observável no harness.
- O panic hook de `main.rs` desliga o raw mode em QUALQUER panic, inclusive panic de task tokio que o runtime captura e após o qual o app segue rodando. Ctrl+C passaria a matar o shell-panel. Não houve panic alcançável demonstrado.
- `Some(Err(_)) => continue` no EventStream pode virar laço ocupado se o erro se repetir. Não construído.
- Resize com tamanho 0 (crossterm e `HeadlessTerminal`, onde o vt100 faz `rows - 1`): não testado.
- Saída com o dropdown aberto e shell morto com Tab pendente não foram dirigidos no binário.
- Core sem mutação: checagem de geração obsoleta (`app.rs:227`), caminho de zero matches (`:234`), fechamento do dropdown no Resize (`:260-264`), dreno de 1 s após a saída (`:184-194`), `--print-default-config`, eprintln de avisos de spec (`:127`).

### Teclado e interação
- Shift+Enter e Ctrl+Enter com o dropdown aberto: só leitura de `classify_key`.
- Ctrl+C e teclas Alt com o dropdown aberto.
- Redimensionar a janela com o dropdown aberto.
- Saída de programa em background fechando o dropdown.
- Completer com mais de 3 s seguido de novo Tab, isolado de C1.
- Ctrl+Tab vira `\t` e Alt+Esc vira `\x1b` (encode medido, sem comparação com o pwsh nativo).
- Alt+numpad: o Release com `u_char` é descartado pelo filtro `kind != Release` (`app.rs:265`). Não verificado.
- Modo de busca do PSReadLine (Ctrl+R) e nested prompts.
- Tab no meio de uma linha com emoji antes do cursor: o cálculo UTF-16 de `plan_replacement` não foi exercitado no binário, porque o emoji nem chega (C4).
- C4: a ordem real dos `KeyEventRecord` não foi confirmada; a causa no crossterm segue como hipótese.
- Uma rodada em que o dropdown não apareceu 500 ms depois de Shift+Tab não se repetiu.
- **A lente interativo só rodou pwsh; não houve sessão interativa com powershell.exe 5.1.**

### Codificação e Unicode
- OEM 437 (en-US): não testado. O e2e atual com `ação` provavelmente falha lá.
- Opção beta "UTF-8 mundial" do Windows: não testada; ela esconderia C2.
- Efeito de `replace_range` quando já há um caractere fora da code page antes do cursor (o relatório e a linha real divergem): não medido.
- cwd do RS com caminho CJK repassado aos providers: não medido.
- Overflow de 1 coluna (M1) num WT ou conhost real: não confirmado, porque o vt100 do harness conta por char.

### Engine e providers
- Carapace nunca foi observado end-to-end no binário: sempre passou do timeout de 200 ms (1,6 a 10,7 s medidos). Não foi avaliado se 200 ms torna o provider inútil em máquinas típicas.
- O timeout de 150 ms do zoxide está perto do tempo medido (154 a 540 ms com o overhead do pwsh). Não foi medido dentro do binário.
- `quote_for_powershell` com `[ ]` (curinga do Set-Location) e aspas tipográficas: não testado.
- `json_spec` ignora o `args` das opções: uma opção que recebe valor ganha sugestão de subcomandos. Não construído, porque os specs embutidos não têm esse caso.
- `nospace` e `noprefix` do carapace são ignorados.
- Semântica de `assets/specs/git.json` e `docker.json`: só foi conferido que parseiam.
- Engine sem mutação: guarda `uses_shell_range` em `shell_result_type`, filtro de description igual a list_item, `BUILTIN_ALIASES`, prioridades por ResultType, raiz case-insensitive, ramo `tokens.len()==1`, delimitadores de `active_token_raw`, guardas `closed_quote` e `len>1` do `&`/`.`, fallback de display vazio do carapace, kill e timeout do zoxide.

### Shell e PTY
- Sem cobertura de teste para a preservação de `$?` e `LASTEXITCODE` pelo wrapper. O comportamento atual foi verificado como correto.
- Teto de 100 matches contra `MAX_MESSAGE_BYTES` (1 MiB): nenhum relatório maior foi construído.
- Guarda contra recarga de `__SP_OriginalReadLine`: nenhum caminho real de segunda carga foi achado.
- Sequências que o filter deveria remover (`\x1b[?9001h`, kitty) podem chegar partidas entre chunks e passar. O residual só guarda prefixo de OSC. Não construído.
- O `HELLO_SHELL_PANEL` de `test_e2e_pty_powershell_session` casa com o eco do texto digitado. Não há evidência de mutação sobrevivente.

### Segurança
- C1 (U+0080 a U+009F) no texto inserido: o `is_insertable` não filtra. O U+009B virou `?` sem que se saiba em que ponto, e não houve comparação com o Tab nativo.
- Se o Windows Terminal interpreta C1 de 8 bits vindo do dropdown: não medido.
- O cwd de um RS forjado vira o `current_dir` do carapace. A cadeia "RS forjado → carapace num repositório malicioso" não foi construída, e a busca do executável no diretório atual pelo std não foi executada.
- Variante de C3 com processo em background apagando o texto digitado: reproduzida pela lente, não pelo refutador.
- `key_event.rs`, `lexer.rs`, `config.rs` e os specs JSON do usuário não foram lidos por inteiro sob a ótica de segurança.
- `--verbose` não foi executado; só houve grep confirmando que contagens e mensagens fixas vão para o log.

### UI e testes de io-ui-vt sem mutação
- `xterm_modifier_code` com ALT, o resto de `control_byte` (`[ \ ] ^ _ ?`), `Theme::from_config` com `selected_fg=invert`, `clear_dropdown` com `start_row+row_count > rows` e `min_width` 30.

### Documentação
- As afirmações dinâmicas da tabela Keys e do README (match único insere, Enter com dropdown, fallback de 3 s, linha de continuação, posição do dropdown) foram conferidas pela lente docs só lendo o código. A lente interativo cobriu parte delas.
- As mensagens reais `stream did not contain valid UTF-8` (Troubleshooting:38) e o texto "line and column" do erro TOML não foram executados.
- Os comentários de módulo e de função de `lexer.rs`, `replacement.rs`, `report.rs`, `command_state.rs`, `filter.rs`, `raw_mode.rs`, `vt/emulator.rs`, `patch.rs`, `carapace.rs` e `zoxide.rs` não foram conferidos um a um.
- A docs/wiki não foi conferida linha a linha pela lente manutencao.
- A afirmação sobre consoles antigos do Windows 10 que truncam OSC longos não foi testada.
- A frase do README "files are only shown if..." foi achada ambígua e não registrada; ela só considera sugestões externas (`aggregate.rs:114`).
- Restos marginais vistos e não registrados: `CarapaceItem.style` com `#[allow(dead_code)]` e um `contains_key` redundante em `JsonSpecProvider::can_handle`.

### Relações para a triagem (sem decisão)
- C1, C3 e I8 tocam a mesma guarda em `app.rs:211-213`. As correções propostas (contador de relatórios pendentes, token de sessão, teste da guarda) se sobrepõem.
- M11 e M17 conflitam: o teste de kill e timeout usaria `with_binary`, que M17 propõe apagar.
- I19 e M1 estão na mesma função (`truncate_to_width`) com defeitos diferentes.
- I3 e M9 envolvem o dedupe de nomes entre aspas entre carapace e PowerShell.
- M3 e M4 compartilham a mesma correção proposta.