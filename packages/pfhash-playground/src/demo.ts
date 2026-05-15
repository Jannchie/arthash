// Demo dataset for the playground waterfall.
// Files live in `public/demo/NN.jpg` (see scripts/download workflow) so the
// site works offline. Each entry records the file's natural pixel dimensions
// — the waterfall uses them to lay out the tile before the image loads.

export interface DemoImage {
  src: string;
  w: number;
  h: number;
  alt: string;
}

const RAW: Array<{ idx: string; w: number; h: number; alt: string }> = [
  { idx: "01", w: 800, h: 1000, alt: "aurora" },
  { idx: "02", w: 1200, h: 800, alt: "harbor" },
  { idx: "03", w: 800, h: 800, alt: "grove" },
  { idx: "04", w: 1200, h: 800, alt: "dune" },
  { idx: "05", w: 700, h: 1050, alt: "prism" },
  { idx: "06", w: 1000, h: 750, alt: "forge" },
  { idx: "07", w: 720, h: 1280, alt: "river" },
  { idx: "08", w: 1200, h: 900, alt: "meadow" },
  { idx: "09", w: 800, h: 1200, alt: "glass" },
  { idx: "10", w: 1100, h: 700, alt: "ember" },
  { idx: "11", w: 900, h: 1350, alt: "tide" },
  { idx: "12", w: 1000, h: 1000, alt: "loft" },
  { idx: "13", w: 1200, h: 800,  alt: "canyon" },
  { idx: "14", w: 800,  h: 1200, alt: "spire" },
  { idx: "15", w: 1000, h: 1000, alt: "quarry" },
  { idx: "16", w: 1400, h: 700,  alt: "horizon" },
  { idx: "17", w: 700,  h: 1400, alt: "cascade" },
  { idx: "18", w: 1200, h: 900,  alt: "atoll" },
  { idx: "19", w: 900,  h: 1200, alt: "cinder" },
  { idx: "20", w: 800,  h: 800,  alt: "mirror" },
  { idx: "21", w: 1100, h: 800,  alt: "reef" },
  { idx: "22", w: 800,  h: 1100, alt: "fern" },
  { idx: "23", w: 1200, h: 800,  alt: "basin" },
  { idx: "24", w: 900,  h: 900,  alt: "cobble" },
  { idx: "25", w: 1300, h: 700,  alt: "plume" },
  { idx: "26", w: 700,  h: 1300, alt: "oasis" },
  { idx: "27", w: 1000, h: 1500, alt: "vista" },
  { idx: "28", w: 1500, h: 1000, alt: "chasm" },
  { idx: "29", w: 800,  h: 800,  alt: "lichen" },
  { idx: "30", w: 1200, h: 800,  alt: "pebble" },
  { idx: "31", w: 800,  h: 1200, alt: "mesa" },
  { idx: "32", w: 1000, h: 1000, alt: "thicket" },
  { idx: "33", w: 1100, h: 700,  alt: "hollow" },
  { idx: "34", w: 700,  h: 1100, alt: "brine" },
  { idx: "35", w: 1200, h: 900,  alt: "ridge" },
  { idx: "36", w: 900,  h: 1350, alt: "marsh" },
];

export const DEMO_IMAGES: DemoImage[] = RAW.map(({ idx, w, h, alt }) => ({
  src: `${import.meta.env.BASE_URL}demo/${idx}.jpg`,
  w,
  h,
  alt,
}));
