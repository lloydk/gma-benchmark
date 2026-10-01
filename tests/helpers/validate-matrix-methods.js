import assert from "node:assert/strict";
import { createMatrixMappers } from "../../src/matrix-mappers.js";
import { createRgbConversions } from "../../src/rgb-convert.js";
import { blueFoldWindow, inBlueFold } from "../../src/matrix-solver-policy.js";
import { createBoundaryReference } from "./matrix-reference.js";

// Policy accuracy, separate from cross-language rounding and hue quantization.
// Every sample is finite-checked before accumulating maxima (NaN cannot pass).
const POLICIES = Object.freeze({
 "oklch-cubic": "bucket", "oklch-cubic-no-cache": "bucket",
 "oklch-cubic-direct": "first", "oklch-halley": "fold",
 "oklch-ostrowski": "fold", raytrace: "raytrace",
});
export function validateMatrixMethods (space, datasets, checked = false, registered = createMatrixMappers(space), oppositeMode) {
	assert.deepEqual(Object.keys(registered).sort(), Object.keys(POLICIES).sort());
	const maps = Object.entries(registered), ref = createBoundaryReference(space.id);
	const conversion = createRgbConversions(space), window = blueFoldWindow(space);
	const maxima = maps.map(([method]) => ({ method, linear: 0, delta: 0, encoded: 0, limit: POLICIES[method] === "bucket" ? 1e-6 : 2e-8 }));
	let count = 0;
	for (const samples of datasets) for (const input of samples) {
		const [L,C,H] = input, canonical = [];
		const achromatic = L <= 0 || L >= 1 || C <= 0;
		const preserve = !achromatic && checked && conversion.oklchToRgbIfInGamut(L,C,H,canonical);
		let normalized = H % 360;
		if (normalized < 0) normalized += 360;
		const bucket = Math.round(normalized*10)/10;
		const exact = achromatic || preserve ? null : ref.boundaries(L,H);
		const quantized = achromatic || preserve ? null : ref.boundaries(L,bucket);
		const outputs = {};
		for (let m = 0; m < maps.length; m++) {
			const [name,map] = maps[m], policy = POLICIES[name], actual = map(input,[],checked);
			outputs[name] = actual;
			assert.ok(actual.every(v => Number.isFinite(v) && v >= 0 && v <= 1), `${space.id} ${name} ${input}: ${actual}`);
			let expected;
			if (achromatic) expected = ref.linearRgb([Math.max(0,Math.min(1,L)),0,H]);
			else if (preserve) { assert.deepEqual(actual,canonical); expected = canonical.map(ref.decode); }
			else if (policy === "raytrace") expected = ref.raytrace(input);
			else {
				const hue = policy === "bucket" ? bucket : H;
				const chroma = policy === "fold" && inBlueFold(H,window)
					? ref.foldChroma(L,C,H,exact)
					: Math.min(C, policy === "bucket" ? quantized.first : exact.first);
				expected = ref.linearRgb([L,chroma,hue]);
				assert.ok(expected.every(v => v >= -1e-12 && v <= 1+1e-12), `reference chose an infeasible chroma: ${space.id} ${name} ${input}: ${expected}`);
			}
			const encoded = expected.map(ref.encode);
			const linear = Math.max(...actual.map((v,i) => Math.abs(ref.decode(v)-expected[i])));
			const a = ref.lab(actual), b = ref.lab(encoded), delta = Math.hypot(...a.map((v,i) => v-b[i]));
			const max = maxima[m];
			assert.ok(Number.isFinite(linear) && Number.isFinite(delta) && linear <= max.limit && delta <= max.limit, `${space.id} ${name} checked=${checked} ${input}: linear ${linear}, delta ${delta}; ${actual} vs ${encoded}`);
			if (linear > max.linear) { max.linear = linear; max.input = input; }
			max.delta = Math.max(max.delta,delta);
			max.encoded = Math.max(max.encoded,...actual.map((v,i) => Math.abs(v-encoded[i])));
			// Compare the selected timed callback with the other entry mode only
			// outside gamut; bucketed policies may intentionally differ inside.
			if (oppositeMode && !conversion.oklchToRgbIfInGamut(L,Math.max(0,C),H,[])) {
				assert.deepEqual(actual, oppositeMode[name](input,[]), `${space.id} ${name} checked/plain ${input}`);
			}
		}
		assert.deepEqual(outputs["oklch-cubic"],outputs["oklch-cubic-no-cache"], `${space.id} cached/no-cache ${input}`);
		count++;
	}
	return { gamut: space.id, checked, count, independentReference: maxima };
}
