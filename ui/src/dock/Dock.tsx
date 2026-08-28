import { useCallback, useEffect, useRef, useState } from "react";

import { dockApi, type DockItem, type DockLayout } from "./api";
import { useIcons } from "./icons";

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

export function Dock() {
  const [items, setItems] = useState<DockItem[]>([]);
  const [layout, setLayout] = useState<DockLayout | null>(null);
  const [cursor, setCursor] = useState<number | null>(null);
  const [hovered, setHovered] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const panel = useRef<HTMLDivElement>(null);
  const icons = useIcons(items, layout?.icon_size ?? 48);

  useEffect(() => {
    dockApi
      .layout()
      .then(setLayout)
      .catch((err) => setError(String(err)));
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

  useEffect(() => {
    refresh();
    // Polling, deliberately: a proper SetWinEventHook subscription is the right
    // answer and is not built yet. Every 1.2s is quick enough to feel live and
    // cheap enough not to matter — the whole call is an EnumWindows sweep.
    const timer = setInterval(refresh, 1200);
    return () => clearInterval(timer);
  }, [refresh]);

  const iconSize = layout?.icon_size ?? 48;

  // Tell Rust how big we ended up, so it can centre us along the bottom edge.
  // This is also what makes the window visible, so it must run even with an
  // empty dock — a dock that stays hidden when something goes wrong is
  // indistinguishable from one that was never created.
  useEffect(() => {
    if (!panel.current) return;
    const width = Math.max(panel.current.offsetWidth, 80);
    const height = panel.current.offsetHeight + HEADROOM;
    dockApi.place(width, height).catch(() => {});
  }, [items.length, iconSize]);

  const open = (item: DockItem) => {
    if (item.windows.length > 0) {
      dockApi.activate(item.windows[0].hwnd).catch(() => {});
    } else {
      dockApi.launch(item.exe).catch(() => {});
    }
  };

  return (
    <div className="stage" onMouseLeave={() => setCursor(null)}>
      <div
        className="dock"
        ref={panel}
        style={{ padding: PAD, gap: GAP }}
        onMouseMove={(e) => {
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

          return (
            <button
              type="button"
              key={item.exe}
              className="slot"
              style={{ width: iconSize, height: iconSize }}
              title={item.name}
              onMouseEnter={() => setHovered(item.exe)}
              onMouseLeave={() => setHovered(null)}
              onClick={() => open(item)}
            >
              {hovered === item.exe && <span className="label">{item.name}</span>}
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
