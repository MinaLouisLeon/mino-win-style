//! Reserving a strip of the desktop, the way the taskbar does.
//!
//! The dock at the bottom of the screen floats: a maximized window goes under
//! it, which is survivable because the taskbar it sits on top of is auto-hidden
//! anyway. A bar across the *top* cannot do that — a maximized window would put
//! its own title bar and close button underneath ours, where nobody can reach
//! them. Neither can a dock down the *side*, which is what Yaru wants. So the
//! strip has to be reserved, and `SHAppBarMessage` is the documented way to ask.
//!
//! Doing it ourselves with `SystemParametersInfo(SPI_SETWORKAREA)` would be
//! fewer moving parts and is deliberately not what happens here: it does not
//! cooperate with the taskbar, so a taskbar that stops auto-hiding, or a
//! resolution change, leaves two parties disagreeing about the same rectangle.
//! That call appears once, in [`reset_work_area`], which is the recovery and
//! not the mechanism.
//!
//! # More than one
//!
//! Two of our surfaces can hold a strip at the same time — Yaru wears a bar
//! across the top *and* a dock down the left — so registrations are kept per
//! window rather than one to a process. Getting that wrong is not a cosmetic
//! bug: a second `register` that overwrote the first would leave the first
//! strip reserved with nothing left that knows how to give it back.
//!
//! # The hazard
//!
//! **An appbar that is registered and never removed leaves dead space at the
//! edge of every screen**, surviving reboots, with nothing on screen to explain
//! it. It is the most user-hostile thing this program can do, so removal is
//! wired to every exit there is — the switch, the window being destroyed, and
//! the process ending — and [`reset_work_area`] exists for the case where all
//! three were missed, reachable from `mino shell-reset` without the app.
//!
//! # Why the window is subclassed
//!
//! Two messages have to be heard or the reservation quietly stops being true:
//!
//! - **`TaskbarCreated`.** When Explorer restarts it destroys every registered
//!   appbar and broadcasts this to say so. That is not a rare event here:
//!   applying a Look restarts Explorer, so a bar that did not listen would lose
//!   its strip *as part of being switched on*. The broadcast reaches every
//!   top-level window, so each of ours re-registers itself.
//! - **`ABN_POSCHANGED`.** The taskbar moved, the resolution changed, or
//!   another appbar appeared, and our rectangle may no longer be where we asked.
//!
//! Subclassing is `SetWindowSubclass` on **our own window**, forwarding
//! everything to `DefSubclassProc` — the documented, cooperative way to see
//! messages a window we own already receives. Nothing is hooked, nothing is
//! injected, and no other process is touched.

use std::sync::{Mutex, OnceLock, PoisonError};

use windows::core::w;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::UI::Shell::{
    DefSubclassProc, RemoveWindowSubclass, SHAppBarMessage, SetWindowSubclass, ABE_BOTTOM,
    ABE_LEFT, ABE_RIGHT, ABE_TOP, ABM_NEW, ABM_QUERYPOS, ABM_REMOVE, ABM_SETPOS,
    ABM_WINDOWPOSCHANGED, ABN_POSCHANGED, APPBARDATA,
};
use windows::Win32::UI::WindowsAndMessaging::{
    RegisterWindowMessageW, SystemParametersInfoW, SPIF_SENDCHANGE, SPI_SETWORKAREA, WM_APP,
    WM_NCDESTROY,
};

use crate::{screen_area, Edge, WorkArea};

/// Ours to choose, and only ever sent back to the window that registered it.
const CALLBACK_MESSAGE: u32 = WM_APP + 0x40;

/// Identifies our subclass on the window, so it can be removed again.
const SUBCLASS_ID: usize = 0x6D69_6E6F; // "mino"

/// One reserved strip, and the window that holds it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Bar {
    hwnd: isize,
    edge: Edge,
    thickness: i32,
}

/// Every strip this process currently holds. Small enough — there are three
/// surfaces in the whole program — that a list beats a map.
static BARS: Mutex<Vec<Bar>> = Mutex::new(Vec::new());

fn bars<T>(f: impl FnOnce(&mut Vec<Bar>) -> T) -> T {
    let mut guard = BARS.lock().unwrap_or_else(PoisonError::into_inner);
    f(&mut guard)
}

fn find(hwnd: isize) -> Option<Bar> {
    bars(|list| list.iter().find(|bar| bar.hwnd == hwnd).copied())
}

/// The `TaskbarCreated` broadcast, registered once and cached.
fn taskbar_created() -> u32 {
    static ID: OnceLock<u32> = OnceLock::new();
    *ID.get_or_init(|| unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) })
}

fn handle(hwnd: isize) -> HWND {
    HWND(hwnd as *mut std::ffi::c_void)
}

fn data_for(bar: Bar) -> APPBARDATA {
    APPBARDATA {
        cbSize: std::mem::size_of::<APPBARDATA>() as u32,
        hWnd: handle(bar.hwnd),
        uCallbackMessage: CALLBACK_MESSAGE,
        uEdge: match bar.edge {
            Edge::Top => ABE_TOP,
            Edge::Bottom => ABE_BOTTOM,
            Edge::Left => ABE_LEFT,
            Edge::Right => ABE_RIGHT,
        },
        rc: RECT::default(),
        lParam: LPARAM(0),
    }
}

/// Reserves a strip and returns the rectangle Windows actually granted.
///
/// The granted rectangle is not always the one asked for — the taskbar, or our
/// own other surface, may already hold part of that edge — which is why the
/// caller places its window to what comes back rather than to what it wanted.
///
/// Registering a window that already holds a strip moves it: the old
/// reservation is given back first, so switching a dock from the bottom of the
/// screen to the left cannot leave a band behind at the bottom.
pub fn register(hwnd: isize, edge: Edge, thickness: i32) -> Option<WorkArea> {
    if find(hwnd).is_some() {
        unregister(hwnd);
    }

    let bar = Bar {
        hwnd,
        edge,
        thickness: thickness.max(0),
    };

    unsafe {
        let mut data = data_for(bar);
        if SHAppBarMessage(ABM_NEW, &mut data) == 0 {
            return None;
        }
        // Only after ABM_NEW: the window has to be an appbar before it can be
        // told about Explorer coming back.
        let _ = SetWindowSubclass(handle(hwnd), Some(subclass), SUBCLASS_ID, 0);
    }

    bars(|list| list.push(bar));
    unsafe { position(bar) }
}

/// Gives one window's strip back. Safe to call for a window that holds none.
pub fn unregister(hwnd: isize) {
    let Some(bar) = bars(|list| {
        let index = list.iter().position(|bar| bar.hwnd == hwnd)?;
        Some(list.remove(index))
    }) else {
        return;
    };
    unsafe {
        let _ = RemoveWindowSubclass(handle(bar.hwnd), Some(subclass), SUBCLASS_ID);
        let mut data = data_for(bar);
        SHAppBarMessage(ABM_REMOVE, &mut data);
    }
}

/// Gives every strip back. What the process calls on its way out.
pub fn unregister_all() {
    let held = bars(std::mem::take);
    for bar in held {
        unsafe {
            let _ = RemoveWindowSubclass(handle(bar.hwnd), Some(subclass), SUBCLASS_ID);
            let mut data = data_for(bar);
            SHAppBarMessage(ABM_REMOVE, &mut data);
        }
    }
}

/// Whether this window currently holds a strip.
pub fn is_registered(hwnd: isize) -> bool {
    find(hwnd).is_some()
}

/// Asks for the rectangle, takes what is given, and tells Windows where the
/// window ended up.
///
/// The `ABM_QUERYPOS` → adjust → `ABM_SETPOS` dance is the documented one:
/// Windows moves the proposed edge to clear anything already there, and the
/// caller then pins the opposite edge to keep its own thickness.
unsafe fn position(bar: Bar) -> Option<WorkArea> {
    let monitor = screen_area();
    let wanted = crate::bar_rect(monitor, bar.edge, bar.thickness);

    let mut data = data_for(bar);
    data.rc = RECT {
        left: wanted.x,
        top: wanted.y,
        right: wanted.x + wanted.width,
        bottom: wanted.y + wanted.height,
    };

    SHAppBarMessage(ABM_QUERYPOS, &mut data);

    match bar.edge {
        Edge::Top => data.rc.bottom = data.rc.top + bar.thickness,
        Edge::Bottom => data.rc.top = data.rc.bottom - bar.thickness,
        Edge::Left => data.rc.right = data.rc.left + bar.thickness,
        Edge::Right => data.rc.left = data.rc.right - bar.thickness,
    }

    SHAppBarMessage(ABM_SETPOS, &mut data);

    let granted = WorkArea {
        x: data.rc.left,
        y: data.rc.top,
        width: (data.rc.right - data.rc.left).max(0),
        height: (data.rc.bottom - data.rc.top).max(0),
    };

    SHAppBarMessage(ABM_WINDOWPOSCHANGED, &mut data);
    Some(granted)
}

/// Re-asks for one window's strip. `None` when it holds none.
///
/// Called after the taskbar moves, which arrives as a message on the window and
/// not as anything we could poll for.
pub fn reposition(hwnd: isize) -> Option<WorkArea> {
    let bar = find(hwnd)?;
    unsafe { position(bar) }
}

/// Registers again from scratch, for after Explorer has taken every appbar with
/// it. `ABM_NEW` on a window Windows has forgotten is how it gets back.
fn readd(hwnd: isize) -> Option<WorkArea> {
    let bar = find(hwnd)?;
    unsafe {
        let mut data = data_for(bar);
        if SHAppBarMessage(ABM_NEW, &mut data) == 0 {
            return None;
        }
        position(bar)
    }
}

/// Everything this window would have received anyway, plus the two messages the
/// reservation depends on.
///
/// A panic across an FFI boundary is undefined behaviour, so nothing in here is
/// allowed to unwrap: it reads a little state, makes a call that cannot panic,
/// and hands the message on.
unsafe extern "system" fn subclass(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    _data: usize,
) -> LRESULT {
    let this = hwnd.0 as isize;

    if message == taskbar_created() {
        // Explorer came back and took every appbar with it, ours included.
        if let Some(area) = readd(this) {
            moved(hwnd, area);
        }
    } else if message == CALLBACK_MESSAGE && wparam.0 as u32 == ABN_POSCHANGED {
        if let Some(area) = reposition(this) {
            moved(hwnd, area);
        }
    } else if message == WM_NCDESTROY {
        // The last message a window gets. Whatever else happened, the strip is
        // given back here rather than left behind.
        unregister(this);
    }

    DefSubclassProc(hwnd, message, wparam, lparam)
}

/// Puts the window where Windows moved its reservation to.
unsafe fn moved(hwnd: HWND, area: WorkArea) {
    use windows::Win32::UI::WindowsAndMessaging::{SetWindowPos, HWND_TOPMOST, SWP_NOACTIVATE};
    // Topmost is re-asserted rather than left alone: the two moments this runs
    // are Explorer restarting and the taskbar moving, and both are exactly when
    // a bar can end up behind something. `SWP_NOACTIVATE` keeps the focus where
    // the user left it.
    let _ = SetWindowPos(
        hwnd,
        HWND_TOPMOST,
        area.x,
        area.y,
        area.width,
        area.height,
        SWP_NOACTIVATE,
    );
}

/// The recovery: give the whole screen back to the desktop.
///
/// `SPI_SETWORKAREA` with a null rectangle sets the work area to the full
/// screen, which clears a reservation left behind by a process that died before
/// it could remove its own. It clears *every* reservation, the taskbar's
/// included, so windows may maximize under the taskbar until Explorer asserts
/// itself again — restarting Explorer, or signing out, puts that back. That is
/// the right trade for a command someone only runs when there is a strip of
/// dead screen they cannot otherwise explain.
pub fn reset_work_area() -> bool {
    unregister_all();
    unsafe { SystemParametersInfoW(SPI_SETWORKAREA, 0, None, SPIF_SENDCHANGE).is_ok() }
}
