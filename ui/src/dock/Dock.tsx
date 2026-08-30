import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
} from "react";

import { watchShellLook, type LookId } from "../lib/shell-look";
import { dockApi, type DockItem, type DockLayout, type Edge } from "./api";
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
/** Gap between the top of an icon and the name shown above it. */
const LABEL_GAP = 12;
/** Height of the name bubble, so the window is tall enough to show it. */
const LABEL_HEIGHT = 26;
/** Slack above the tallest thing the stage can hold. */
const HEADROOM = 4;
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
 * Where an icon's name sits: always on the inner side of the dock, so it reads
 * over the desktop rather than off the edge of the screen.
 */
function labelOffset(edge: Edge, size: number): CSSProperties {
  const away = size + LABEL_GAP;
  switch (edge) {
    case "left":
      return { left: away };
    case "right":
      return { right: away };
    case "top":
      return { top: away };
    default:
      return { bottom: away };
  }
}

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

/**
 * Everything below works in *rest coordinates*: where the icons sit when
 * nothing is hovered, measured from the left edge of the unmagnified panel.
 *
 * That indirection is the whole trick. A magnified icon takes up more room, so
 * the panel it lives in gets wider and its neighbours slide outwards — which
 * means reading the cursor against the panel's *current* layout would feed the
 * effect back into itself and the icons would judder. The rest layout never
 * moves, so the same cursor position always picks out the same icon.
 */

/** Where an icon's centre sits at rest. */
function restCentre(index: number, iconSize: number): number {
  return PAD + index * (iconSize + GAP) + iconSize / 2;
}

/** How wide the panel is with nothing hovered. */
function restWidth(count: number, iconSize: number): number {
  return count > 0 ? PAD * 2 + count * iconSize + (count - 1) * GAP : MIN_WIDTH;
}

/** The scale of every icon, for a cursor at `x`. Null while nothing hovers. */
function scalesFor(count: number, iconSize: number, x: number | null): number[] {
  return Array.from({ length: count }, (_, index) =>
    x === null ? 1 : scaleFor(Math.abs(x - restCentre(index, iconSize)), iconSize),
  );
}

/** Extra width the magnified icons add over their resting size. */
function spreadOf(scales: number[], iconSize: number): number {
  return scales.reduce((total, scale) => total + (scale - 1) * iconSize, 0);
}

/**
 * How far to slide the panel so the icon under the cursor stays under it.
 *
 * The panel grows around its centre, so without this the icons either side of
 * the cursor would drift out from under it. Sliding by the growth that happened
 * to the left of the cursor cancels that exactly: point `x` in the rest layout
 * lands at `x` on screen, whatever the magnification is doing around it.
 */
function slideFor(scales: number[], iconSize: number, x: number | null): number {
  if (x === null) return 0;
  let slide = 0;
  scales.forEach((scale, index) => {
    const left = PAD + index * (iconSize + GAP);
    // How much of this icon the cursor has passed: 0 before it, 1 after it.
    const through = Math.min(Math.max((x - left) / iconSize, 0), 1);
    slide += (scale - 1) * iconSize * through;
  });
  return slide;
}

/**
 * The widest the panel can ever get, sampled across every cursor position.
 *
 * The window has to be big enough for the fully magnified panel *before* the
 * mouse arrives — resizing it mid-hover would mean a round trip to Rust on
 * every frame, and the dock would lag behind the cursor.
 */
function widestPanel(count: number, iconSize: number): number {
  const rest = restWidth(count, iconSize);
  if (count === 0) return rest;
  let widest = rest;
  for (let x = 0; x <= rest; x += 4) {
    widest = Math.max(widest, rest + spreadOf(scalesFor(count, iconSize, x), iconSize));
  }
  return widest;
}

interface MenuState {
  item: DockItem;
  /** Where the icon rests along the edge the dock is on. */
  anchor: number;
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
  /**
   * Which Look is worn, for the one thing that is behaviour rather than colour.
   *
   * Zen holds the magnification still: a dock that leaps at the cursor is the
   * opposite of what that Look is for. Everything else a Look does to the dock
   * is a block of CSS and never reaches this file.
   */
  const [look, setLook] = useState<LookId | null>(null);
  useEffect(() => watchShellLook((config) => setLook(config.active)), []);

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
  // Revealed from the edge, the dock should arrive rather than appear. The
  // class is dropped again once the slide is over, so the next reveal replays
  // it — an animation left on the element would only ever run once.
  const [arriving, setArriving] = useState(false);
  useEffect(() => {
    if (!active) return;
    setArriving(true);
    const id = window.setTimeout(() => setArriving(false), 260);
    return () => window.clearTimeout(id);
  }, [active]);

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
  /**
   * Which edge the dock is on, and so which way everything below runs.
   *
   * All the magnification arithmetic is one-dimensional already — it works in
   * "rest coordinates" along the dock — so standing the dock up is a matter of
   * feeding it the other cursor axis and swapping which side of a slot grows.
   * There is no second copy of the maths.
   */
  const edge: Edge = layout?.edge ?? "bottom";
  const vertical = edge === "left" || edge === "right";

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
  // matter how many icons arrive. Every slot's size follows from the icon size
  // and the cursor, so arithmetic is both exact and immune to it.
  const rest = restWidth(items.length, iconSize);
  const widest = useMemo(
    () => widestPanel(items.length, iconSize),
    [items.length, iconSize],
  );

  useEffect(() => {
    // Room for the panel at its widest *and* for the slide that keeps the
    // hovered icon under the cursor, which can push it a whole spread either
    // way. Symmetrical, because the resting panel is centred in the window.
    const spread = widest - rest;
    // scrollWidth/Height, not offsetWidth/Height: the menu lives inside this
    // window too, so the same clamping problem applies to measuring it.
    const menuAcross = menuBox.current?.scrollWidth ?? 0;

    // Along the edge: as long as the panel can get.
    const along = Math.max(rest + spread * 2, MIN_WIDTH);
    // Across it: the biggest an icon gets, plus the name above it. The icons
    // grow out of the panel, so this is what stops them being clipped.
    const thickness = PAD + iconSize * (1 + LIFT) + LABEL_GAP + LABEL_HEIGHT + HEADROOM;

    if (vertical) {
      // A menu opens beside a standing dock, so it adds to the thickness of the
      // window — but never to `thickness` itself, which is what the desktop
      // gives up when this edge reserves. A strip that grew every time a menu
      // opened would move every maximized window with it.
      const extra = menuAcross > 0 ? menuAcross + MENU_GAP : 0;
      dockApi
        .place(thickness + extra, Math.max(along, menuHeight + MENU_MARGIN * 2), thickness)
        .catch(() => {});
    } else {
      const extra = menuHeight > 0 ? menuHeight + MENU_GAP : 0;
      dockApi
        .place(Math.max(along, menuAcross + MENU_MARGIN * 2), thickness + extra, thickness)
        .catch(() => {});
    }
  }, [rest, widest, iconSize, menuHeight, active, vertical]);

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

  // How far in from the dock's edge the menu has to start to clear the panel.
  const panelReach =
    (vertical
      ? (panel.current?.offsetWidth ?? iconSize + PAD * 2)
      : (panel.current?.offsetHeight ?? iconSize + PAD * 2)) + MENU_GAP;

  // Where the magnification is centred, in rest coordinates. Nowhere while a
  // menu is open: the icons hold still while you choose from it.
  const at = menu || look === "zen" ? null : cursor;
  const scales = scalesFor(items.length, iconSize, at);
  // Flexbox centres the panel on its grown width; sliding it back by half the
  // growth returns it to where it rests, and the rest of the slide keeps the
  // icon under the cursor there.
  const slide = spreadOf(scales, iconSize) / 2 - slideFor(scales, iconSize, at);

  return (
    <div
      className="stage"
      data-edge={edge}
      data-arriving={arriving || undefined}
      // Tracked here rather than on the panel: the icons stand well above it
      // once they grow, and the effect should start as the cursor comes down
      // towards the dock rather than snapping on at its edge.
      onMouseMove={(e) => {
        if (menu) return; // keep the icons still while choosing from a menu
        const box = e.currentTarget.getBoundingClientRect();
        // Into rest coordinates, along whichever axis the dock runs: the
        // resting panel is centred in the window on that axis.
        setCursor(
          vertical
            ? e.clientY - box.top - (box.height - rest) / 2
            : e.clientX - box.left - (box.width - rest) / 2,
        );
      }}
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
          anchor={menu.anchor}
          offset={panelReach}
          edge={edge}
          onClose={() => setMenu(null)}
          measureRef={menuBox}
        />
      )}

      <div
        className="dock"
        ref={panel}
        style={{
          padding: PAD,
          gap: GAP,
          transform: vertical ? `translateY(${slide}px)` : `translateX(${slide}px)`,
        }}
      >
        {error && <span className="oops">{error}</span>}
        {!error && items.length === 0 && <span className="oops">no windows yet</span>}

        {items.map((item, index) => {
          const size = iconSize * scales[index];
          const icon = icons.get(item.exe.toLowerCase());
          const isOpen = menu?.item.exe === item.exe;

          return (
            <button
              type="button"
              key={item.exe}
              className={`slot${isOpen ? " slot--open" : ""}`}
              // The slot is as big as the icon it holds *along* the dock, so a
              // growing icon takes its room from the layout and its neighbours
              // step aside instead of being sat on top of. Across the dock it
              // stays the resting size; the icon grows out of it.
              style={
                vertical
                  ? { width: iconSize, height: size }
                  : { width: size, height: iconSize }
              }
              title={item.name}
              onMouseEnter={() => setHovered(item.exe)}
              onMouseLeave={() => setHovered(null)}
              onClick={() => open(item)}
              onContextMenu={(e) => {
                e.preventDefault();
                // Anchored to where the icon rests, not to where the cursor
                // just magnified it to: opening the menu drops the
                // magnification, so by the time the menu paints the icon is
                // back at its resting place and a live measurement would point
                // the menu at somewhere the icon no longer is.
                const window_extent = vertical ? window.innerHeight : window.innerWidth;
                const anchor = (window_extent - rest) / 2 + restCentre(index, iconSize);
                setMenu({ item, anchor });
                setCursor(null);
              }}
            >
              {hovered === item.exe && !menu && (
                // Sits above the icon at whatever size it currently is, rather
                // than at a fixed height the magnified icon would grow through.
                <span className="label" style={labelOffset(edge, size)}>
                  {item.name}
                </span>
              )}
              <span
                className="icon"
                // Bottom-anchored by the slot's flex-end, so an icon bigger
                // than its slot grows up out of the dock rather than down
                // through its floor.
                style={{ width: size, height: size }}
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
