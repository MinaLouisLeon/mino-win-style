/**
 * The voice and the noises.
 *
 * Two decisions worth knowing before reading the code.
 *
 * **It lives in the settings window, not the HUD.** The HUD is click-through
 * and never takes focus, so nothing in it can ever be a user gesture — and
 * without a user gesture Chromium (and so WebView2) refuses to start an
 * `AudioContext` or speak. The click that turns JARVIS mode on happens *here*,
 * which is the only place in the app that has the activation. The HUD is
 * silent and visual; the two stay in step because Rust's `jarvis-mode` event
 * starts both at once.
 *
 * **Nothing is shipped.** Every sound is an oscillator drawn on the spot, and
 * the voice is whichever SAPI voice Windows already has, reached through the
 * Web Speech API. No audio files, no licences, no network, and it works with
 * the machine offline.
 */

let context: AudioContext | null = null;
let enabled = false;

/** Quiet enough to live behind, since these fire on hover. */
const VOLUME = 0.05;

export function setSoundEnabled(on: boolean): void {
  enabled = on;
  if (!on) {
    try {
      window.speechSynthesis?.cancel();
    } catch {
      // A webview without speech support has nothing to cancel.
    }
  }
}

export function isSoundEnabled(): boolean {
  return enabled;
}

/**
 * The shared context, made on first use.
 *
 * It can come back suspended — a page that has not been clicked yet gets one
 * that way — so every call asks it to resume. That is a no-op once it is
 * running, and the promise is deliberately dropped: a sound that cannot play is
 * not worth an error path.
 */
function audio(): AudioContext | null {
  if (!enabled) return null;
  try {
    const Ctor = window.AudioContext ?? (window as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
    if (!Ctor) return null;
    context ??= new Ctor();
    if (context.state === "suspended") void context.resume();
    return context;
  } catch {
    return null;
  }
}

/**
 * One tone.
 *
 * The envelope matters more than the pitch: a gain that jumps straight to full
 * volume clicks, which on a hover sound is the difference between an interface
 * that hums and one that crackles. So it ramps up over 8ms and decays
 * exponentially, and the exponential never quite reaches zero, which is why it
 * finishes with a linear ramp to silence.
 */
function tone(
  from: number,
  to: number,
  ms: number,
  type: OscillatorType = "sine",
  gain = VOLUME,
): void {
  const ctx = audio();
  if (!ctx) return;

  const now = ctx.currentTime;
  const seconds = ms / 1000;

  const osc = ctx.createOscillator();
  osc.type = type;
  osc.frequency.setValueAtTime(from, now);
  if (to !== from) osc.frequency.exponentialRampToValueAtTime(Math.max(1, to), now + seconds);

  const amp = ctx.createGain();
  amp.gain.setValueAtTime(0.0001, now);
  amp.gain.exponentialRampToValueAtTime(gain, now + 0.008);
  amp.gain.exponentialRampToValueAtTime(gain * 0.01, now + seconds);
  amp.gain.linearRampToValueAtTime(0, now + seconds + 0.02);

  osc.connect(amp).connect(ctx.destination);
  osc.start(now);
  osc.stop(now + seconds + 0.03);
}

/** Hovering something. Barely there on purpose. */
export function hover(): void {
  tone(1180, 1180, 45, "sine", VOLUME * 0.5);
}

/** Pressing something. */
export function click(): void {
  tone(1560, 980, 90, "triangle");
}

/** A switch going on: two notes up. */
export function on(): void {
  tone(740, 740, 70, "triangle");
  window.setTimeout(() => tone(1110, 1110, 110, "triangle"), 70);
}

/** A switch going off: the same two, reversed. */
export function off(): void {
  tone(1110, 1110, 70, "triangle");
  window.setTimeout(() => tone(740, 740, 110, "triangle"), 70);
}

/** Boot: a rising sweep with a fifth above it, so it reads as a chord and not
 *  as a slide whistle. */
export function bootSweep(): void {
  tone(180, 1400, 900, "sawtooth", VOLUME * 0.8);
  window.setTimeout(() => tone(270, 2100, 700, "sine", VOLUME * 0.4), 120);
}

/** Shutdown: the same shape, falling, and slower. */
export function powerDown(): void {
  tone(1200, 160, 1000, "sawtooth", VOLUME * 0.8);
  window.setTimeout(() => tone(1800, 240, 800, "sine", VOLUME * 0.4), 100);
}

/**
 * Says something, in the voice Windows already has.
 *
 * `lang` picks the voice: Arabic if one is installed, and otherwise whatever is
 * default — a wrong-accented reading is better than silence with no explanation.
 * The rate is a little under normal because the stock SAPI voices run fast and
 * the line is meant to land, not to be got through.
 */
export function speak(text: string, lang: "en" | "ar"): void {
  if (!enabled || !text) return;
  try {
    const synth = window.speechSynthesis;
    if (!synth) return;

    const line = new SpeechSynthesisUtterance(text);
    line.lang = lang === "ar" ? "ar-SA" : "en-GB";
    line.rate = 0.92;
    line.pitch = 0.9;

    const match = synth.getVoices().find((voice) => voice.lang.startsWith(lang));
    if (match) line.voice = match;

    // Anything still being said is stale by definition: the greeting and the
    // sign-off must never overlap.
    synth.cancel();
    synth.speak(line);
  } catch {
    // No speech engine, or one that refused. Not worth surfacing.
  }
}

/**
 * The greeting, by the hour on the clock.
 *
 * Returned rather than spoken so the HUD can show the same words it hears, and
 * so the translation stays in the caller where the dictionary is.
 */
export function greetingKey(now = new Date()): "morning" | "afternoon" | "evening" {
  const hour = now.getHours();
  if (hour < 12) return "morning";
  if (hour < 18) return "afternoon";
  return "evening";
}
