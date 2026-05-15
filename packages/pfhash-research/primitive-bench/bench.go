// In-process Go benchmark for fogleman/primitive — the engine SQIP wraps.
//
// This represents the lower bound of a "pure Go service" running primitive:
// no Node.js, no SVGO post-processing, no subprocess spawn per request.
// Just the primitive library in a long-running Go process.
//
// Search-budget knobs are exposed (n, age, m) so we can match pfhash's
// budget for apples-to-apples comparison. `model.Step()` hardcodes
// n=1000, age=100, m=16; we replicate that loop manually using the
// public Worker API.
//
// Usage:
//   go run bench.go <mode> <n_shapes> <alpha> <input_size> <workers> \
//                   <n> <age> <m> <warmup> <path1> [path2 ...]
//
// Output (one line per image, after `warmup` runs are discarded):
//   <ms>\t<bytes>

package main

import (
	"fmt"
	"math/rand"
	"os"
	"runtime"
	"strconv"
	"time"

	"github.com/fogleman/primitive/primitive"
	"github.com/nfnt/resize"
)

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
	if len(os.Args) < 11 {
		fmt.Fprintln(os.Stderr,
			"Usage: bench <mode> <n_shapes> <alpha> <input_size> <workers> "+
				"<n> <age> <m> <warmup> <path1> [path2 ...]")
		os.Exit(2)
	}

	mode, _ := strconv.Atoi(os.Args[1])
	nShapes, _ := strconv.Atoi(os.Args[2])
	alpha, _ := strconv.Atoi(os.Args[3])
	inputSize, _ := strconv.Atoi(os.Args[4])
	workers, _ := strconv.Atoi(os.Args[5])
	nRandom, _ := strconv.Atoi(os.Args[6])
	maxAge, _ := strconv.Atoi(os.Args[7])
	mAttempts, _ := strconv.Atoi(os.Args[8])
	warmup, _ := strconv.Atoi(os.Args[9])
	paths := os.Args[10:]

	if workers < 1 {
		workers = runtime.NumCPU()
	}

	processOne := func(path string) (float64, int, error) {
		t0 := time.Now()

		input, err := primitive.LoadImage(path)
		if err != nil {
			return 0, 0, err
		}
		if inputSize > 0 {
			input = resize.Thumbnail(uint(inputSize), uint(inputSize),
				input, resize.Bilinear)
		}

		bg := primitive.MakeColor(primitive.AverageImageColor(input))

		model := primitive.NewModel(input, bg, 256, workers)

		// Deterministic seed so timings reflect identical work across runs.
		rand.Seed(1)
		for i := 0; i < nShapes; i++ {
			customStep(model, primitive.ShapeType(mode), alpha,
				nRandom, maxAge, mAttempts)
		}

		svg := model.SVG()
		ms := float64(time.Since(t0).Nanoseconds()) / 1e6
		return ms, len(svg), nil
	}

	for i := 0; i < warmup && i < len(paths); i++ {
		_, _, err := processOne(paths[i])
		if err != nil {
			fmt.Fprintf(os.Stderr, "warmup failed on %s: %v\n", paths[i], err)
			os.Exit(1)
		}
	}

	for _, p := range paths {
		ms, b, err := processOne(p)
		if err != nil {
			fmt.Fprintf(os.Stderr, "failed on %s: %v\n", p, err)
			os.Exit(1)
		}
		fmt.Printf("%.3f\t%d\n", ms, b)
	}
}
