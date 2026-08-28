/**
 * Generates the placeholder app icon.
 *
 * `tauri-build` needs `src-tauri/icons/icon.ico` to exist before it will build
 * at all, so this makes one from nothing — no image editor, no dependencies,
 * just the ICO and PNG container formats written by hand.
 *
 * Replace it with the real thing when there is artwork:
 *     pnpm tauri icon path\to\logo.png
 *
 * Run: node tools/make-placeholder-icons.mjs
 */

import { deflateSync, crc32 } from "node:zlib";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const outDir = join(root, "src-tauri", "icons");

// The same two blues as the brand mark in the UI.
const TOP = [0x2b, 0x86, 0xdc];
const BOTTOM = [0x0a, 0x3f, 0x82];
const GLYPH = [0xff, 0xff, 0xff];

/** Straight-alpha RGBA pixels, top-down. */
function render(size) {
  const px = new Uint8Array(size * size * 4);
  const radius = size * 0.22;

  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      const i = (y * size + x) * 4;
      const t = y / (size - 1);

      // Rounded-square coverage, sampled at the pixel centre.
      const cx = Math.min(x + 0.5, size - x - 0.5);
      const cy = Math.min(y + 0.5, size - y - 0.5);
      let inside = 1;
      if (cx < radius && cy < radius) {
        const d = Math.hypot(radius - cx, radius - cy);
        inside = Math.max(0, Math.min(1, radius - d + 0.5));
      }

      // The mark: a slash, the "restyle" gesture.
      const slash =
        Math.abs(x - (size - y) * 0.62 - size * 0.14) < size * 0.075 &&
        y > size * 0.24 &&
        y < size * 0.76
          ? 1
          : 0;

      for (let c = 0; c < 3; c++) {
        const bg = Math.round(TOP[c] + (BOTTOM[c] - TOP[c]) * t);
        px[i + c] = slash ? GLYPH[c] : bg;
      }
      px[i + 3] = Math.round(inside * 255);
    }
  }
  return px;
}

// ---------------------------------------------------------------- PNG

function chunk(type, body) {
  const head = Buffer.alloc(8);
  head.writeUInt32BE(body.length, 0);
  head.write(type, 4, "ascii");
  const crcInput = Buffer.concat([head.subarray(4, 8), body]);
  const tail = Buffer.alloc(4);
  tail.writeUInt32BE(crc32(crcInput) >>> 0, 0);
  return Buffer.concat([head, body, tail]);
}

function png(size) {
  const px = render(size);
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(size, 0);
  ihdr.writeUInt32BE(size, 4);
  ihdr[8] = 8; // bit depth
  ihdr[9] = 6; // colour type: RGBA
  // 10, 11, 12 stay 0: deflate, adaptive filtering, no interlace.

  // One filter byte (0 = None) in front of every scanline.
  const raw = Buffer.alloc(size * (size * 4 + 1));
  for (let y = 0; y < size; y++) {
    const from = y * size * 4;
    raw[y * (size * 4 + 1)] = 0;
    Buffer.from(px.subarray(from, from + size * 4)).copy(
      raw,
      y * (size * 4 + 1) + 1,
    );
  }

  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk("IHDR", ihdr),
    chunk("IDAT", deflateSync(raw, { level: 9 })),
    chunk("IEND", Buffer.alloc(0)),
  ]);
}

// ---------------------------------------------------------------- ICO

/** A 32-bit BGRA DIB, bottom-up, with the (unused but required) AND mask. */
function dib(size) {
  const px = render(size);
  const header = Buffer.alloc(40);
  header.writeUInt32LE(40, 0);
  header.writeInt32LE(size, 4);
  header.writeInt32LE(size * 2, 8); // XOR + AND, as the format demands
  header.writeUInt16LE(1, 12); // planes
  header.writeUInt16LE(32, 14); // bits per pixel

  const xor = Buffer.alloc(size * size * 4);
  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      const from = ((size - 1 - y) * size + x) * 4; // flip vertically
      const to = (y * size + x) * 4;
      xor[to] = px[from + 2]; // B
      xor[to + 1] = px[from + 1]; // G
      xor[to + 2] = px[from]; // R
      xor[to + 3] = px[from + 3]; // A
    }
  }

  const maskRow = Math.ceil(size / 32) * 4;
  const and = Buffer.alloc(maskRow * size); // all zero: nothing masked out

  header.writeUInt32LE(xor.length + and.length, 20);
  return Buffer.concat([header, xor, and]);
}

function ico(sizes) {
  const images = sizes.map(dib);
  const dir = Buffer.alloc(6);
  dir.writeUInt16LE(0, 0);
  dir.writeUInt16LE(1, 2); // 1 = icon
  dir.writeUInt16LE(sizes.length, 4);

  let offset = 6 + sizes.length * 16;
  const entries = sizes.map((size, i) => {
    const e = Buffer.alloc(16);
    e[0] = size >= 256 ? 0 : size; // 0 means 256
    e[1] = size >= 256 ? 0 : size;
    e.writeUInt16LE(1, 4); // planes
    e.writeUInt16LE(32, 6); // bpp
    e.writeUInt32LE(images[i].length, 8);
    e.writeUInt32LE(offset, 12);
    offset += images[i].length;
    return e;
  });

  return Buffer.concat([dir, ...entries, ...images]);
}

mkdirSync(outDir, { recursive: true });

const written = [];
for (const size of [32, 128, 256]) {
  const name = size === 256 ? "128x128@2x.png" : `${size}x${size}.png`;
  writeFileSync(join(outDir, name), png(size));
  written.push(name);
}
writeFileSync(join(outDir, "icon.png"), png(512));
written.push("icon.png");
writeFileSync(join(outDir, "icon.ico"), ico([16, 32, 48, 64, 128, 256]));
written.push("icon.ico");

console.log(`wrote ${written.join(", ")} to src-tauri/icons`);
