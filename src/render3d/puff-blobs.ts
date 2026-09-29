const PUFF_CELL_SIZE = 128;
const PUFF_CELLS = 4;
const PUFF_BLOB_COUNT = 34;

function createSeededRandom(seed: number): () => number {
  let state = seed >>> 0;
  return () => {
    state = (Math.imul(state, 1664525) + 1013904223) >>> 0;
    return state / 4294967296;
  };
}

/**
 * The soft, lumpy smoke puffs of the former 2x2 canvas atlas, as seeded blob parameters
 * (4 floats per blob: cell-local x, y in canvas pixels, radius, alpha; cells in reading
 * order). The shaders evaluate them per pixel (`puffCoverage`) instead of sampling a
 * texture: smoke sprites use a cell each, and scorch decals reuse cell 2 as a noisy mask.
 */
export function createPuffBlobs(): number[] {
  const random = createSeededRandom(0x5eed);
  const center = PUFF_CELL_SIZE / 2;
  const blobs: number[] = [];
  for (let cell = 0; cell < PUFF_CELLS; cell += 1) {
    for (let blob = 0; blob < PUFF_BLOB_COUNT; blob += 1) {
      const angle = random() * Math.PI * 2;
      const distance = Math.pow(random(), 0.7) * PUFF_CELL_SIZE * 0.26;
      const radius = PUFF_CELL_SIZE * (0.08 + (random() * 0.17));
      const alpha = 0.16 + (random() * 0.22);
      blobs.push(center + (Math.cos(angle) * distance), center + (Math.sin(angle) * distance), radius, alpha);
    }
  }
  return blobs;
}
