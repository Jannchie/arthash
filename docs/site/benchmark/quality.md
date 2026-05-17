# Visual quality

PSNR vs the original image, computed at 256 long-edge output, averaged across
the standard test corpus. Higher is better; +3 dB ≈ doubled signal-to-noise.

## Quality table (sorted by PSNR desc)

| Output                   |   bytes |    PSNR |
| ------------------------ | ------: | ------: |
| sqip · 12 primitives SVG | ~1100 B | 24.4 dB |
| arthash · DCT            |    17 B | 23.3 dB |
| thumbhash                |    17 B | 22.9 dB |
| arthash · TRIANGLE 12    |    77 B | 21.4 dB |
| arthash · CIRCLE 12      |    53 B | 20.7 dB |

## Takeaways

- **At the 17 B budget**, arthash DCT beats thumbhash by **+0.4 dB PSNR** — for
  free, no encoder-tuning required.
- **arthash TRIANGLE 12** lands at 21.4 dB / 77 B. sqip's 24.4 dB output is
  better quality, but at **14× the byte cost** (~1100 B). That's a 3 dB quality
  drop in exchange for a 1/14 size reduction — usually the right trade for a
  placeholder.
- **arthash CIRCLE 12** is the smallest shape-mode output (53 B) and still
  hits 20.7 dB — competitive with thumbhash's blurred output at 3× the byte
  budget, but with sharp SVG primitives instead of a blurry raster.

## When PSNR matters and when it doesn't

PSNR is a useful objective metric, but placeholders are a perceptual problem.
The actual ranking on "does this look like the original image at first glance"
is roughly:

1. **arthash TRIANGLE 24+** — recognisable shapes, real colour fidelity.
2. **arthash DCT** — strong colour and structure, smooth blur.
3. **thumbhash** — same idea as arthash DCT, slightly weaker quantisation.
4. **arthash CIRCLE 12** — distinctive style, lower fidelity.
5. **sqip primitives** — high PSNR but large; competitive only when bytes are
   abundant.

For brand-consistent placeholders, palette modes can score lower in raw PSNR
but score higher subjectively because the colour distribution matches your
site's design system.

## Methodology

- **Corpus**: 24 reference images (portraits, landscapes, product shots, UI
  screenshots). Same set as the [JS cross-impl bench](https://github.com/Jannchie/arthash/tree/main/bench/js-cross).
- **Decode size**: long edge = 256 px. Hashes that decode at a smaller native
  size (notably thumbhash's ~32 px default) are bilinearly upsampled to 256 px
  before PSNR.
- **Colour space**: PSNR computed on sRGB after gamma decode. Switching to
  linear-light or Oklab moves all numbers up by ~0.5 dB but doesn't change the
  ranking.
