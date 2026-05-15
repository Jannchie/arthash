package main
import (
  "encoding/json"
  "fmt"
  "image"
  "image/color"
  "io"
  "os"
  "go.n16f.net/thumbhash"
)
type In struct { W, H int; RGBA []int }
type Out struct { W, H int; RGBA []int; HashBytes int }
func main(){
  data, _ := io.ReadAll(os.Stdin)
  var in In; json.Unmarshal(data, &in)
  img := image.NewRGBA(image.Rect(0,0,in.W,in.H))
  for y:=0;y<in.H;y++ { for x:=0;x<in.W;x++ {
    p := (y*in.W+x)*4
    img.SetRGBA(x,y,color.RGBA{uint8(in.RGBA[p]),uint8(in.RGBA[p+1]),uint8(in.RGBA[p+2]),uint8(in.RGBA[p+3])})
  }}
  h := thumbhash.EncodeImage(img)
  cfg := thumbhash.DecodingCfg{BaseSize: 256, SaturationBoost: 1.25}
  dec, err := thumbhash.DecodeImageWithCfg(h, cfg)
  if err != nil { fmt.Fprintln(os.Stderr, err); os.Exit(1) }
  b := dec.Bounds(); w, hh := b.Dx(), b.Dy()
  rgba := make([]int, w*hh*4)
  for y:=0;y<hh;y++ { for x:=0;x<w;x++ {
    r,g,bb,a := dec.At(x+b.Min.X,y+b.Min.Y).RGBA()
    p := (y*w+x)*4
    rgba[p]=int(r>>8); rgba[p+1]=int(g>>8); rgba[p+2]=int(bb>>8); rgba[p+3]=int(a>>8)
  }}
  json.NewEncoder(os.Stdout).Encode(Out{w,hh,rgba,len(h)})
}
