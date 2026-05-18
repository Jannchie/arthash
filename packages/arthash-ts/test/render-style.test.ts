// Tests for the new RenderStyle surface introduced in 0.3.0.
//
// These tests deliberately avoid wasm init (the published bindings are
// `--target web` and require fetch + browser globals to load). They cover:
//  * conditional type behavior via `@ts-expect-error` directives — vitest's
//    type-check pass (`vitest --typecheck`) validates these.
//  * Node-environment helpers throw a friendly error (no wasm needed for the
//    pre-check path).
//  * `Preset` surface still exposes the deprecated aliases.

import { describe, expect, it } from "vitest";
import {
  codec,
  decode,
  Preset,
  toImageBitmap,
  toImageData,
  toSvg,
  type RenderStyle,
} from "../src/index.js";

describe("Preset aliases (0.3 deprecation window)", () => {
  it("exposes new names", () => {
    expect(Preset.SmallTriangle).toBe("small_triangle");
    expect(Preset.LargeTriangle).toBe("large_triangle");
    expect(Preset.SmallRect).toBe("small_rect");
    expect(Preset.SmallSquare).toBe("small_square");
    expect(Preset.Dct).toBe("dct");
  });

  it("keeps pre-0.3 aliases for source compatibility", () => {
    expect(Preset.PlaceholderTriangle).toBe("placeholder_triangle");
    expect(Preset.DetailTriangle).toBe("detail_triangle");
    expect(Preset.TinyDct).toBe("tiny_dct");
  });
});

describe("Raster helpers in Node environment", () => {
  it("toImageData throws a friendly error in Node", async () => {
    const hash = new Uint8Array([0]);
    const c = codec.triangle({ n: 12 });
    // ImageData is undefined in plain Node; the helper must error early.
    if (typeof ImageData !== "undefined") {
      // Browser-like vitest env — skip the negative test.
      return;
    }
    await expect(toImageData(hash, c)).rejects.toThrow(/browser-only/);
  });

  it("toImageBitmap throws a friendly error in Node", async () => {
    const hash = new Uint8Array([0]);
    const c = codec.triangle({ n: 12 });
    if (typeof createImageBitmap !== "undefined") {
      return;
    }
    await expect(toImageBitmap(hash, c)).rejects.toThrow(/browser-only/);
  });
});

describe("RenderStyle conditional type", () => {
  it("allows blur on any codec", () => {
    const _dct: RenderStyle<ReturnType<typeof codec.dct>> = { blur: 4 };
    const _circle: RenderStyle<ReturnType<typeof codec.circle>> = { blur: 4 };
    const _rect: RenderStyle<ReturnType<typeof codec.rect>> = { blur: 4 };
    expect(_rect.blur).toBe(4);
  });

  it("allows cornerRadius on rect / square / rotrect", () => {
    const _rect: RenderStyle<ReturnType<typeof codec.rect>> = {
      cornerRadius: 1,
    };
    const _square: RenderStyle<ReturnType<typeof codec.square>> = {
      cornerRadius: 1,
    };
    const _rotrect: RenderStyle<ReturnType<typeof codec.rotatedRect>> = {
      cornerRadius: 1,
    };
    expect(_rect.cornerRadius).toBe(1);
    expect(_square.cornerRadius).toBe(1);
    expect(_rotrect.cornerRadius).toBe(1);
  });

  it("toSvg / decode call-site enforces conditional", () => {
    // Type-only assertions — these are compile-time checks; the function
    // calls are wrapped in a never-run branch so wasm doesn't get invoked.
    const hash = new Uint8Array([0]);
    if (false as unknown as boolean) {
      // OK — rect with cornerRadius.
      void toSvg(hash, codec.rect({ n: 32 }), {
        style: { blur: 2, cornerRadius: 1 },
      });
      // OK — circle with blur only.
      void toSvg(hash, codec.circle({ n: 24 }), {
        style: { blur: 4 },
      });
      // OK — decode with rect+corner.
      void decode(hash, codec.square({ n: 12 }), {
        style: { cornerRadius: 3 },
      });
      // FAIL — circle codec does not accept cornerRadius.
      void toSvg(hash, codec.circle({ n: 24 }), {
        // @ts-expect-error cornerRadius not allowed on circle codec
        style: { cornerRadius: 1 },
      });
      // FAIL — triangle codec does not accept cornerRadius via decode.
      void decode(hash, codec.triangle({ n: 12 }), {
        // @ts-expect-error cornerRadius not allowed on triangle codec
        style: { cornerRadius: 5 },
      });
      // FAIL — pixel codec does not accept cornerRadius.
      void toSvg(hash, codec.pixel({ n: 16 }), {
        // @ts-expect-error cornerRadius not allowed on pixel codec
        style: { cornerRadius: 2 },
      });
    }
    expect(true).toBe(true);
  });

  it("rejects cornerRadius on circle / triangle / pixel / dct", () => {
    const _circle: RenderStyle<ReturnType<typeof codec.circle>> = {
      // @ts-expect-error — cornerRadius not allowed on circle.
      cornerRadius: 1,
    };
    const _tri: RenderStyle<ReturnType<typeof codec.triangle>> = {
      // @ts-expect-error — cornerRadius not allowed on triangle.
      cornerRadius: 1,
    };
    const _pixel: RenderStyle<ReturnType<typeof codec.pixel>> = {
      // @ts-expect-error — cornerRadius not allowed on pixel.
      cornerRadius: 1,
    };
    const _dct: RenderStyle<ReturnType<typeof codec.dct>> = {
      // @ts-expect-error — cornerRadius not allowed on dct.
      cornerRadius: 1,
    };
    // Variables referenced once so noUnused doesn't trip.
    void _circle;
    void _tri;
    void _pixel;
    void _dct;
    expect(true).toBe(true);
  });
});
