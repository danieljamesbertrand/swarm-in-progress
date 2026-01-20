## Durable engineering “memory” for this repo

These rules exist because agent-local memory does not persist. The repo must enforce correctness.

### Toolchain (Rust)
- **Toolchain is pinned** via `rust-toolchain.toml`. Do not “upgrade rust” ad-hoc.
- **Dependencies must remain reproducible**: `Cargo.lock` is tracked; CI runs `cargo build --locked` and `cargo test --locked`.
- If you *must* update dependencies:
  - Prefer targeted updates: `cargo update -p <crate> --precise <version>`
  - Re-run `cargo build --locked` and `cargo test --locked`
  - Document the reason in the commit message / PR description.

### PowerShell scripts (quoting + safety)
- **Never construct command lines as strings** (fragile quoting). Prefer:
  - **Argument arrays**: `& $exe @args`
  - **Splatting** for parameters: `& $exe @paramSplat`
- **Default quoting rule**:
  - Use **single quotes** for literal strings.
  - Use **double quotes** only when interpolation is required.
- **Paths**: build them with `Join-Path` rather than manual quoting/escaping.
- **Environment variables (PowerShell vs bash)**:
  - bash style `VAR=1 command ...` **does not work** in PowerShell.
  - PowerShell: `$env:VAR="1"; command ...`
  - Cmd.exe: `set VAR=1` then run the command.
- **Guardrail**: CI runs a **parse check** over all `*.ps1` (`scripts/ci/pwsh_parse_all.ps1`) to catch broken quoting/syntax before it lands.

### Adjacent/backup folders
- Backup folders are archival. CI enforces correctness for the primary crate at repo root.
