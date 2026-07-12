// Benchmarks OKLCh → Display-P3 gamut mapping over the grid shape used by
// color.js-org/apps/gamut-mapping/benchmark: oklch(L 0.4 H), H = 0..359 step 1,
// L = 0.99..0.01 step 0.01. Each method takes [L, C, H] and writes the clipped
// Display-P3 result into a reused 3-vector (no allocation per call).
//
//   npm run bench   (or: node bench.js)

import { bench, run, summary } from "mitata";

import { clip } from "./src/clip.js";
import { oklchCubic } from "./src/oklch-cubic.js";
import { oklchCubicNoCache } from "./src/oklch-cubic-no-cache.js";
import { oklchHalley } from "./src/oklch-halley.js";
import { bottossonLightness, bottossonLightnessCached } from "./src/bottosson-lightness.js";
import { edgeSeeker, edgeSeekerIndexed } from "./src/edge-seeker/index.js";
import { raytrace } from "./src/raytrace.js";

const CHROMA = 0.4;
const HUE_STEP = 1;
const LIGHTNESS_STEP = 0.01;

// Small deterministic PRNG so the random workload is reproducible run to run.
function mulberry32 (a) {
	return function () {
		a |= 0;
		a = (a + 0x6D2B79F5) | 0;
		let t = Math.imul(a ^ (a >>> 15), 1 | a);
		t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
		return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
	};
}

// `count` stratified/jittered values evenly covering [min, min+range) — one
// random sample per equal bin — then Fisher–Yates shuffled so they don't arrive
// in sorted order. Deterministic via `seed`.
function stratifiedShuffled (count, min, range, seed) {
	const rand = mulberry32(seed);
	const values = new Array(count);
	for (let i = 0; i < count; i++) {
		values[i] = min + (i + rand()) * (range / count);
	}
	for (let i = count - 1; i > 0; i--) {
		const j = Math.floor(rand() * (i + 1));
		const tmp = values[i];
		values[i] = values[j];
		values[j] = tmp;
	}
	return values;
}

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

// Build a random workload: same sample count as the grid, but every lightness
// and hue is an independent stratified/jittered fractional value (even coverage
// of its range, shuffled). The grid repeats just 360 integer hues and 99 fixed
// lightness steps, which keeps the gamut-edge lookup cache-hot and the dark/
// bright branches predictable; arbitrary non-repeating input is closer to real-
// world gamut mapping. Lightness covers the same 0.01..0.99 range as the grid.
const randHues = stratifiedShuffled(n, 0, 360, 0x9e3779b9);
const randLightness = stratifiedShuffled(n, LIGHTNESS_STEP, 1 - 2 * LIGHTNESS_STEP, 0x85ebca6b);
const randomSamples = [];
for (let i = 0; i < n; i++) {
	randomSamples.push([randLightness[i], CHROMA, randHues[i]]);
}
console.log(`random:  ${randomSamples.length.toLocaleString()} OKLCh colors, C=${CHROMA}, H=stratified/jittered 0..360, L=stratified/jittered 0.01..0.99 (both shuffled)`);

const oklchCubicChecked = (oklch, out) => oklchCubic(oklch, out, true);
const oklchCubicNoCacheChecked = (oklch, out) => oklchCubicNoCache(oklch, out, true);
const oklchHalleyChecked = (oklch, out) => oklchHalley(oklch, out, true);
const bottossonLightnessChecked = (oklch, out) => bottossonLightness(oklch, out, true);
const bottossonLightnessCachedChecked = (oklch, out) => bottossonLightnessCached(oklch, out, true);
const edgeSeekerChecked = (oklch, out) => edgeSeeker(oklch, out, true);
const edgeSeekerIndexedChecked = (oklch, out) => edgeSeekerIndexed(oklch, out, true);
const raytraceChecked = (oklch, out) => raytrace(oklch, out, true);

// `--in-gamut-check` runs the in-gamut-precheck variant of every method instead
// of the plain one, so a run shows one mode at a time rather than both mixed.
const inGamutCheck = process.argv.slice(2).includes("--in-gamut-check");
console.log(`in-gamut precheck: ${inGamutCheck ? "ENABLED (--in-gamut-check)" : "disabled (pass --in-gamut-check to enable)"}\n`);

const methods = inGamutCheck ? [
	["clip", clip],
	["oklch-cubic (cached)", oklchCubicChecked],
	["oklch-cubic (no cache)", oklchCubicNoCacheChecked],
	["oklch-halley", oklchHalleyChecked],
	["bottosson-lightness", bottossonLightnessChecked],
	["bottosson-lightness (cached)", bottossonLightnessCachedChecked],
	["edge-seeker", edgeSeekerChecked],
	["edge-seeker (indexed)", edgeSeekerIndexedChecked],
	["raytrace", raytraceChecked],
] : [
	["clip", clip],
	["oklch-cubic (cached)", oklchCubic],
	["oklch-cubic (no cache)", oklchCubicNoCache],
	["oklch-halley", oklchHalley],
	["bottosson-lightness", bottossonLightness],
	["bottosson-lightness (cached)", bottossonLightnessCached],
	["edge-seeker", edgeSeeker],
	["edge-seeker (indexed)", edgeSeekerIndexed],
	["raytrace", raytrace],
];

const out = [0, 0, 0];
let sink = 0;

// Sanity: every method must yield an in-gamut Display-P3 color.
const inGamut = v => v[0] >= -1e-6 && v[0] <= 1 + 1e-6 && v[1] >= -1e-6 && v[1] <= 1 + 1e-6 && v[2] >= -1e-6 && v[2] <= 1 + 1e-6;
for (const [name, fn] of methods) {
	for (const dataset of [samples, randomSamples]) {
		for (const s of dataset) {
			fn(s, out);
			if (!inGamut(out)) {
				throw new Error(`${name} produced out-of-gamut P3 at oklch(${s.join(" ")}) -> ${out.join(" ")}`);
			}
		}
	}
}
console.log("sanity: all methods produce in-gamut Display-P3 ✓\n");

const uncheckedOut = [0, 0, 0];
const checkedOut = [0, 0, 0];
let maxCheckedDiff = 0;
let maxCheckedSample = null;
let maxCheckedDataset = null;
for (const [unchecked, checked] of [
	[oklchCubic, oklchCubicChecked],
	[oklchCubicNoCache, oklchCubicNoCacheChecked],
	[oklchHalley, oklchHalleyChecked],
	[bottossonLightness, bottossonLightnessChecked],
	[bottossonLightnessCached, bottossonLightnessCachedChecked],
	[edgeSeeker, edgeSeekerChecked],
	[edgeSeekerIndexed, edgeSeekerIndexedChecked],
	[raytrace, raytraceChecked],
]) {
	for (const [label, dataset] of [["grid", samples], ["random", randomSamples]]) {
		for (const s of dataset) {
			unchecked(s, uncheckedOut);
			checked(s, checkedOut);
			for (let i = 0; i < 3; i++) {
				const diff = Math.abs(uncheckedOut[i] - checkedOut[i]);
				if (diff > maxCheckedDiff) {
					maxCheckedDiff = diff;
					maxCheckedSample = s;
					maxCheckedDataset = label;
				}
			}
		}
	}
}
if (maxCheckedDiff > 1e-12) {
	throw new Error(`in-gamut check variants differ on the ${maxCheckedDataset} workload: max channel diff ${maxCheckedDiff} at oklch(${maxCheckedSample.join(" ")})`);
}
console.log(`equivalence: unchecked/in-gamut-check max channel diff ${maxCheckedDiff} (grid + random)\n`);

let maxCubicNoCacheDiff = 0;
let maxCubicNoCacheSample = null;
let maxCubicNoCacheDataset = null;
for (const [label, dataset] of [["grid", samples], ["random", randomSamples]]) {
	for (const s of dataset) {
		oklchCubic(s, uncheckedOut);
		oklchCubicNoCache(s, checkedOut);
		for (let i = 0; i < 3; i++) {
			const diff = Math.abs(uncheckedOut[i] - checkedOut[i]);
			if (diff > maxCubicNoCacheDiff) {
				maxCubicNoCacheDiff = diff;
				maxCubicNoCacheSample = s;
				maxCubicNoCacheDataset = label;
			}
		}
	}
}
if (maxCubicNoCacheDiff > 1e-12) {
	throw new Error(`oklch-cubic no-cache differs on the ${maxCubicNoCacheDataset} workload: max channel diff ${maxCubicNoCacheDiff} at oklch(${maxCubicNoCacheSample.join(" ")})`);
}
console.log(`equivalence: oklch-cubic cached/no-cache max channel diff ${maxCubicNoCacheDiff} (grid + random)\n`);

// Both methods find the exact constant-L/H P3 boundary. Halley's 1e-9 chroma
// stopping threshold can be amplified slightly by the transfer function near
// black, so compare encoded channels with the corresponding output tolerance.
let maxHalleyCubicDiff = 0;
let maxHalleyCubicSample = null;
for (const s of samples) {
	oklchHalley(s, uncheckedOut);
	oklchCubicNoCache(s, checkedOut);
	for (let i = 0; i < 3; i++) {
		const diff = Math.abs(uncheckedOut[i] - checkedOut[i]);
		if (diff > maxHalleyCubicDiff) {
			maxHalleyCubicDiff = diff;
			maxHalleyCubicSample = s;
		}
	}
}
if (maxHalleyCubicDiff > 2e-8) {
	throw new Error(`oklch-halley differs from the exact cubic boundary: max channel diff ${maxHalleyCubicDiff} at oklch(${maxHalleyCubicSample.join(" ")})`);
}
console.log(`equivalence: oklch-halley/cubic max channel diff ${maxHalleyCubicDiff.toExponential(2)} (exact grid hues)\n`);

// The cached bottosson variant evaluates the hue-dependent structure (cusp +
// LMS' slopes) at the 0.1° bucket hue. On the grid the integer hues hit bucket
// centers exactly, so it must match the exact method to float noise; on random
// fractional hues the difference is bounded by the hue quantization.
let maxBottossonCachedGridDiff = 0;
let maxBottossonCachedGridSample = null;
for (const s of samples) {
	bottossonLightness(s, uncheckedOut);
	bottossonLightnessCached(s, checkedOut);
	for (let i = 0; i < 3; i++) {
		const diff = Math.abs(uncheckedOut[i] - checkedOut[i]);
		if (diff > maxBottossonCachedGridDiff) {
			maxBottossonCachedGridDiff = diff;
			maxBottossonCachedGridSample = s;
		}
	}
}
if (maxBottossonCachedGridDiff > 1e-12) {
	throw new Error(`bottosson cached differs on bucket-exact grid hues: max channel diff ${maxBottossonCachedGridDiff} at oklch(${maxBottossonCachedGridSample.join(" ")})`);
}
let maxBottossonCachedRandomDiff = 0;
let maxBottossonCachedRandomSample = null;
for (const s of randomSamples) {
	bottossonLightness(s, uncheckedOut);
	bottossonLightnessCached(s, checkedOut);
	for (let i = 0; i < 3; i++) {
		const diff = Math.abs(uncheckedOut[i] - checkedOut[i]);
		if (diff > maxBottossonCachedRandomDiff) {
			maxBottossonCachedRandomDiff = diff;
			maxBottossonCachedRandomSample = s;
		}
	}
}
if (maxBottossonCachedRandomDiff > 0.05) {
	throw new Error(`bottosson cached exceeds the hue-quantization bound on random hues: max channel diff ${maxBottossonCachedRandomDiff} at oklch(${maxBottossonCachedRandomSample.join(" ")})`);
}
console.log(`equivalence: bottosson cached/exact max channel diff ${maxBottossonCachedGridDiff} (grid, bucket-exact hues), ${maxBottossonCachedRandomDiff.toExponential(2)} (random, 0.1° hue quantization)\n`);

let maxIndexedDiff = 0;
let maxIndexedSample = null;
let maxIndexedDataset = null;
for (const [label, dataset] of [["grid", samples], ["random", randomSamples]]) {
	for (const s of dataset) {
		edgeSeeker(s, uncheckedOut);
		edgeSeekerIndexed(s, checkedOut);
		for (let i = 0; i < 3; i++) {
			const diff = Math.abs(uncheckedOut[i] - checkedOut[i]);
			if (diff > maxIndexedDiff) {
				maxIndexedDiff = diff;
				maxIndexedSample = s;
				maxIndexedDataset = label;
			}
		}
	}
}
if (maxIndexedDiff !== 0) {
	throw new Error(`edge-seeker indexed differs on the ${maxIndexedDataset} workload: max channel diff ${maxIndexedDiff} at oklch(${maxIndexedSample.join(" ")})`);
}
console.log("equivalence: edge-seeker indexed max channel diff 0 (grid + random)\n");

// Grid workload: fixed integer hues 0..359, repeated at every lightness.
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

// Random workload: stratified/jittered fractional hues, shuffled.
summary(() => {
	for (const [name, fn] of methods) {
		bench(`${name} (random hues)`, () => {
			for (let i = 0; i < randomSamples.length; i++) {
				fn(randomSamples[i], out);
				sink += out[0];
			}
		});
	}
});

await run();

if (!Number.isFinite(sink)) {
	console.log("sink", sink);
}
