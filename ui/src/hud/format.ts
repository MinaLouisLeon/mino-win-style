/**
 * Turning raw counters into something readable at a glance.
 *
 * Rust hands over bytes and seconds and nothing else, deliberately: how a
 * number is written is a display decision, and one that differs by language.
 * Digits stay Western here even in Arabic — a readout you are meant to scan
 * without reading is easier to scan when the shapes never change, which is the
 * same reason the app wraps build numbers and hex colours in `dir="ltr"`.
 */

const KIB = 1024;

/** `20_078_886_912` → `18.7 GB`. Binary units, since that is what the memory
 *  and disk figures are counted in. */
export function bytes(value: number): string {
  const units = ["B", "KB", "MB", "GB", "TB"];
  let n = Math.max(0, value);
  let unit = 0;
  while (n >= KIB && unit < units.length - 1) {
    n /= KIB;
    unit += 1;
  }
  // Bytes and kilobytes never want a decimal — nobody reads "512.0 B" — but
  // from megabytes up the first decimal is most of the information: without it
  // a 1.9 MB/s transfer and a 1.1 MB/s one are both "1 MB/s".
  return `${n.toFixed(unit >= 2 ? 1 : 0)} ${units[unit]}`;
}

/** `1_258_291` → `1.2 MB/s`. */
export function rate(bytesPerSecond: number): string {
  return `${bytes(bytesPerSecond)}/s`;
}

/** `211_620` → `58:47:00`, counting hours rather than rolling over to days:
 *  an uptime of three days reads better as 71 hours on a HUD. */
export function duration(seconds: number): string {
  const whole = Math.max(0, Math.floor(seconds));
  const hours = Math.floor(whole / 3600);
  const minutes = Math.floor((whole % 3600) / 60);
  return `${hours}:${String(minutes).padStart(2, "0")}:${String(whole % 60).padStart(2, "0")}`;
}

/** A part of a whole as 0–100, with a zero whole reading as zero rather than
 *  as a division by nothing. */
export function percent(part: number, whole: number): number {
  if (whole <= 0) return 0;
  return Math.min(100, Math.max(0, (part / whole) * 100));
}

/** `21:47:03`, always 24-hour: a HUD has no room for am and pm. */
export function clockTime(now: Date): string {
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${pad(now.getHours())}:${pad(now.getMinutes())}:${pad(now.getSeconds())}`;
}
