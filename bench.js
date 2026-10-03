import { buildWorkloads, CHROMA, HUE_STEP, LIGHTNESS_STEP } from "./benchmark-workloads.js";
// Benchmarks OKLCh → target RGB gamut mapping over the grid shape used by
// color.js-org/apps/gamut-mapping/benchmark: oklch(L 0.4 H), H = 0..359 step 1,
// L = 0.99..0.01 step 0.01. Each method takes [L, C, H] and writes the clipped
// target RGB result into a reused 3-vector (no allocation per call).
//
//   npm run bench   (or: node bench.js / bun bench.js)
//   Validation and timing run in separate processes by default.

import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";
import { bench, run, summary } from "mitata";

import { clip as p3Clip, createClip } from "./src/clip.js";
import { cssMinde as p3CssMinde, createCssMinde } from "./src/css-minde.js";

import { getRgbSpace } from "./src/rgb-spaces.js";

const { values } = parseArgs({ options: {
	"in-gamut-check": { type: "boolean", default: false },
	"validate-only": { type: "boolean", default: false },
	"timing-only": { type: "boolean", default: false },
	gamut: { type: "string", default: "display-p3" },
	help: { type: "boolean", short: "h", default: false },
	warmup: { type: "string", default: "50" },
} });
if (values.help) {
	console.log("Usage: node bench.js [--gamut display-p3|srgb|rec2020|all] [--validate-only|--timing-only] [--in-gamut-check] [--warmup 50]\nDefault: display-p3, all 15 methods. sRGB and Rec.2020: all 15 methods.\nEach target runs in a separate process; validation is separate from timing.");
	process.exit(0);
}
const gamut = values.gamut;
if (gamut !== "all") getRgbSpace(gamut);
const validateOnly = values["validate-only"];
const timingOnly = values["timing-only"];
const inGamutCheck = values["in-gamut-check"];
const warmup = Number(values.warmup);
if (validateOnly && timingOnly) {
	throw new Error("--validate-only and --timing-only are mutually exclusive");
}
if (!/^\d+$/.test(values.warmup) || !Number.isSafeInteger(warmup) || warmup < 1) {
	throw new Error("--warmup must be a positive safe integer (complete workload passes)");
}

// Isolate targets too: one target's factory calls must not train another's
// inline caches. Child flags preserve the caller's mode and warmup settings.
if (gamut === "all") {
	for (const target of ["display-p3", "srgb", "rec2020"]) {
		const args = ["--gamut", target, "--warmup", String(warmup)];
		for (const flag of ["validate-only", "timing-only", "in-gamut-check"]) if (values[flag]) args.push(`--${flag}`);
		const result = spawnSync(process.execPath, [...(process.versions.bun ? [] : ["--expose-gc"]), fileURLToPath(import.meta.url), ...args], { stdio: "inherit" });
		if (result.error) throw result.error;
		if (result.signal) { process.kill(process.pid, result.signal); process.exit(1); }
		if (result.status !== 0) process.exit(result.status ?? 1);
	}
	process.exit(0);
}
const space = getRgbSpace(gamut);
const clip = gamut === "display-p3" ? p3Clip : createClip(space);
const cssMinde = gamut === "display-p3" ? p3CssMinde : createCssMinde(space);
console.log(`gamut: ${gamut}`);

// Never let validation's checked/unchecked calls or rare probes prepare the
// timing process. Direct invocations and npm scripts use the same entry point.
if (!validateOnly && !timingOnly) {
	const flags = process.versions.bun ? [] : ["--expose-gc"];
	const script = fileURLToPath(import.meta.url);
	const args = ["--warmup", String(warmup), "--gamut", gamut];
	if (inGamutCheck) args.push("--in-gamut-check");
	for (const mode of ["--validate-only", "--timing-only"]) {
		console.log(mode === "--validate-only"
			? "Validating in a separate process..."
			: "Validation passed; starting a fresh timing process...");
		const result = spawnSync(process.execPath, [...flags, script, ...args, mode], { stdio: "inherit" });
		if (result.error) throw result.error;
		if (result.signal) {
			process.kill(process.pid, result.signal);
			process.exit(1);
		}
		if (result.status !== 0) process.exit(result.status ?? 1);
	}
	process.exit(0);
}

const { createMatrixMappers } = await import("./src/matrix-mappers.js");
const matrixMappers = createMatrixMappers(space);
const {
 "oklch-cubic": oklchCubic, "oklch-cubic-no-cache": oklchCubicNoCache,
 "oklch-cubic-direct": oklchCubicDirect, "oklch-halley": oklchHalley,
 "oklch-ostrowski": oklchOstrowski, raytrace,
} = matrixMappers;
const { createBottossonMappers } = await import("./src/bottosson-factory.js");
const bottossonMappers = createBottossonMappers(space);
const { "bottosson-lightness": bottossonLightness, "bottosson-lightness-cached": bottossonLightnessCached } = bottossonMappers;
const { createEdgeSeekerMappers } = await import("./src/edge-seeker/factory.js");
const edgeSeekerMappers = createEdgeSeekerMappers(space);
const { "edge-seeker": edgeSeeker, "edge-seeker-indexed": edgeSeekerIndexed } = edgeSeekerMappers;
const { createDualray } = await import("./src/dualray-factory.js");
const dualray = createDualray(space);
const { createDualrayFast, createDualrayFastTables } = await import("./src/dualray-fast.js");
const dualrayFast = createDualrayFast(space);
const dualrayFastTables = createDualrayFastTables(space);

const { samples, randomSamples } = buildWorkloads();
const n = samples.length;
console.log(`dataset: ${n.toLocaleString()} OKLCh colors, C=${CHROMA}, H=0..359 step ${HUE_STEP}, L=0.99..0.01 step ${LIGHTNESS_STEP}`);
console.log(`random:  ${randomSamples.length.toLocaleString()} OKLCh colors, C=${CHROMA}, H=stratified/jittered 0..360, L=stratified/jittered 0.01..0.99 (both shuffled)`);

const oklchCubicChecked = (oklch, out) => oklchCubic(oklch, out, true);
const oklchCubicNoCacheChecked = (oklch, out) => oklchCubicNoCache(oklch, out, true);
const oklchCubicDirectChecked = (oklch, out) => oklchCubicDirect(oklch, out, true);
const oklchHalleyChecked = (oklch, out) => oklchHalley(oklch, out, true);
const oklchOstrowskiChecked = (oklch, out) => oklchOstrowski(oklch, out, true);
const bottossonLightnessChecked = (oklch, out) => bottossonLightness(oklch, out, true);
const bottossonLightnessCachedChecked = (oklch, out) => bottossonLightnessCached(oklch, out, true);
const edgeSeekerChecked = (oklch, out) => edgeSeeker(oklch, out, true);
const edgeSeekerIndexedChecked = (oklch, out) => edgeSeekerIndexed(oklch, out, true);
const raytraceChecked = (oklch, out) => raytrace(oklch, out, true);

// `--in-gamut-check` runs the in-gamut-precheck variant of every method instead
// of the plain one, so a run shows one mode at a time rather than both mixed.
console.log(`in-gamut precheck: ${inGamutCheck ? "ENABLED (--in-gamut-check)" : "disabled (pass --in-gamut-check to enable)"}\n`);

const methods = [
	["clip", clip],
	["css-minde", cssMinde], // intrinsic membership check in both modes
	["oklch-cubic (cached)", inGamutCheck ? oklchCubicChecked : oklchCubic, "oklch-cubic"],
	["oklch-cubic (no cache)", inGamutCheck ? oklchCubicNoCacheChecked : oklchCubicNoCache, "oklch-cubic-no-cache"],
	["oklch-cubic-direct", inGamutCheck ? oklchCubicDirectChecked : oklchCubicDirect],
	["oklch-halley", inGamutCheck ? oklchHalleyChecked : oklchHalley],
	["oklch-ostrowski", inGamutCheck ? oklchOstrowskiChecked : oklchOstrowski],
	["dualray", dualray], // intrinsic checks in both modes
	["dualray fast", dualrayFast, "dualray-fast"], // canonical precheck in both modes
	["dualray fast (tables)", dualrayFastTables, "dualray-fast-tables"], // likewise
	["bottosson-lightness", inGamutCheck ? bottossonLightnessChecked : bottossonLightness],
	["bottosson-lightness (cached)", inGamutCheck ? bottossonLightnessCachedChecked : bottossonLightnessCached, "bottosson-lightness-cached"],
	["edge-seeker", inGamutCheck ? edgeSeekerChecked : edgeSeeker],
	["edge-seeker (indexed)", inGamutCheck ? edgeSeekerIndexedChecked : edgeSeekerIndexed, "edge-seeker-indexed"],
	["raytrace", inGamutCheck ? raytraceChecked : raytrace],
];

const out = [0, 0, 0];
// Keep every channel of every mapped color observable in the timed loops.
let sink = 0;

if (validateOnly) {
	// Sanity: every registered method must yield an in-gamut target color.
	const inGamut = v => v[0] >= -1e-6 && v[0] <= 1 + 1e-6 && v[1] >= -1e-6 && v[1] <= 1 + 1e-6 && v[2] >= -1e-6 && v[2] <= 1 + 1e-6;
	for (const [name, fn] of methods) {
		for (const dataset of [samples, randomSamples]) {
			for (const s of dataset) {
				fn(s, out);
				if (!inGamut(out)) {
					throw new Error(`${name} produced out-of-gamut ${gamut} at oklch(${s.join(" ")}) -> ${out.join(" ")}`);
				}
			}
		}
	}
	console.log(`sanity: all methods produce in-gamut ${gamut} ✓\n`);
	const { validateRgbMethods } = await import("./tests/helpers/validate-rgb-methods.js");
	const registered = Object.fromEntries(methods.map(([label,map,id = label]) => [id,map]));
	console.log(validateRgbMethods(space, [samples, randomSamples], { clip: registered.clip, "css-minde": registered["css-minde"] }));
	const { validateMatrixMethods } = await import("./tests/helpers/validate-matrix-methods.js");
	const { mappingProbes } = await import("./tests/helpers/matrix-samples.js");
	const { bindMapperMode } = await import("./tests/helpers/canonical-reference.js");
	const { validateBottossonMethods } = await import("./tests/helpers/validate-bottosson-methods.js");
	const { bottossonProbes } = await import("./tests/helpers/bottosson-samples.js");
	const { validateEdgeSeekerMethods } = await import("./tests/helpers/validate-edge-seeker-methods.js");
	const { edgeSeekerProbes } = await import("./tests/helpers/edge-seeker-samples.js");
	for (const [maps,validate,probes] of [
		[matrixMappers,validateMatrixMethods,mappingProbes()],
		[bottossonMappers,validateBottossonMethods,bottossonProbes(gamut)],
		[edgeSeekerMappers,validateEdgeSeekerMethods,edgeSeekerProbes(gamut)],
	]) {
		const selected = Object.fromEntries(Object.keys(maps).map(name => [name,registered[name]]));
		console.log(validate(space,[samples,randomSamples,probes],inGamutCheck,selected,bindMapperMode(maps,!inGamutCheck)));
	}
	const { validateDualrayMethod } = await import("./tests/helpers/validate-dualray-method.js");
	const { dualrayProbes } = await import("./tests/helpers/dualray-samples.js");
	console.log(validateDualrayMethod(space,[samples,randomSamples,dualrayProbes(gamut)],registered.dualray));
	const { validateDualrayFastMethod } = await import("./tests/helpers/validate-dualray-fast-method.js");
	for (const id of ["dualray-fast","dualray-fast-tables"]) console.log(validateDualrayFastMethod(space,[samples,randomSamples,dualrayProbes(gamut)],registered[id]));
	if (gamut !== "display-p3") process.exit(0);

	const uncheckedOut = [0, 0, 0];
	const checkedOut = [0, 0, 0];
	let maxCheckedDiff = 0;
	let maxCheckedSample = null;
	let maxCheckedDataset = null;
	for (const [unchecked, checked] of [
		[oklchCubic, oklchCubicChecked],
		[oklchCubicNoCache, oklchCubicNoCacheChecked],
		[oklchCubicDirect, oklchCubicDirectChecked],
		[oklchHalley, oklchHalleyChecked],
		[oklchOstrowski, oklchOstrowskiChecked],
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

	// The direct cubic and iterative methods find the exact constant-L/H P3
	// boundary. The grid uses exact bucket-center hues, so the existing no-cache
	// cubic is a closed-form reference. Iterative stopping thresholds can be
	// amplified slightly by the transfer function near black.
	for (const [name, fn, tolerance] of [
		["oklch-cubic-direct", oklchCubicDirect, 5e-8],
		["oklch-halley", oklchHalley, 2e-8],
		["oklch-ostrowski", oklchOstrowski, 5e-8],
	]) {
		let maxCubicDiff = 0;
		let maxCubicSample = null;
		for (const s of samples) {
			fn(s, uncheckedOut);
			oklchCubicNoCache(s, checkedOut);
			for (let i = 0; i < 3; i++) {
				const diff = Math.abs(uncheckedOut[i] - checkedOut[i]);
				if (diff > maxCubicDiff) {
					maxCubicDiff = diff;
					maxCubicSample = s;
				}
			}
		}
		if (maxCubicDiff > tolerance) {
			throw new Error(`${name} differs from the exact cubic boundary: max channel diff ${maxCubicDiff} at oklch(${maxCubicSample.join(" ")})`);
		}
		console.log(`equivalence: ${name}/cubic max channel diff ${maxCubicDiff.toExponential(2)} (exact grid hues)`);
	}
	console.log();

	let maxDirectHalleyDiff = 0;
	let maxDirectHalleySample = null;
	for (const s of randomSamples) {
		oklchCubicDirect(s, uncheckedOut);
		oklchHalley(s, checkedOut);
		for (let i = 0; i < 3; i++) {
			const diff = Math.abs(uncheckedOut[i] - checkedOut[i]);
			if (diff > maxDirectHalleyDiff) {
				maxDirectHalleyDiff = diff;
				maxDirectHalleySample = s;
			}
		}
	}
	if (maxDirectHalleyDiff > 5e-8) {
		throw new Error(`oklch-cubic-direct differs from Halley on random exact hues: max channel diff ${maxDirectHalleyDiff} at oklch(${maxDirectHalleySample.join(" ")})`);
	}
	console.log(`equivalence: oklch-cubic-direct/Halley max channel diff ${maxDirectHalleyDiff.toExponential(2)} (random exact hues)\n`);

	let maxDualrayDiff = 0;
	for (const dataset of [samples, randomSamples]) {
		for (const s of dataset) {
			dualray(s, uncheckedOut);
			oklchCubicDirect(s, checkedOut);
			for (let i = 0; i < 3; i++) {
				maxDualrayDiff = Math.max(maxDualrayDiff, Math.abs(uncheckedOut[i] - checkedOut[i]));
			}
		}
	}
	if (!(maxDualrayDiff <= 5e-8)) {
		throw new Error(`dualray differs from cubic-direct: max channel diff ${maxDualrayDiff}`);
	}
	console.log(`equivalence: dualray/cubic-direct max channel diff ${maxDualrayDiff.toExponential(2)} (grid + random)\n`);

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
	process.exit(0);
}

console.log(`warmup: ${warmup} complete passes per method/workload, excluded from timing\n`);

// Mitata executes the generator setup before measuring the yielded callback.
// Warm the exact timed call site and input/mode, consuming every output channel.
function* warmed (batch) {
	for (let pass = 0; pass < warmup; pass++) batch();
	if (!Number.isFinite(sink)) throw new Error(`non-finite warmup checksum: ${sink}`);
	yield batch;
}

// Grid workload: fixed integer hues 0..359, repeated at every lightness.
summary(() => {
	for (const [name, fn] of methods) {
		const batch = () => {
			for (let i = 0; i < n; i++) {
				fn(samples[i], out);
				sink += out[0] + out[1] + out[2];
			}
		};
		bench(`${gamut} / ${name}`, function* () { yield* warmed(batch); });
	}
});

// Random workload: stratified/jittered fractional hues, shuffled.
summary(() => {
	for (const [name, fn] of methods) {
		const batch = () => {
			for (let i = 0; i < randomSamples.length; i++) {
				fn(randomSamples[i], out);
				sink += out[0] + out[1] + out[2];
			}
		};
		bench(`${gamut} / ${name} (random hues)`, function* () { yield* warmed(batch); });
	}
});

await run({ throw: true });

if (!Number.isFinite(sink)) {
	throw new Error(`non-finite benchmark checksum: ${sink}`);
}
console.log(`benchmark checksum: ${sink}`);
