# Raw input passthrough while a program runs — design

Date: 2026-10-02. Status: approach chosen by the user (option 2: two input modes), design for review.

## Problem

Two symptoms the user reported, with one cause.

1. **Slow start, and `[?65;4;6;18;22;52c` typed at the first prompt.** ConPTY sends `ESC[c` (Primary Device Attributes, DA1) when it starts, and waits for the answer. shell-panel passes the query on to the host terminal, and the host answers. But shell-panel reads its console as key events (crossterm), so the answer reaches it as the keys `[ ? 6 5 ; 4 ; 6 c`, with the ESC dropped. It re-encodes them one by one, and PowerShell receives them as typed text. ConPTY never gets an answer and only continues after its timeout.
   - Measured in a test ConPTY: the prompt appears after about 2.1 s when the query is answered, and about 5.9 s when it is not.
2. **Programs render or behave wrongly.** While a program runs, everything the host sends as an input sequence is dropped or turned into text: mouse reports, focus reports (`ESC[I`/`ESC[O`; ConPTY turns them on with `?1004h`), bracketed paste, answers to queries, and keys crossterm decodes but shell-panel cannot encode (F13–F24 are dropped).
   - The reactor only handles key and resize events.
   - The exact rendering defect the user saw is not reproduced yet. This design removes the input-side cause. Verifying against the user's programs is a step in the plan.

## Measured facts the design rests on

From a throwaway probe: a crossterm reader inside a test ConPTY, with input written by the test.

| Input written by the host | Classic console mode (today) | `ENABLE_VIRTUAL_TERMINAL_INPUT` set |
|---|---|---|
| `ab` | `a`,`b` press **and release** events | `a`,`b` press events only |
| `ESC[A` | `Up` | `[`, `A` — the ESC is missing |
| `ESC[25~` (F13) | nothing | `[`,`2`,`5`,`~` — the ESC is missing |
| `ESC[?65;4;6c` (DA1 answer) | `[`,`?`,`6`,`5`,`;`,`4`,`;`,`6`,`c` — the bug | `[`,`?`,… — the ESC is missing |
| `ESC[I` (focus) | `FocusGained` | dropped |

- With VT input on, the console delivers the host's bytes in order, as key records with virtual-key code 0 and the character in `u_char`.
- crossterm 0.28.1 drops a record whose `u_char` is a C0 control (`ESC`, also `^A`…) when its virtual-key code is 0, because `get_char_for_key` finds no key for it. That is the only loss.
- Answering DA1 with a minimal or a rich reply makes no difference to colour output (SGR and truecolour pass through either way). The content of the answer does not matter to ConPTY.

## Design

### Two input modes

- **Prompt mode**, while PSReadLine is reading a line (`CommandState::reading_line`). This is today's behaviour, unchanged: classic console input, key events, `classify_key`/`encode_key_event`, Tab requests a report, and the dropdown keys.
- **Program mode**, at all other times, including start-up before the first prompt. The console input mode has `ENABLE_VIRTUAL_TERMINAL_INPUT` set. Every key event is the host's raw text: shell-panel writes its UTF-8 bytes to the PTY unchanged, without classification, re-encoding, report requests or dropdown handling. Resize events are handled as today.

The mode follows `reading_line`. After each PTY chunk is ingested, if `reading_line` changed, shell-panel switches the console mode. It starts in program mode, so the DA1 answer to ConPTY's start-up query reaches ConPTY raw: no delay, and nothing typed.

### Components

1. **`vendor/crossterm` patch #2:** in `parse_key_event_record`, a key-down record with virtual-key code 0 and a `u_char` in `0x00..=0x1f` yields `KeyCode::Char(<that control char>)` instead of being dropped. Classic mode never produces such a record for a real key, so prompt mode is unaffected. `SHELL-PANEL-PATCH.md` documents it next to patch #1.
2. **`src/io/console_mode.rs` (new):** `pub fn set_vt_input(enabled: bool) -> std::io::Result<()>`, which reads the stdin console mode and sets or clears `ENABLE_VIRTUAL_TERMINAL_INPUT` (0x0200) through `crossterm_winapi::ConsoleMode` (it is already in `Cargo.lock` as a crossterm dependency; it becomes a direct dependency, with no new crate in the lock file). Outside a console, for example with stdin redirected, it returns the error and the caller logs it. shell-panel then keeps working in prompt-mode style.
3. **Reactor (`src/core/app.rs`):** a `program_mode: bool`, recomputed from `command_state.reading_line` after each ingest. The console mode switches on change, and also once before the loop starts. In program mode, a `Key` event writes the raw bytes of its char and skips everything else in `handle_key`. A key event other than a plain `Char` in program mode should not happen; if it does, it goes through `encode_key_event` as today.
4. **Restoring the console:** `RawModeGuard::drop` and the panic hook clear `ENABLE_VIRTUAL_TERMINAL_INPUT` before disabling raw mode, so the user's console is left as it was found.

Output is unchanged. The existing stripping of `?9001h` and kitty negotiation stays, and the VT mirror and dropdown are untouched.

### What this fixes and what stays

- The DA1 answer at start-up reaches ConPTY. The start-up wait and the typed `[?65;…c` go away.
- While a program runs, mouse, focus, bracketed paste, query answers, F13–F24 and every other host sequence reach the program unchanged.
- In prompt mode, nothing changes. A Tab inside pasted text still triggers completion, which is already documented in Known limitations.
- If a host sequence is split across the switch between modes (typed in the instant a program starts or ends), it may be read in the wrong mode. This is inherent to switching. The window is the time between RE/RS and the mode switch.

## Testing

End-to-end, through the real binary in the test ConPTY (`tests/e2e_binary_test.rs`):

1. **DA1 at start-up:** the harness answers every `ESC[c` it sees, like a real terminal. That includes the query shell-panel forwards from its own ConPTY. After the first prompt the screen must not contain `?6` / `;4;6c`. RED today: the forwarded answer is typed. Also measure and report time-to-prompt, without asserting it.
2. **F13 reaches a running program:** `[Console]::ReadKey($true)`, as in the I11 test. The harness writes `ESC[25~` and expects `GOT-F13-…`. RED today: dropped.
3. **ESC sequences intact in program mode:** `ESC[A` written during `ReadKey` gives `GOT-UpArrow-…`. This guards patch #2: without it, the ESC is lost and `[`, `A` arrive.
4. **Prompt mode unchanged:** the existing e2e suite (Tab, dropdown, Enter, withheld Tab, fallback, chord never reaches a program) stays green.
5. **Console restored:** after exit, the console input mode no longer has the VT bit. Unit test of the mode helper if a console is available; otherwise covered by a manual check written into the plan.

Mutation proofs: remove patch #2 (test 3 fails); force prompt mode always (tests 1 and 2 fail).

## Out of scope

- Answering DA1 from shell-panel itself (option 1). It is unnecessary once the host's answer passes through.
- Raw input in prompt mode (option 3).
- Output-side changes for specific programs, until the user's failing programs are known.
