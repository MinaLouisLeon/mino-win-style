# Phase 2 — The top bar

**Delivers:** a third `mino-shell` surface — a strip across the top of the
primary monitor that Cupertino and Yaru both wear. No Look ships in this phase;
it is the infrastructure two of them need.

**New Win32:** `SHAppBarMessage`, `GetForegroundWindow`, and
`SystemParametersInfo(SPI_SETWORKAREA)` for the recovery path.

**Gate:** do not start this phase until the HUD has been seen running on a real
machine. Click-through, always-on-top and full-screen placement over the taskbar
are still untested outside the browser and the fakes. This phase adds a window
that is always on top, *takes clicks*, and changes the desktop work area — if it
misbehaves on top of three unverified assumptions there is no way to tell which
layer did it.

## How it differs from the dock and the HUD

| | Dock | HUD | Top bar |
| --- | --- | --- | --- |
| Takes clicks | Yes | **No** (`WS_EX_TRANSPARENT`) | Yes |
| Always on top | Yes | Yes | Yes |
| In the work area | Sits over it | Covers the screen | **Reserves a slice of it** |
| Steals focus | Never (`focused(false)`) | Never | On click, unavoidably |

The third row is the whole difficulty. The dock floats over the bottom of the
work area and a maximized window simply goes under it, which is survivable
because the taskbar is auto-hidden there anyway. A bar across the top cannot do
that: a maximized window would put its own title bar and close button
underneath ours, and the user could not reach them. The strip has to be
*reserved*, so Windows maximizes everything below it.

## Reserving the strip

`SHAppBarMessage` is the documented way, and it is what the taskbar itself uses:

1. `ABM_NEW` with the bar's `HWND` and a private callback message.
2. `ABM_QUERYPOS` then `ABM_SETPOS` with `ABE_TOP` and the rectangle wanted;
   Windows may adjust it, and the adjusted rectangle is what we place to.
3. Move the window to whatever came back from `ABM_SETPOS`, then
   `ABM_WINDOWPOSCHANGED`.
4. `ABM_REMOVE` when the bar is switched off, **and** on process exit.

The alternative — calling `SystemParametersInfo(SPI_SETWORKAREA)` ourselves —
is fewer moving parts and is rejected: it does not cooperate with the taskbar,
so a taskbar that stops auto-hiding, or a resolution change, leaves two parties
disagreeing about the same rectangle.

### The hazard, and the way out

**If the process dies without `ABM_REMOVE`, the desktop work area can stay
shrunk** — a strip of dead space at the top of every screen, surviving reboots,
with nothing on screen to explain it. This is the most user-hostile failure
anything in this repo can produce, and it must be handled the way the registry
writes are: with a recovery that does not depend on the app starting.

- `ABM_REMOVE` on `RunEvent::Exit`, on window destroy, and on the look being
  switched off.
- Register the bar with `ABM_NEW` only while it is actually shown.
- **`mino shell-reset`** — a new `mino-cli` subcommand alongside `safe-restore`:
  removes any appbar this program registered and restores the work area to the
  monitor rectangle via `SPI_SETWORKAREA` with `SPIF_SENDCHANGE`. It is a
  one-line fix the user can run from a command prompt, and it belongs in the
  README's Recovery section next to `mino safe-restore`.
- A test cannot cover this. Phase 6 covers it by hand: kill the process from
  Task Manager with the bar up, and check the work area comes back.

### Explorer restarting takes the reservation with it

When `explorer.exe` restarts, every registered appbar is destroyed and Windows
broadcasts `TaskbarCreated` to tell surviving shell extensions to register
again. This is not an edge case here — **applying a Look restarts Explorer**, so
a Look that reserves a strip and then applies its pack would lose the
reservation as part of being switched on, and the symptom is a bar that looks
right until the moment the desktop repaints.

So the bar registers a window message for `TaskbarCreated`
(`RegisterWindowMessageW`) and re-runs `ABM_NEW` + `ABM_SETPOS` when it arrives.
The same handler covers a manual Explorer restart and the app's own
`shell refresh`.

## Focus, and the title that must not flip

Clicking our bar activates it, which deactivates whatever the user was in. If
the bar shows "the focused application's name" naively, that name becomes
*Mino* the moment they click it — the one moment they are looking at it.

So the bar tracks **the last foreground window that was not one of ours**. Add
to `mino-shell`:

```rust
/// The foreground window, or `None` when it belongs to this process.
pub fn foreground() -> Option<AppWindow>;
```

and let the page keep the last `Some`. Filtering by process id rather than by
window label is what makes it hold for the dock and the HUD too.

Polling at ~250 ms, like the dock polls its list. `SetWinEventHook` with
`EVENT_SYSTEM_FOREGROUND` is the right long-term answer for both surfaces and is
already on the README's list; it is deliberately *not* in this phase, because a
hook means a message loop on a thread of our own and this phase has enough new
Win32 in it.

## What a menu bar can and cannot show

**We cannot read another application's menus.** There is no supported way to
enumerate the File/Edit/View of an arbitrary window from outside its process,
and this project does not go inside one. So the bar shows what it can actually
act on, and shows nothing it cannot:

- **The focused app's name**, from `AppWindow::exe` via `display_name`.
- **Window commands we already implement** — Minimise, Maximise/Restore, Close
  (`mino_shell::{minimize, toggle_maximize, close}`).
- **Our own menu** — the Looks picker, dock and bar switches, Settings, Quit.
- **A status cluster** — clock and date from the page's locale, battery and
  network from the same `Sampler` the HUD uses, and a click-through to Windows'
  own panels where one exists.

Greyed-out File/Edit/View that do nothing would be the one dishonest thing in
this app. If a Look wants them, the answer is no.

## Work

1. **`crates/mino-shell`** — `foreground()`, and an `appbar` module:
   `register(hwnd, edge, thickness) -> WorkArea`, `unregister()`,
   `reset_work_area()`. Behind `#[cfg(windows)]` like the rest of
   `windows_impl.rs`; `windows-sys` gains `Win32_UI_Shell`.
2. **`src-tauri/src/top_bar.rs`** — same shape as `dock.rs`: config in
   `topbar.json` (`enabled`, `height`, and later `style`), window created hidden
   at startup **on the main thread** (the `dock::create` comment explains why in
   full), `show`/`hide`, `ABM_*` calls paired with them, and the
   `TaskbarCreated` re-registration handler.
3. **`ui/topbar.html` + `ui/src/topbar/`** — `TopBar.tsx`, `topbar.css`, its own
   `api.ts` bridge and `main.tsx`, following the dock's layout. It subscribes to
   `shell-look` for its skin like every other page.
4. **`mino-cli`** — the `shell-reset` subcommand.
5. **Vite** — a fourth entry point in `ui/vite.config.ts`; `tauri.conf.json` needs
   nothing, since windows are built in code.
6. **Docs** — the top bar and `mino shell-reset` in the root README, under the
   dock and in Recovery.

## Non-goals

- **Multi-monitor.** Primary only, exactly like the dock. One monitor per
  surface is a whole feature and it is not this one.
- **Menus of other applications.** Stated above; not a scope question.
- **Auto-hide.** The bar is reserved space or it is nothing.

## Tests

- `WorkArea` arithmetic — placement and the DPI division — is pure and testable
  without Windows; put it in `mino-shell` next to the existing unit tests.
- `foreground()` filtering out our own windows.
- Config round-trips, and an older/malformed `topbar.json` falls back.
- The appbar calls themselves cannot be unit-tested. They are Phase 6 by hand.

## Done when

The bar sits across the top of the primary monitor, a maximized window stops
underneath it rather than behind it, switching it off gives the space back
immediately, killing the process and running `mino shell-reset` gives the space
back too, restarting Explorer leaves the strip still reserved, and the app name
in the bar does not change to *Mino* when clicked.

## What shipped, where it differs from the above

**The gate was not met.** The HUD has still never run on a real machine, and
this phase was built anyway at the user's direction. Nothing here has been seen
working: `mino-shell` compiles and its arithmetic is unit-tested, but no
`SHAppBarMessage` call in this repository has ever been made. Phase 6 is now
carrying the HUD's smoke test *and* every appbar question at once, which is
exactly the situation the gate existed to avoid — when the bar misbehaves, the
overlay's three unverified assumptions are still in the frame.

- **`SetWindowSubclass`, which the plan did not name.** The plan said to
  register for `TaskbarCreated` without saying how a Tauri window would ever see
  it. It cannot: Tauri exposes no window procedure. The bar's own window is
  subclassed instead, forwarding everything to `DefSubclassProc` — documented,
  on a window we own, and the same hook picks up `ABN_POSCHANGED`. A hidden
  proxy window was the alternative and was rejected: an appbar whose window is
  never visible is not a shape Windows documents.
- **The HWND crosses into `mino-shell` as an `isize`.** Tauri builds against
  `windows` 0.61 and this workspace pins 0.58, so the two `HWND` types are not
  the same type. `window.hwnd()?.0 as isize` is the line most likely to need a
  nudge on a machine that can actually compile `src-tauri`.
- **The bar's menu is Settings and Quit, not a second copy of the app.** The
  plan listed the Looks picker and the dock and bar switches as menu items.
  Every one of those already exists in the settings window, and a second copy of
  a control is a second thing to keep true — so the menu opens the window that
  has them.
- **The window commands are the dock's.** `dock_minimize`,
  `dock_toggle_maximize` and `dock_close` are one line each into `mino-shell`
  and have nothing to do with docks; the bar calls them rather than adding three
  identical commands under a different prefix.
- **`describe()` now filters our own windows by process id**, not by matching
  the executable name — which is what makes `foreground()` able to tell "the
  user clicked the bar" from "the user switched application". The dock inherits
  the better filter.
- **The bar has its own switch on Home**, like the dock's. It had to: no Look
  wears the bar until Phase 3, so without a switch there would be no way to see
  it at all.
