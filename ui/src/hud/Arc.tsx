/**
 * The arc reactor: concentric rings that turn.
 *
 * Drawn as SVG rather than as a stack of bordered divs because the only thing
 * that actually moves is a `transform: rotate`, which the compositor handles
 * without laying anything out again. A ring built from `border-radius` and
 * animated with `border-color` repaints its whole box every frame, and there
 * are six of them on screen at once.
 *
 * Nothing here is bound to real data on purpose. The reactor is the thing you
 * look at when you are not reading the numbers, and a ring whose speed tracked
 * the processor would make an idle machine look broken.
 */

interface Props {
  /** Diameter in pixels; every proportion below is a fraction of it. */
  size: number;
  /** Multiplies every rotation. The boot sequence spins it up from a stop. */
  speed?: number;
}

interface Ring {
  /** Radius as a fraction of the diameter, so 0.5 touches the edge. */
  radius: number;
  /** Stroke width, likewise a fraction of the diameter. */
  width: number;
  /** How many dashes around the ring. Zero draws it solid. */
  segments: number;
  /** How much of each segment is ink rather than gap, 0 to 1. */
  duty: number;
  /** Seconds for one full turn. Zero holds it still. */
  seconds: number;
  reverse?: boolean;
  opacity: number;
}

const RINGS: Ring[] = [
  { radius: 0.46, width: 0.006, segments: 0, duty: 1, seconds: 0, opacity: 0.28 },
  { radius: 0.42, width: 0.01, segments: 3, duty: 0.8, seconds: 34, opacity: 0.75 },
  { radius: 0.35, width: 0.004, segments: 48, duty: 0.25, seconds: 22, reverse: true, opacity: 0.5 },
  { radius: 0.29, width: 0.016, segments: 4, duty: 0.68, seconds: 17, opacity: 0.9 },
  { radius: 0.22, width: 0.005, segments: 0, duty: 1, seconds: 0, opacity: 0.35 },
  { radius: 0.16, width: 0.022, segments: 6, duty: 0.6, seconds: 11, reverse: true, opacity: 1 },
];

/** Twelve marks around the outside, like a bezel. */
const TICKS = Array.from({ length: 12 }, (_, i) => i * 30);

/** A dash pattern that closes exactly, whatever the radius. */
function dashesFor(ring: Ring, radiusPx: number): string | undefined {
  if (ring.segments === 0) return undefined;
  const segment = (2 * Math.PI * radiusPx) / ring.segments;
  return `${segment * ring.duty} ${segment * (1 - ring.duty)}`;
}

export function Arc({ size, speed = 1 }: Props) {
  const centre = size / 2;
  const px = (fraction: number) => fraction * size;

  return (
    <svg
      className="arc"
      width={size}
      height={size}
      viewBox={`0 0 ${size} ${size}`}
      aria-hidden="true"
    >
      <defs>
        {/* The glow at the centre. A radial gradient rather than a blur filter:
            filters are re-rasterised as the rings turn over them, and this is
            the largest thing on screen. */}
        <radialGradient id="arc-core">
          <stop offset="0%" stopColor="var(--hud-hot)" stopOpacity="0.55" />
          <stop offset="45%" stopColor="var(--hud-line)" stopOpacity="0.16" />
          <stop offset="100%" stopColor="var(--hud-line)" stopOpacity="0" />
        </radialGradient>
      </defs>

      <circle cx={centre} cy={centre} r={px(0.34)} fill="url(#arc-core)" />

      {RINGS.map((ring, index) => {
        const radius = px(ring.radius);
        return (
          <circle
            key={index}
            className="arc__ring"
            cx={centre}
            cy={centre}
            r={radius}
            fill="none"
            stroke="var(--hud-line)"
            strokeWidth={Math.max(1, px(ring.width))}
            strokeDasharray={dashesFor(ring, radius)}
            opacity={ring.opacity}
            style={
              ring.seconds === 0
                ? undefined
                : {
                    animation: `arc-spin ${
                      ring.seconds / Math.max(0.001, speed)
                    }s linear infinite${ring.reverse ? " reverse" : ""}`,
                  }
            }
          />
        );
      })}

      <g opacity="0.55">
        {TICKS.map((angle) => (
          <line
            key={angle}
            x1={centre}
            y1={centre - px(0.485)}
            x2={centre}
            y2={centre - px(0.455)}
            stroke="var(--hud-line)"
            strokeWidth={angle % 90 === 0 ? 2 : 1}
            opacity={angle % 90 === 0 ? 1 : 0.5}
            transform={`rotate(${angle} ${centre} ${centre})`}
          />
        ))}
      </g>

      {/* The bright core, pulsing. Slow enough not to read as a warning light. */}
      <circle className="arc__core" cx={centre} cy={centre} r={px(0.05)} fill="var(--hud-hot)" />
    </svg>
  );
}
