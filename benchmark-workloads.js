export const CHROMA = 0.4;
export const HUE_STEP = 1;
export const LIGHTNESS_STEP = 0.01;

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

export function buildWorkloads () {
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

	return { samples, randomSamples };
}
