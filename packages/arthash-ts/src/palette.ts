/**
 * Palette type + construction helpers.
 *
 * Lives in its own module so both `index.ts` (which re-exports it on the
 * public surface) and `palettes.ts` (which calls `palette.fromHex` at module
 * top level to build the curated presets) can import it without forming a
 * cycle. Putting `palette` in `index.ts` directly created a circular import:
 * `index.ts → palettes.ts → index.ts`, where `palettes.ts`'s top-level
 * `palette.fromHex(...)` calls executed before `index.ts` had reached its
 * `export const palette = {...}` line, throwing TDZ.
 */

/** sRGB palette — flat row-major bytes (length = 3·K, K ∈ [2, 1024]). K need
 *  not be a power of 2; the codec uses `ceil(log₂K)` bits per index. */
export interface Palette {
  bytes: Uint8Array;
  k?: number;
}

const MIN_PALETTE_K = 2;
const MAX_PALETTE_K = 1024;

/** Palette construction helpers. */
export const palette = {
  /** Build a palette from an array of `[r, g, b]` triplets. */
  fromRgb(colors: ReadonlyArray<readonly [number, number, number]>): Palette {
    if (colors.length < MIN_PALETTE_K || colors.length > MAX_PALETTE_K) {
      throw new Error(
        `palette K must be in [${MIN_PALETTE_K}, ${MAX_PALETTE_K}]; got ${colors.length}`,
      );
    }
    const bytes = new Uint8Array(colors.length * 3);
    for (let i = 0; i < colors.length; i++) {
      const c = colors[i]!;
      bytes[i * 3] = c[0];
      bytes[i * 3 + 1] = c[1];
      bytes[i * 3 + 2] = c[2];
    }
    return { bytes };
  },
  /** Build a palette from hex color strings (`"#aabbcc"` or `"aabbcc"`). */
  fromHex(hexes: ReadonlyArray<string>): Palette {
    const rgb: Array<readonly [number, number, number]> = hexes.map((h) => {
      const s = h.startsWith("#") ? h.slice(1) : h;
      if (s.length !== 6) {
        throw new Error(`palette: expected 6-char hex, got ${h!}`);
      }
      return [
        parseInt(s.slice(0, 2), 16),
        parseInt(s.slice(2, 4), 16),
        parseInt(s.slice(4, 6), 16),
      ] as const;
    });
    return palette.fromRgb(rgb);
  },
} as const;
