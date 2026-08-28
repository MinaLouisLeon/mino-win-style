import { useEffect, useRef, useState } from "react";

import { dockApi, type DockItem } from "./api";

/**
 * Turns the raw RGBA the Rust side extracts from each executable into something
 * an `<img>` can show, once per executable.
 *
 * Icons are fetched exactly once and kept for the life of the dock: they come
 * out of files on disk that do not change while the app is open, and the list
 * of running programs is re-read every second or so.
 */
export function useIcons(items: DockItem[], size: number): Map<string, string> {
  const [icons, setIcons] = useState<Map<string, string>>(new Map());
  const inFlight = useRef<Set<string>>(new Set());

  useEffect(() => {
    let cancelled = false;

    for (const item of items) {
      const key = item.exe.toLowerCase();
      if (icons.has(key) || inFlight.current.has(key)) continue;
      inFlight.current.add(key);

      dockApi
        .icon(item.exe, size)
        .then((data) => {
          if (cancelled || !data) return;
          const url = toDataUrl(data.rgba_base64, data.width, data.height);
          if (!url) return;
          setIcons((current) => new Map(current).set(key, url));
        })
        .catch(() => {
          // An icon we cannot read is not worth a broken dock; the slot falls
          // back to the first letter of the name.
        })
        .finally(() => inFlight.current.delete(key));
    }

    return () => {
      cancelled = true;
    };
  }, [items, size, icons]);

  return icons;
}

function toDataUrl(base64: string, width: number, height: number): string | null {
  try {
    const binary = atob(base64);
    const bytes = new Uint8ClampedArray(binary.length);
    for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);

    const canvas = document.createElement("canvas");
    canvas.width = width;
    canvas.height = height;
    const context = canvas.getContext("2d");
    if (!context) return null;
    context.putImageData(new ImageData(bytes, width, height), 0, 0);
    return canvas.toDataURL("image/png");
  } catch {
    return null;
  }
}
