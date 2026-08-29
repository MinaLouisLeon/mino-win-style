import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";

import { dockApi, type DockItem, type DockLayout } from "./api";
import { useIcons } from "./icons";
import { Menu, type MenuAction } from "./Menu";

/** How far the magnification reaches, in multiples of the icon size. */
const REACH = 1.6;
/** How much the icon under the cursor grows. */
const LIFT = 0.9;
/** Space between icons at rest. */
const GAP = 8;
/** Padding inside the dock panel. */
const PAD = 10;
/** Room above the panel for the biggest an icon can get, plus its label. */
const HEADROOM = 62;
/** Gap between the top of the dock panel and an open menu. */
const MENU_GAP = 10;
/** Narrowest the dock is allowed to get, so an empty one is still a target. */
const MIN_WIDTH = 120;
/** Breathing room either side of an open menu. */
const MENU_MARGIN = 8;
/** At most this many window entries, so a browser with thirty tabs open does
 *  not produce a menu taller than the screen. */
const MAX_WINDOWS = 8;

/**
 * The classic dock magnification: an icon's scale falls off with distance from
 * the cursor. A Gaussian rather than a linear ramp, because the linear one has
 * a visible corner where the effect stops and reads as mechanical.
 */
function scaleFor(distance: number, iconSize: number): number {
  const reach = iconSize * REACH;
  if (distance > reach) return 1;
  const t = distance / reach;
  return 1 + LIFT * Math.exp(-(t * t) * 3.2);
}

interface MenuState {
  item: DockItem;
  anchorX: number;
}

export function Dock() {
  const [items, setItems] = useState<DockItem[]>([]);
  const [layout, setLayout] = useState<DockLayout | null>(null);
  const [pinned, setPinned] = useState<string[]>([]);
  const [cursor, setCursor] = useState<number | null>(null);
  const [hovered, setHovered] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [menu, setMenu] = useState<MenuState | null>(null);
  const [menuHeight, setMenuHeight] = useState(0);
  /** False while the dock is switched off; the window still exists, hidden. */
  const [active, setActive] = useState(true);

  const panel = useRef<HTMLDivElement>(null);
  const menuBox = useRef<HTMLDivElement | null>(null);
  const icons = useIcons(items, layout?.icon_size ?? 48);

  useEffect(() => {
    dockApi
      .layout()
      .then(setLayout)
      .catch((err) => setError(String(err)));
    dockApi
      .config()
      .then((config) => setPinned(config.pinned))
      .catch(() => {});
  }, []);

  const refresh = useCallback(() => {
    dockApi
      .items()
      .then((next) => {
        setItems(next);
        setError(null);
      })
      // Shown on the dock itself. A dock that silently renders nothing is
      // indistinguishable from one that never started.
      .catch((err) => setError(String(err)));
  }, []);

  // The window outlives the dock being switched off — it is only hidden — so
  // Rust says when it is on screen and the page stops looking at the desktop
  // while nobody can see it.
  useEffect(() => {
    let stop: (() => void) | undefined;
    import("@tauri-apps/api/event")
      .then(({ listen }) =>
        listen<boolean>("dock-active", (event) => setActive(event.payload)),
      )
      .then((unlisten) => {
        stop = unlisten;
      })
      .catch(() => {});
    return () => stop?.();
  }, []);

  useEffect(() => {
    if (!active) return;
    refresh();
    // Polling, deliberately: a proper SetWinEventHook subscription is the right
    // answer and is not built yet. Every 1.2s is quick enough to feel live and
    // cheap enough not to matter — the whole call is an EnumWindows sweep.
    // Paused while a menu is open, so the list cannot shift under the cursor.
    if (menu) return;
    const timer = setInterval(refresh, 1200);
    return () => clearInterval(timer);
  }, [refresh, menu, active]);

  const iconSize = layout?.icon_size ?? 48;

  // Measured after the menu renders, because the window has to grow to fit it —
  // the menu cannot paint outside its own window.
  useLayoutEffect(() => {
    setMenuHeight(menu && menuBox.current ? menuBox.current.scrollHeight : 0);
  }, [menu, items]);

  // Tell Rust how big to make the window, computed rather than measured.
  //
  // Measuring the panel cannot work: the panel lives *inside* this window, so
  // its width is clamped by the window's width. Sizing the window from that
  // measurement is a feedback loop — the first pass runs with no icons yet,
  // shrinks the window to the width of the empty panel, and from then on the
  // panel can never measure wider, so the dock stays stuck at that size no
  // matter how many icons arrive. The slots are fixed-size, so arithmetic is
  // both exact and immune to it.
  useEffect(() => {
    const count = items.length;
    const iconsWidth =
      count > 0 ? PAD * 2 + count * iconSize + (count - 1) * GAP : MIN_WIDTH;
    // scrollWidth, not offsetWidth: same clamping problem applies to the menu.
    const menuWidth = menuBox.current?.scrollWidth ?? 0;
    const width = Math.max(iconsWidth, menuWidth + MENU_MARGIN * 2, MIN_WIDTH);

    const extra = menuHeight > 0 ? menuHeight + MENU_GAP : 0;
    const height = PAD * 2 + iconSize + HEADROOM + extra;

    dockApi.place(width, height).catch(() => {});
  }, [items.length, iconSize, menuHeight, active]);

  const isPinned = (exe: string) =>
    pinned.some((p) => p.toLowerCase() === exe.toLowerCase());

  const open = (item: DockItem) => {
    if (item.windows.length > 0) {
      dockApi.activate(item.windows[0].hwnd).catch(() => {});
    } else {
      dockApi.launch(item.exe).catch(() => {});
    }
  };

  /** What right-clicking an icon offers. Mirrors what a dock can actually do. */
  const actionsFor = (item: DockItem): MenuAction[] => {
    const actions: MenuAction[] = [];
    const front = item.windows[0];

    actions.push({
      id: "new",
      label: item.windows.length > 0 ? "New window" : "Open",
      run: () => void dockApi.launch(item.exe).catch(() => {}),
    });

    item.windows.slice(0, MAX_WINDOWS).forEach((window, index) => {
      actions.push({
        id: `window-${window.hwnd}`,
        label: trim(window.title),
        separatorBefore: index === 0,
        run: () => void dockApi.activate(window.hwnd).catch(() => {}),
      });
    });

    if (front) {
      actions.push({
        id: "minimize",
        label: front.minimized ? "Restore" : "Minimise",
        separatorBefore: true,
        run: () =>
          void (front.minimized
            ? dockApi.activate(front.hwnd)
            : dockApi.minimize(front.hwnd)
          ).catch(() => {}),
      });
      actions.push({
        id: "maximize",
        label: front.maximized ? "Unmaximise" : "Maximise",
        run: () => void dockApi.toggleMaximize(front.hwnd).catch(() => {}),
      });
      actions.push({
        id: "close",
        label: item.windows.length > 1 ? "Close all windows" : "Close",
        run: () => {
          for (const window of item.windows) {
            void dockApi.close(window.hwnd).catch(() => {});
          }
        },
      });
    }

    const wasPinned = isPinned(item.exe);
    actions.push({
      id: "pin",
      label: "Keep in Dock",
      checked: wasPinned,
      separatorBefore: true,
      run: () => {
        const call = wasPinned ? dockApi.unpin(item.exe) : dockApi.pin(item.exe);
        call
          .then((config) => {
            setPinned(config.pinned);
            refresh();
          })
          .catch((err) => setError(String(err)));
      },
    });

    return actions;
  };

  const panelHeight = (panel.current?.offsetHeight ?? iconSize + PAD * 2) + MENU_GAP;

  return (
    <div
      className="stage"
      onMouseLeave={() => {
        setCursor(null);
        setMenu(null);
      }}
      // A click anywhere off the menu dismisses it, the way a menu should.
      onMouseDown={() => setMenu(null)}
    >
      {menu && (
        <Menu
          item={menu.item}
          actions={actionsFor(menu.item)}
          anchorX={menu.anchorX}
          bottom={panelHeight}
          onClose={() => setMenu(null)}
          measureRef={menuBox}
        />
      )}

      <div
        className="dock"
        ref={panel}
        style={{ padding: PAD, gap: GAP }}
        onMouseMove={(e) => {
          if (menu) return; // keep the icons still while choosing from a menu
          const box = e.currentTarget.getBoundingClientRect();
          setCursor(e.clientX - box.left);
        }}
      >
        {error && <span className="oops">{error}</span>}
        {!error && items.length === 0 && <span className="oops">no windows yet</span>}

        {items.map((item, index) => {
          // Where this icon sits at rest, measured to its centre.
          const centre = PAD + index * (iconSize + GAP) + iconSize / 2;
          const scale = cursor === null ? 1 : scaleFor(Math.abs(cursor - centre), iconSize);
          const size = iconSize * scale;
          const icon = icons.get(item.exe.toLowerCase());
          const isOpen = menu?.item.exe === item.exe;

          return (
            <button
              type="button"
              key={item.exe}
              className={`slot${isOpen ? " slot--open" : ""}`}
              style={{ width: iconSize, height: iconSize }}
              title={item.name}
              onMouseEnter={() => setHovered(item.exe)}
              onMouseLeave={() => setHovered(null)}
              onClick={() => open(item)}
              onContextMenu={(e) => {
                e.preventDefault();
                const box = e.currentTarget.getBoundingClientRect();
                setMenu({ item, anchorX: box.left + box.width / 2 });
                setCursor(null);
              }}
            >
              {hovered === item.exe && !menu && <span className="label">{item.name}</span>}
              <span
                className="icon"
                style={{
                  width: size,
                  height: size,
                  // Anchored to the bottom, so icons grow upward out of the
                  // dock rather than pushing through its floor.
                  marginBottom: (iconSize - size) / 2,
                }}
              >
                {icon ? (
                  <img src={icon} alt="" draggable={false} />
                ) : (
                  <span className="fallback">{item.name.slice(0, 1)}</span>
                )}
              </span>
              <span className={`dot${item.windows.length > 0 ? " dot--on" : ""}`} />
            </button>
          );
        })}
      </div>
    </div>
  );
}

/** Window titles get long; a menu is not the place to read one in full. */
function trim(title: string): string {
  const clean = title.trim();
  return clean.length > 42 ? `${clean.slice(0, 41)}…` : clean;
}
