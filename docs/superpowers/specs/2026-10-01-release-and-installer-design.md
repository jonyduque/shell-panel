# Release workflows and PowerShell installer — design

Date: 2026-10-01. Status: approved in conversation (approach A, sections 1 and 2).

## Goal

Anyone on Windows installs shell-panel with one line,

```powershell
irm https://github.com/jonyduque/shell-panel/releases/latest/download/install.ps1 | iex
```

and gets the binary on `PATH` plus a "PowerShell (shell-panel)" profile in Windows Terminal. A
maintainer publishes a release by bumping `version` in `Cargo.toml`, tagging `v<version>` and
pushing the tag.

## Decisions taken with the user

| Question | Decision |
|----------|----------|
| Who installs | The public: the repository becomes public, releases are downloaded without authentication. Making it public is done only after a secret scan of the history and an explicit confirmation from the user. |
| Release trigger | Push of a tag `v*`. The tag must equal `v` + `version` of `Cargo.toml`. |
| Targets | `x86_64-pc-windows-msvc` and `aarch64-pc-windows-msvc`. ARM64 is cross-compiled on the x64 runner and is not executed in CI. |
| Windows Terminal | The installer always writes a Windows Terminal profile fragment. |
| Build approach | Own workflows using only `actions/checkout` and the preinstalled `gh` CLI (approach A). cargo-dist and third-party upload actions were rejected: the installer has to be ours anyway (Terminal profile, checksum, PATH). |

## Workflows

### `.github/workflows/ci.yml`

- Triggers: push to `main`, pull requests.
- One job on `windows-latest`: checkout, `pwsh -NoProfile -File .claude/skills/shell-panel-review/scripts/verify.ps1` (fmt, clippy `-D warnings`, full test suite; the end-to-end tests need pwsh and Windows PowerShell 5.1, both present on the runner).
- A second job `installer` (see Testing) runs after a release build of x64.
- `permissions: contents: read`.

### `.github/workflows/release.yml`

Trigger: `push: tags: ['v*']`. Jobs:

1. **verify** — fails unless `github.ref_name == "v" + version` read from `Cargo.toml`; then runs `verify.ps1`.
2. **build** — matrix over the two targets, `needs: verify`. `rustup target add <target>`, `cargo build --release --locked --target <target>`, then `shell-panel-<version>-<arch>.zip` (arch `x64` or `arm64`) containing `shell-panel.exe`, `LICENSE`, `README.md`. Uploaded as a workflow artifact.
3. **installer** — `needs: build`; the installer test (see Testing) against the x64 zip just built.
4. **publish** — `needs: [build, installer]`, `permissions: contents: write`. Downloads the artifacts, writes `SHA256SUMS.txt` (`<lowercase sha256>  <file name>` per zip, the `sha256sum` format), and runs `gh release create <tag> <zips> SHA256SUMS.txt install.ps1 uninstall.ps1 --title <tag> --generate-notes`, adding `--prerelease` when the tag contains `-`.

All other jobs have `permissions: contents: read`. Release-profile settings already in `Cargo.toml` (LTO, `panic = "abort"`, strip) apply.

## Installer — `install.ps1` (repository root)

Runs on Windows PowerShell 5.1 and PowerShell 7, both via `irm | iex` and as a file. `Set-StrictMode -Version Latest`, `$ErrorActionPreference = 'Stop'`. When piped through `iex` the script receives no arguments, so every parameter has a default; parameters are honoured when the file is run directly.

Parameters:
- `-Version <x.y.z>` — default: the latest release, from `https://api.github.com/repos/jonyduque/shell-panel/releases/latest` (`tag_name` without the `v`).
- `-InstallDir <path>` — default `%LOCALAPPDATA%\Programs\shell-panel`.
- `-ZipPath <path>`, `-ChecksumsPath <path>` — testing only: install from local files instead of downloading.

Steps:
1. **Architecture**: `$env:PROCESSOR_ARCHITEW6432` if set, else `$env:PROCESSOR_ARCHITECTURE`; `AMD64` → `x64`, `ARM64` → `arm64`, anything else aborts with a message naming the architecture.
2. **Download** (unless `-ZipPath`): force TLS 1.2 on 5.1; download `shell-panel-<version>-<arch>.zip` and `SHA256SUMS.txt` from `https://github.com/jonyduque/shell-panel/releases/download/v<version>/` into a fresh temp directory.
3. **Verify**: the zip's SHA-256 (`Get-FileHash`) must equal its line in `SHA256SUMS.txt` (case-insensitive). Missing line or mismatch aborts before anything installed is touched.
4. **Install**: extract into the temp directory; create `InstallDir`; copy `shell-panel.exe` over the old one. If the copy fails because the file is in use, abort with "close every shell-panel session and run the installer again". Also copy `LICENSE` and `README.md`.
5. **PATH**: read `HKCU:\Environment` `Path` raw (`GetValue` with `DoNotExpandEnvironmentNames`), append `InstallDir` if no entry equals it (case-insensitive, ignoring a trailing `\`), write it back preserving the value kind (`ExpandString` when it was one or is new). Broadcast `WM_SETTINGCHANGE` so new processes see it. Update `$env:Path` of the current session.
6. **Windows Terminal**: write `%LOCALAPPDATA%\Microsoft\Windows Terminal\Fragments\shell-panel\shell-panel.json`:
   ```json
   { "profiles": [ { "name": "PowerShell (shell-panel)", "commandline": "\"<InstallDir>\\shell-panel.exe\"", "icon": "ms-appx:///ProfileIcons/pwsh.png", "guid": "{<fixed GUID>}" } ] }
   ```
   with a fixed GUID constant in the script so reinstalling updates the same profile. Written as UTF-8 without BOM. Inert when Windows Terminal is absent.
7. **Report**: run `"<InstallDir>\shell-panel.exe" --version` and print it, the install path, and how to start (the Terminal profile, or `shell-panel` in a new window).

Output uses plain ASCII markers (`[*]`, `[OK]`, `[!]`) — a CP-850 console mangles other symbols.

## Uninstaller — `uninstall.ps1` (repository root)

Parameters: `-InstallDir` (same default), `-Purge`. Removes `InstallDir`, its `PATH` entry (same comparison as the installer, same value-kind preservation, broadcast), and the fragment directory. With `-Purge` also removes `%USERPROFILE%\.config\shell-panel.toml` and `%USERPROFILE%\.config\shell-panel\`. Without it, says those were kept. Aborts with the same "close every session" message when the binary is in use. Idempotent: running it twice succeeds.

## Testing

A PowerShell test script `scripts/test-installer.ps1`, run in CI by the `installer` job, first under `pwsh` and then under `powershell.exe`, with a temporary `InstallDir` and against the zip built in the same run:

1. Install with `-ZipPath`/`-ChecksumsPath` → exit 0; `shell-panel.exe --version` prints the Cargo version; `HKCU` `Path` contains `InstallDir` exactly once; the fragment exists, parses as JSON and its `commandline` points at `InstallDir`.
2. Install again → `Path` still contains `InstallDir` exactly once (idempotent).
3. Install with a checksums file whose hash is altered → non-zero exit / thrown error, and the binary from step 1 is unchanged (same hash).
4. Uninstall → `InstallDir`, the `Path` entry and the fragment are gone; uninstall again succeeds.

The test saves and restores the user's `HKCU` `Path` value around the run, so running it on a developer machine leaves `PATH` as it was. Locally the same script runs against `target\release` packed into a zip by the script itself when `-ZipPath` is not given.

`verify.ps1` stays the code gate; the workflows call it rather than duplicating its steps.

## Documentation

README: an "Install" section with the one-line command, the uninstall command, what gets installed where, and the release procedure for maintainers (bump version, `git tag vX.Y.Z`, `git push origin vX.Y.Z`).

## Going public (manual, gated)

Before the repository is made public: scan the whole history for secrets (`gitleaks`-style patterns via `git log -p` grep for keys/tokens/passwords, plus a review of every tracked file list), report findings to the user, and only on explicit confirmation run `gh repo edit jonyduque/shell-panel --visibility public --accept-visibility-change-consequences`. The installer and workflows can be merged while the repository is private (CI tests the installer from local files); the first release tag is pushed only after the repository is public, since the one-line install cannot work against a private repository.

## Out of scope

Code signing, winget/scoop manifests, auto-update, Linux/macOS builds, CHANGELOG files (release notes come from `--generate-notes`).
