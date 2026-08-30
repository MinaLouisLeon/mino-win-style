import { useEffect, useRef } from "react";

import type { DockItem, Edge } from "./api";

export interface MenuAction {
  /** Stable id, used for keys and for deciding what to run. */
  id: string;
  label: string;
  /** Shown with a tick on the right. */
  checked?: boolean;
  /** Draws a hairline above this item. */
  separatorBefore?: boolean;
  run: () => void;
}

interface Props {
  item: DockItem;
  actions: MenuAction[];
  /** Centre of the icon this belongs to, along the edge the dock is on. */
  anchor: number;
  /** How far in from that edge the panel reaches, so the menu clears it. */
  offset: number;
  /** Which edge the dock is on: the menu opens away from it. */
  edge: Edge;
  onClose: () => void;
  measureRef: React.RefObject<HTMLDivElement | null>;
}

const MARGIN = 8;

/**
 * The dock's context menu.
 *
 * Hand-written rather than pulled from a library, so it matches the dock rather
 * than the operating system: this is the menu a dock shows, not the menu a
 * window shows. That means the keyboard handling and dismissal below are ours
 * to get right — arrow keys, Escape, Enter, and clicking away.
 */
export function Menu({ item, actions, anchor, offset, edge, onClose, measureRef }: Props) {
  const list = useRef<HTMLDivElement>(null);

  useEffect(() => {
    // Focus the menu itself, not an item: the first arrow key picks the first
    // item, which is what a menu opened by the mouse should do.
    list.current?.focus();
  }, []);

  const onKeyDown = (e: React.KeyboardEvent) => {
    const buttons = Array.from(
      list.current?.querySelectorAll<HTMLButtonElement>("button:not(:disabled)") ?? [],
    );
    if (buttons.length === 0) return;
    const here = buttons.indexOf(document.activeElement as HTMLButtonElement);

    switch (e.key) {
      case "Escape":
        e.preventDefault();
        onClose();
        break;
      case "ArrowDown":
        e.preventDefault();
        buttons[(here + 1) % buttons.length].focus();
        break;
      case "ArrowUp":
        e.preventDefault();
        buttons[(here - 1 + buttons.length) % buttons.length].focus();
        break;
      case "Home":
        e.preventDefault();
        buttons[0].focus();
        break;
      case "End":
        e.preventDefault();
        buttons[buttons.length - 1].focus();
        break;
    }
  };

  // Clamped so a menu on the first or last icon still lands inside the window,
  // which is the only surface we are allowed to paint on.
  const width = measureRef.current?.offsetWidth ?? 220;
  const height = measureRef.current?.offsetHeight ?? 160;

  const clamp = (value: number, size: number, limit: number) =>
    Math.min(Math.max(value - size / 2, MARGIN), Math.max(limit - size - MARGIN, MARGIN));

  // The menu opens away from the edge the dock is on: upwards from a dock along
  // the bottom, sideways from one down a side.
  const style =
    edge === "left"
      ? { top: clamp(anchor, height, window.innerHeight), left: offset }
      : edge === "right"
        ? { top: clamp(anchor, height, window.innerHeight), right: offset }
        : edge === "top"
          ? { left: clamp(anchor, width, window.innerWidth), top: offset }
          : { left: clamp(anchor, width, window.innerWidth), bottom: offset };

  return (
    <div
      className="menu"
      ref={(node) => {
        list.current = node;
        measureRef.current = node;
      }}
      role="menu"
      aria-label={item.name}
      tabIndex={-1}
      style={style}
      onKeyDown={onKeyDown}
      // Stops the click that chose an item from also counting as a click on the
      // stage, which is what dismisses the menu.
      onMouseDown={(e) => e.stopPropagation()}
      onContextMenu={(e) => e.preventDefault()}
    >
      <div className="menu__title">{item.name}</div>
      {actions.map((action) => (
        <button
          type="button"
          key={action.id}
          role="menuitem"
          className={`menu__item${action.separatorBefore ? " menu__item--sep" : ""}`}
          onClick={() => {
            action.run();
            onClose();
          }}
        >
          <span className="menu__label">{action.label}</span>
          {action.checked && (
            <span className="menu__tick" aria-hidden="true">
              ✓
            </span>
          )}
        </button>
      ))}
    </div>
  );
}
