// Benchmarks OKLCh → Display-P3 gamut mapping over the grid shape used by
// color.js-org/apps/gamut-mapping/benchmark: oklch(L 0.4 H), H = 0..359 step 1,
// L = 0.99..0.01 step 0.01. Each method takes [L, C, H] and writes the clipped
// Display-P3 result into a reused 3-vector (no allocation per call).
//
//   npm run bench   (or: node bench.js)

import { bench, run, summary } from "mitata";

import { clip } from "./src/clip.js";
import { oklchCubic } from "./src/oklch-cubic.js";
import { edgeSeeker } from "./src/edge-seeker/index.js";

const CHROMA = 0.4;
const HUE_STEP = 1;
const LIGHTNESS_STEP = 0.01;

// Build the grid (lightest first, as the reference benchmark does).
const samples = [];
const den = Math.round(1 / LIGHTNESS_STEP);
const hi = Math.round((1 - LIGHTNESS_STEP) * den);
const lo = Math.round(LIGHTNESS_STEP * den);
for (let li = hi; li >= lo; li--) {
	const l = li / den;
	for (let h = 0; h < 360; h += HUE_STEP) {
		samples.push([l, CHROMA, h]);
	}
}
const n = samples.length;
console.log(`dataset: ${n.toLocaleString()} OKLCh colors, C=${CHROMA}, H=0..359 step ${HUE_STEP}, L=0.99..0.01 step ${LIGHTNESS_STEP}`);

const oklchCubicChecked = (oklch, out) => oklchCubic(oklch, out, true);
const edgeSeekerChecked = (oklch, out) => edgeSeeker(oklch, out, true);

const methods = [
	["clip", clip],
	["oklch-cubic (cached)", oklchCubic],
	["oklch-cubic (cached, in-gamut check)", oklchCubicChecked],
	["edge-seeker", edgeSeeker],
	["edge-seeker (in-gamut check)", edgeSeekerChecked],
];

const out = [0, 0, 0];
let sink = 0;

// Sanity: every method must yield an in-gamut Display-P3 color.
const inGamut = v => v[0] >= -1e-6 && v[0] <= 1 + 1e-6 && v[1] >= -1e-6 && v[1] <= 1 + 1e-6 && v[2] >= -1e-6 && v[2] <= 1 + 1e-6;
for (const [name, fn] of methods) {
	for (const s of samples) {
		fn(s, out);
		if (!inGamut(out)) {
			throw new Error(`${name} produced out-of-gamut P3 at oklch(${s.join(" ")}) -> ${out.join(" ")}`);
		}
	}
}
console.log("sanity: all methods produce in-gamut Display-P3 ✓\n");

const uncheckedOut = [0, 0, 0];
const checkedOut = [0, 0, 0];
let maxCheckedDiff = 0;
let maxCheckedSample = null;
for (const [unchecked, checked] of [[oklchCubic, oklchCubicChecked], [edgeSeeker, edgeSeekerChecked]]) {
	for (const s of samples) {
		unchecked(s, uncheckedOut);
		checked(s, checkedOut);
		for (let i = 0; i < 3; i++) {
			const diff = Math.abs(uncheckedOut[i] - checkedOut[i]);
			if (diff > maxCheckedDiff) {
				maxCheckedDiff = diff;
				maxCheckedSample = s;
			}
		}
	}
}
if (maxCheckedDiff > 1e-12) {
	throw new Error(`in-gamut check variants differ on the benchmark grid: max channel diff ${maxCheckedDiff} at oklch(${maxCheckedSample.join(" ")})`);
}
console.log(`equivalence: unchecked/in-gamut-check max channel diff ${maxCheckedDiff}\n`);

summary(() => {
	for (const [name, fn] of methods) {
		bench(name, () => {
			for (let i = 0; i < n; i++) {
				fn(samples[i], out);
				sink += out[0];
			}
		});
	}
});

await run();

if (!Number.isFinite(sink)) {
	console.log("sink", sink);
}
