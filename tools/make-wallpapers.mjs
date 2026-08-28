/**
 * Generates the wallpapers that ship with the built-in Looks.
 *
 * Apple's wallpapers are Apple's, so nothing here is copied from macOS — these
 * are original abstract gradients drawn in the same register: deep, quiet, and
 * out of the way of whatever is on top of them.
 *
 * Run: node tools/make-wallpapers.mjs
 */

import { deflateSync, crc32 } from "node:zlib";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");

const WIDTH = 2560;
const HEIGHT = 1440;

/** Smoothstep, so the light falls off like light rather than like a cone. */
const smooth = (t) => {
  const x = Math.min(1, Math.max(0, t));
  return x * x * (3 - 2 * x);
};

const mix = (a, b, t) => a + (b - a) * t;

/**
 * A soft radial glow, added to the base gradient.
 * Coordinates are fractions of the canvas so the shape survives a resize.
 */
function glow(x, y, cx, cy, radius, aspect) {
  const dx = (x - cx) * aspect;
  const dy = y - cy;
  return smooth(1 - Math.hypot(dx, dy) / radius);
}

function render({ top, bottom, glows }) {
  const px = Buffer.alloc(WIDTH * HEIGHT * 3);
  const aspect = WIDTH / HEIGHT;

  for (let iy = 0; iy < HEIGHT; iy++) {
    const y = iy / (HEIGHT - 1);
    // Slight ease on the vertical ramp: a linear one reads as a flat wash.
    const t = smooth(y * 0.85 + 0.075);

    for (let ix = 0; ix < WIDTH; ix++) {
      const x = ix / (WIDTH - 1);
      let r = mix(top[0], bottom[0], t);
      let g = mix(top[1], bottom[1], t);
      let b = mix(top[2], bottom[2], t);

      for (const spot of glows) {
        const amount = glow(x, y, spot.x, spot.y, spot.r, aspect) * spot.strength;
        if (amount <= 0) continue;
        r += (spot.color[0] - r) * amount;
        g += (spot.color[1] - g) * amount;
        b += (spot.color[2] - b) * amount;
      }

      // Ordered dither in the low bit only. Without it, a gradient this smooth
      // bands visibly on an 8-bit display; with it, the banding disappears and
      // the file still compresses.
      const d = ((ix + iy * 3) % 4) / 4 - 0.375;

      const i = (iy * WIDTH + ix) * 3;
      px[i] = Math.max(0, Math.min(255, Math.round(r + d)));
      px[i + 1] = Math.max(0, Math.min(255, Math.round(g + d)));
      px[i + 2] = Math.max(0, Math.min(255, Math.round(b + d)));
    }
  }
  return px;
}

function chunk(type, body) {
  const head = Buffer.alloc(8);
  head.writeUInt32BE(body.length, 0);
  head.write(type, 4, "ascii");
  const tail = Buffer.alloc(4);
  tail.writeUInt32BE(crc32(Buffer.concat([head.subarray(4, 8), body])) >>> 0, 0);
  return Buffer.concat([head, body, tail]);
}

function png(px) {
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(WIDTH, 0);
  ihdr.writeUInt32BE(HEIGHT, 4);
  ihdr[8] = 8; // bit depth
  ihdr[9] = 2; // colour type: RGB, no alpha a wallpaper would never use

  const stride = WIDTH * 3;
  const raw = Buffer.alloc(HEIGHT * (stride + 1));
  for (let y = 0; y < HEIGHT; y++) {
    // Filter 2 (Up): a vertical gradient is almost identical row to row, so
    // this leaves the deflater with near-zero deltas to store.
    raw[y * (stride + 1)] = 2;
    for (let x = 0; x < stride; x++) {
      const here = px[y * stride + x];
      const above = y === 0 ? 0 : px[(y - 1) * stride + x];
      raw[y * (stride + 1) + 1 + x] = (here - above) & 0xff;
    }
  }

  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk("IHDR", ihdr),
    chunk("IDAT", deflateSync(raw, { level: 9 })),
    chunk("IEND", Buffer.alloc(0)),
  ]);
}

const wallpapers = [
  {
    out: join(root, "packs", "macos", "assets", "wallpaper.png"),
    top: [14, 18, 46],
    bottom: [72, 34, 92],
    glows: [
      { x: 0.28, y: 0.72, r: 0.62, strength: 0.55, color: [96, 62, 160] },
      { x: 0.74, y: 0.32, r: 0.5, strength: 0.42, color: [42, 96, 178] },
      { x: 0.5, y: 0.95, r: 0.38, strength: 0.3, color: [188, 96, 140] },
    ],
  },
];

for (const spec of wallpapers) {
  mkdirSync(dirname(spec.out), { recursive: true });
  const file = png(render(spec));
  writeFileSync(spec.out, file);
  console.log(
    `${spec.out.replace(root + "\\", "")}  ${WIDTH}x${HEIGHT}  ${(file.length / 1024 / 1024).toFixed(2)} MB`,
  );
}
