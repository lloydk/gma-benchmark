import assert from "node:assert/strict";
import { createReference } from "./css-minde-reference.js";
import { createClip } from "../../src/clip.js";
import { createCssMinde } from "../../src/css-minde.js";

// Also used by --validate-only. Comparing decoded channels avoids Rec.2020's
// unbounded encoding slope at zero without hiding perceptual differences.
export function validateRgbMethods (space, datasets, registered = { clip: createClip(space), "css-minde": createCssMinde(space) }) {
	const ref = createReference(space.id), maxima = {};
	for (const [name, map] of Object.entries(registered)) {
		let count = 0, linear = 0, delta = 0, encoded = 0;
		const out = [0, 0, 0];
		for (const samples of datasets) for (const input of samples) {
			map(input, out);
			assert.ok(out.every(v => Number.isFinite(v) && v >= 0 && v <= 1), `${space.id} ${name} ${input}: ${out}`);
			const expected = name === "clip" ? ref.encoded(input).map(v => Math.max(0, Math.min(1, v))) : ref.cssMinde(input).rgb;
			const le = Math.max(...out.map((v, i) => Math.abs(ref.decode(v) - ref.decode(expected[i]))));
			const lab = ref.lab(out), expectedLab = ref.lab(expected);
			const de = Math.hypot(...lab.map((v, i) => v - expectedLab[i]));
			assert.ok(le <= 2e-11 && de <= 2e-11, `${space.id} ${name} ${input}: linear ${le}, DeltaEOK ${de}, ${out} vs ${expected}`);
			linear = Math.max(linear, le); delta = Math.max(delta, de);
			encoded = Math.max(encoded, ...out.map((v, i) => Math.abs(v - expected[i])));
			count++;
		}
		maxima[name] = { count, linear, delta, encoded };
	}
	return { gamut: space.id, independentReference: maxima };
}
