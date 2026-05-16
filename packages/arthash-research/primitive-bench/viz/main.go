// Visual-output companion to bench.go — same algorithm, but saves a
// rendered PNG instead of printing timing. Used for side-by-side quality
// comparison against arthash output.
//
// Usage:
//   go run ./viz <input> <output_png> <mode> <n_shapes> <n> <age> <m>
//
// Workers default to NumCPU. Output is rendered at 256px long-edge to
// match arthash's default decode size.

package main

import (
	"fmt"
	"math/rand"
	"os"
	"runtime"
	"strconv"

	"github.com/fogleman/primitive/primitive"
	"github.com/nfnt/resize"
)

const outputSize = 256

func customStep(model *primitive.Model, t primitive.ShapeType, alpha int,
	n, age, m int) {
	wn := len(model.Workers)
	ch := make(chan *primitive.State, wn)
	wm := m / wn
	if m%wn != 0 {
		wm++
	}
	for i := 0; i < wn; i++ {
		worker := model.Workers[i]
		worker.Init(model.Current, model.Score)
		go func(w *primitive.Worker) {
			ch <- w.BestHillClimbState(t, alpha, n, age, wm)
		}(worker)
	}
	var bestEnergy float64
	var bestState *primitive.State
	for i := 0; i < wn; i++ {
		state := <-ch
		e := state.Energy()
		if i == 0 || e < bestEnergy {
			bestEnergy = e
			bestState = state
		}
	}
	model.Add(bestState.Shape, bestState.Alpha)
}

func main() {
	if len(os.Args) < 8 {
		fmt.Fprintln(os.Stderr,
			"Usage: viz <input> <output_png> <mode> <n_shapes> <n> <age> <m>")
		os.Exit(2)
	}
	inPath := os.Args[1]
	outPath := os.Args[2]
	mode, _ := strconv.Atoi(os.Args[3])
	nShapes, _ := strconv.Atoi(os.Args[4])
	nRandom, _ := strconv.Atoi(os.Args[5])
	maxAge, _ := strconv.Atoi(os.Args[6])
	mAttempts, _ := strconv.Atoi(os.Args[7])

	input, err := primitive.LoadImage(inPath)
	if err != nil {
		fmt.Fprintf(os.Stderr, "load: %v\n", err)
		os.Exit(1)
	}
	input = resize.Thumbnail(256, 256, input, resize.Bilinear)

	bg := primitive.MakeColor(primitive.AverageImageColor(input))
	model := primitive.NewModel(input, bg, outputSize, runtime.NumCPU())

	rand.Seed(1)
	for i := 0; i < nShapes; i++ {
		customStep(model, primitive.ShapeType(mode), 128,
			nRandom, maxAge, mAttempts)
	}

	if err := primitive.SavePNG(outPath, model.Context.Image()); err != nil {
		fmt.Fprintf(os.Stderr, "save: %v\n", err)
		os.Exit(1)
	}
	// Print SVG byte count to stdout for the caller's labeling. This is
	// the actual wire-format size; the PNG we just saved is a render only.
	svg := model.SVG()
	fmt.Printf("%d\n", len(svg))
}
