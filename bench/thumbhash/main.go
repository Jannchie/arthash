// Benchmark go.n16f.net/thumbhash (community Go port of evanw/thumbhash).
//
// Inputs and methodology mirror examples/bench.rs / scripts/bench_py.py:
//   100x100 synthetic gradient RGB → encode + decode at 256-px output.
//   Output is one NDJSON line per measurement on stdout.

package main

import (
	"encoding/json"
	"fmt"
	"image"
	"image/color"
	"math"
	"os"
	"sort"
	"time"

	"go.n16f.net/thumbhash"
)

func gradient(w, h int) *image.RGBA {
	img := image.NewRGBA(image.Rect(0, 0, w, h))
	for y := 0; y < h; y++ {
		for x := 0; x < w; x++ {
			r := uint8(math.Round(float64(x) * 255.0 / math.Max(float64(w-1), 1)))
			g := uint8(math.Round(float64(y) * 255.0 / math.Max(float64(h-1), 1)))
			b := uint8(math.Min(255.0, float64(x+y)*0.3))
			img.SetRGBA(x, y, color.RGBA{r, g, b, 255})
		}
	}
	return img
}

func measure(name, op string, w, h int, fn func(), warmup, iters int) {
	for i := 0; i < warmup; i++ {
		fn()
	}
	// Windows Go monotonic clock has ~µs-level jitter that swallows fast
	// individual calls into the 0-µs bucket. Batch timing amortizes one
	// measurement over `batch` calls so per-call resolution is recoverable.
	const batch = 50
	samples := make([]float64, iters)
	for i := 0; i < iters; i++ {
		t0 := time.Now()
		for j := 0; j < batch; j++ {
			fn()
		}
		samples[i] = float64(time.Since(t0).Nanoseconds()) / float64(batch) / 1000.0
	}
	sort.Float64s(samples)
	median := samples[len(samples)/2]
	p95 := samples[int(float64(len(samples))*0.95)]
	min := samples[0]
	mpix := float64(w*h) / median
	rec := map[string]interface{}{
		"impl":       "go",
		"mode":       name,
		"op":         op,
		"w":          w,
		"h":          h,
		"median_us":  math.Round(median*100) / 100,
		"p95_us":     math.Round(p95*100) / 100,
		"min_us":     math.Round(min*100) / 100,
		"iters":      iters,
		"mpix_per_s": math.Round(mpix*1000) / 1000,
	}
	b, err := json.Marshal(rec)
	if err != nil {
		fmt.Fprintln(os.Stderr, "marshal err:", err, "rec:", rec)
		return
	}
	fmt.Fprintln(os.Stdout, string(b))
	os.Stdout.Sync()
}

func main() {
	w, h := 100, 100
	img := gradient(w, h)

	var hash []byte
	measure("dct", "encode", w, h, func() {
		hash = thumbhash.EncodeImage(img)
	}, 30, 200)
	// Emit hash length as a separate stderr note so the NDJSON stays one
	// line per measurement.
	_ = hash // keep for decode below
	fmt.Fprintf(os.Stderr, "go thumbhash hash bytes: %d\n", len(hash))

	fmt.Fprintln(os.Stderr, "decode @ default baseSize=32 (matches thumbhash JS)...")
	measure("dct", "decode_default", w, h, func() {
		_, err := thumbhash.DecodeImage(hash)
		if err != nil {
			panic(err)
		}
	}, 10, 50)

	fmt.Fprintln(os.Stderr, "decode @ baseSize=256 (matches arthash decode)...")
	cfg := thumbhash.DecodingCfg{BaseSize: 256, SaturationBoost: 1.25}
	measure("dct", "decode_256", w, h, func() {
		_, err := thumbhash.DecodeImageWithCfg(hash, cfg)
		if err != nil {
			panic(err)
		}
	}, 10, 50)
}
