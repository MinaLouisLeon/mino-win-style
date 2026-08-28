# mino-win-style

Change how Windows 11 looks — accent colour, dark mode, taskbar, Start, File
Explorer — and put every change back exactly as it was.

A Rust core (`mino-core`) drives the registry and the Win32 API directly. The
interface is HTML and TypeScript running in WebView2, hosted by Tauri 2.

> **Status: scaffold (M0 + most of M1). Never compiled.**
> Rust and the MSVC toolchain are not installed on the machine this was written
> on, by request. Everything below is written to compile, but nothing has been
> run. See [Before the first build](#before-the-first-build).

## Why it is built this way

- **Reversible by construction.** No change is applied before its previous value
  is written to a journal on disk, together with a `.reg` file you could import
  by hand. `plan()` is pure and never writes; `Engine::apply` is the only path
  to a write in the whole program.
- **Supported mechanisms only.** Documented APIs and per-user registry values
  (Tier A), a small number of undocumented-but-stable keys behind build gating
  and a warning (Tier B). No `uxtheme` patching, no DLL injection into
  `explorer.exe` (Tier C) — a test in `tweaks/mod.rs` fails the build if a Tier C
  setting ever appears in this binary.
- **The UI cannot name a registry key.** There is no `write_value` command. The
  front end can only ask for settings the engine already implements.

## Layout

```
crates/
  mino-core/   engine, tweaks, journal, packs, compatibility gating — no Windows deps
  mino-win/    the only code that calls Win32: registry, shell refresh, OS detection
  mino-cli/    `mino` — headless apply/revert, and the safe-restore path
src-tauri/     Tauri 2 shell: commands, window, bundler config
ui/            React 19 + TypeScript + Vite, English and Arabic with real RTL
packs/         style packs (`manifest.json` + assets)
```

`mino-core` has no dependency on `windows`, so the entire planner, journal and
pack parser are unit-testable on any machine in milliseconds. Everything that
touches the OS sits behind two traits, faked by `MemoryRegistry` in tests.

## Before the first build

Install, in this order:

1. **Visual Studio 2022 Build Tools** with the *Desktop development with C++*
   workload — this provides the MSVC linker Rust needs on Windows.
   `winget install --id Microsoft.VisualStudio.2022.BuildTools -e`
2. **Rust**: `winget install --id Rustlang.Rustup -e`, then
   `rustup default stable-x86_64-pc-windows-msvc`
3. **Tauri CLI**: `cargo install tauri-cli --version "^2"`
4. **UI dependencies**: `pnpm --dir ui install`

Then:

```
cargo test -p mino-core     # the part that matters; no Windows APIs involved
cargo clippy --workspace
cargo tauri dev             # from src-tauri, or `cargo tauri dev` at the root
```

`pnpm --dir ui dev` also runs the interface on its own in a browser at
<http://localhost:1420>, backed by the mock in `ui/src/lib/mock.ts`. Useful for
layout work; it never touches the registry.

### Expect to fix these first

Written without a compiler to hand, so these are the places to look when the
first `cargo build` complains:

| Where | What to check |
| --- | --- |
| `crates/mino-win/src/reg.rs` | Every `unsafe` call is written against `windows = "0.58"` exactly. Argument shapes (`uloptions: u32` vs `Option<u32>`) move between versions of that crate. |
| `crates/mino-win/src/shell.rs` | Same, for `SendMessageTimeoutW`, `SHChangeNotify` and `SystemParametersInfoW`. |
| `src-tauri/tauri.conf.json` | Validate against the v2 schema your installed Tauri CLI ships. |
| `AccentColorTweak::palette` | The `AccentPalette` byte layout is an assumption, not a documented format. Set an accent through Settings, dump the value, diff it against the function. |

## Testing

`crates/mino-core/tests/revert_is_exact.rs` is the test that decides whether a
tweak may ship: apply everything, revert everything, and require the registry to
be byte-identical to where it started — including values that were absent before
and must be absent again.

The VM matrix (22H2 / 23H2 / 24H2 / 25H2) described in the project plan is M3
work and is not in the repository yet.

## Recovery

Every applied batch writes to
`%LOCALAPPDATA%\mino-win-style\journal\<id>.json`, plus `<id>.reg` holding the
previous values.

```
mino history                 # what has been changed
mino revert <id>             # undo one batch
mino safe-restore            # undo the most recent batch
```

`mino safe-restore` deliberately does not depend on the app starting.

## Licence

MIT — see [LICENSE](LICENSE).
