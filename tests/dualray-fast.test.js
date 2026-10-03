import assert from "node:assert/strict";
import { test } from "node:test";
import { RGB_SPACES, DISPLAY_P3 } from "../src/rgb-spaces.js";
import { createDualrayFast, createDualrayFastTables } from "../src/dualray-fast.js";
import { buildWorkloads } from "../benchmark-workloads.js";
import { dualraySamples } from "./helpers/dualray-samples.js";
import { createCanonicalReference } from "./helpers/canonical-reference.js";
import { mapperOutput } from "./helpers/mapper-output.js";
import { validateDualrayFastMethod } from "./helpers/validate-dualray-fast-method.js";
import { createReference } from "./helpers/css-minde-reference.js";

test("dualray fast stays within its accuracy budget and keeps in-gamut colors", () => {
	const { randomSamples } = buildWorkloads();
	for (const space of Object.values(RGB_SPACES)) {
		const map = createDualrayFast(space);
		const probes = validateDualrayFastMethod(space, [dualraySamples(space.id)], map);
		const random = validateDualrayFastMethod(space, [randomSamples], map);
		// The runtime target's p99 (1e-4) on the random workload.
		assert.ok(random.deltaP99 <= 1e-4, `${space.id}: p99 ${random.deltaP99}`);
		console.log(`${space.id}: deltaEOK max ${probes.deltaMax.toExponential(2)} (probes), p99 ${random.deltaP99.toExponential(2)} (random)`);
	}
});

test("dualray fast keeps canonical in-gamut colors at out-of-range hues", () => {
	const hues = [-1e300, -1e16, -720.25, -95.947975, -0, 360, 1e7, 1e12, 10000000000005742, 3e38, 1e300];
	const samples = hues.flatMap(h => Array.from({ length: 39 * 40 }, (_, i) => [(1 + Math.floor(i / 40)) / 40, (i % 40) * 0.01, h]));
	for (const space of Object.values(RGB_SPACES)) assert.ok(validateDualrayFastMethod(space, [samples]).inside > 0);
	// Rust review reproducer: the reduced hue gives 262 degrees, the authored
	// direction differs, and the shortcut must not claim the in-gamut color.
	const input = [0.5, 0.3, 10000000000005742], canonical = createCanonicalReference(DISPLAY_P3)(input);
	assert.ok(canonical.inside);
	assert.deepEqual(mapperOutput(createDualrayFast(DISPLAY_P3), input), canonical.encoded);
});

test("dualray fast (tables) keeps the budget, in-gamut colors and plain Fast's outputs", () => {
	const { randomSamples } = buildWorkloads();
	const hues = [-1e300, -720.25, -0, 360, 1e12, 10000000000005742];
	const outOfRange = hues.flatMap(h => Array.from({ length: 39 * 40 }, (_, i) => [(1 + Math.floor(i / 40)) / 40, (i % 40) * 0.01, h]));
	// A dense grid, then near-white lightnesses at fractional hues, where the
	// face channel is nearly flat and the residual tolerance moves outputs most.
	const grid = [];
	for (let li = 1; li < 200; li++) for (let ci = 1; ci < 10; ci++) for (let hi = 0; hi < 1440; hi += 3) grid.push([li / 200, ci * 0.05, hi * 0.25]);
	for (const l of [0.996, 0.999, 0.9995, 0.9999, 0.99999, 0.999999])
		for (let ci = 1; ci < 10; ci++) for (let hi = 0; hi < 9000; hi++) grid.push([l, ci * 0.05, hi * 0.04 + 0.0071]);
	// Review reproducers (Rust f32 near white; Rec.2020 f64 4.6e-7 at bright yellow).
	grid.push([0.99999, 0.4, 103.945], [0.971, 0.4, 110]);
	for (const space of Object.values(RGB_SPACES)) {
		const tables = createDualrayFastTables(space), plain = createDualrayFast(space), ref = createReference(space.id);
		validateDualrayFastMethod(space, [dualraySamples(space.id)], tables);
		assert.ok(validateDualrayFastMethod(space, [randomSamples], tables).deltaP99 <= 1e-4, space.id);
		assert.ok(validateDualrayFastMethod(space, [outOfRange], tables).inside > 0, space.id);
		// Only the upper solve's seed and tolerance differ (Rust's f64 limit;
		// the sampled worst is about 5e-7, Rec.2020 bright yellow).
		let differ = 0, worst = 0;
		const a = [0, 0, 0], b = [0, 0, 0];
		for (const input of grid) {
			plain(input, a);
			tables(input, b);
			if (a[0] === b[0] && a[1] === b[1] && a[2] === b[2]) continue;
			differ++;
			worst = Math.max(worst, Math.hypot(...ref.lab(a).map((v, i) => v - ref.lab(b)[i])));
		}
		assert.ok(differ > 10_000 && worst <= 1e-6, `${space.id}: ${differ} differ, deltaEOK ${worst}`);
		console.log(`${space.id}: tables vs plain deltaEOK max ${worst.toExponential(2)} over ${differ} differing outputs`);
	}
});

test("dualray fast rejects targets without fits", () => {
	assert.throws(() => createDualrayFast({ ...DISPLAY_P3 }), RangeError);
});
