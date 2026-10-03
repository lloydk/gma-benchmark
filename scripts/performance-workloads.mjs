import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { buildWorkloads } from "../benchmark-workloads.js";
import { RGB_SPACES } from "../src/rgb-spaces.js";
import { createCanonicalReference } from "../tests/helpers/canonical-reference.js";
import { createBoundaryReference } from "../tests/helpers/matrix-reference.js";

export const methods = [
 ["clip", "clip"], ["css-minde", "css-minde"],
 ["oklch-cubic", "oklch-cubic (cached)"], ["oklch-cubic-no-cache", "oklch-cubic (no cache)"],
 ["oklch-cubic-direct", "oklch-cubic-direct"], ["oklch-halley", "oklch-halley"],
 ["oklch-ostrowski", "oklch-ostrowski"], ["dualray", "dualray"],
 ["dualray-fast", "dualray fast"], ["dualray-fast-poly", "dualray fast (poly encode)"],
 ["bottosson-lightness", "bottosson-lightness"], ["bottosson-lightness-cached", "bottosson-lightness (cached)"],
 ["edge-seeker", "edge-seeker"], ["edge-seeker-indexed", "edge-seeker (indexed)"], ["raytrace", "raytrace"],
];

export const runtimeMethods = Object.fromEntries(
 ["node", "bun", "rust-f64", "rust-f32"].map(runtime => [runtime,
  methods.map(([id]) => id).filter(id => runtime.startsWith("rust") || id !== "dualray-fast-poly")]),
);

export function buildPerformanceWorkloads() {
 const { samples, randomSamples } = buildWorkloads();
 const result = [];
 function add(gamut, name, inputs, checked = false) {
  const bytes = Buffer.alloc(inputs.length * 24);
  inputs.forEach((row, i) => row.forEach((x, j) => bytes.writeDoubleLE(x, i * 24 + j * 8)));
  const canonical = createCanonicalReference(RGB_SPACES[gamut]);
  const inside = inputs.reduce((n, input) => n + Number(canonical(input).inside), 0);
  result.push({ id: `${gamut}-${name}`, gamut, name, checked, count: inputs.length, inside,
   sha256: createHash("sha256").update(bytes).digest("hex"), bytes });
 }
 for (const gamut of ["display-p3", "srgb", "rec2020"]) {
  add(gamut, "grid", samples);
  add(gamut, "random", randomSamples);
 }
 // P3 has a connected gamut: derive its cusp from the independent XYZ
 // first-exit oracle at L=.1, then use RGB's cubic lightness homogeneity.
 // Keep the random workload's fractional hues, order and normalized L ranks.
 const ref = createBoundaryReference("display-p3"), below = [], above = [];
 let belowCount = 0;
 for (const [l, c, h] of randomSamples) {
  const u = ref.boundaries(.1, h).first / .1;
  const cusp = 1 / Math.cbrt(Math.max(...ref.linearRgb([1, u, h])));
  assert.ok(cusp > .02 && cusp < .98);
  const rank = (l - .01) / .98;
  const low = .01 + rank * (cusp - .02), high = cusp + .01 + rank * (.98 - cusp);
  assert.ok(low < cusp && high > cusp && low > 0 && high < 1);
  below.push([low, c, h]); above.push([high, c, h]);
  belowCount += Number(l < cusp);
 }
 add("display-p3", "below-cusp", below);
 add("display-p3", "above-cusp", above);
 assert.ok(result.filter(x => x.gamut === "display-p3").every(x => x.inside === 0));
 add("display-p3", "random-checked", randomSamples, true);
 add("display-p3", "mixed-checked", randomSamples.map(([l, c, h], i) => [l, i % 2 ? c : .01 * Math.min(l, 1 - l), h]), true);
 add("display-p3", "inside-checked", randomSamples.map(([l, , h]) => [l, .01 * Math.min(l, 1 - l), h]), true);
 assert.equal(result.at(-2).inside, samples.length / 2);
 assert.equal(result.at(-1).inside, samples.length);
 return { workloads: result, p3RandomBelowCusp: belowCount / samples.length };
}
