import assert from "node:assert/strict";
import { test } from "node:test";
import { RGB_SPACES, DISPLAY_P3 } from "../src/rgb-spaces.js";
import { createDualrayFast } from "../src/dualray-fast.js";
import { buildWorkloads } from "../benchmark-workloads.js";
import { dualraySamples } from "./helpers/dualray-samples.js";
import { createCanonicalReference } from "./helpers/canonical-reference.js";
import { mapperOutput } from "./helpers/mapper-output.js";
import { validateDualrayFastMethod } from "./helpers/validate-dualray-fast-method.js";

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

test("dualray fast rejects targets without fits", () => {
	assert.throws(() => createDualrayFast({ ...DISPLAY_P3 }), RangeError);
});
