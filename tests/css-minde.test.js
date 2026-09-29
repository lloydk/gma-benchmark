import assert from "node:assert/strict";
import { test } from "node:test";
import { readFileSync } from "node:fs";
import { cssMinde } from "../src/css-minde.js";
import { oklchToP3IfInGamut, oklchToClippedP3 } from "../src/convert.js";
import { referenceCssMinde, referenceP3 } from "./helpers/css-minde-reference.js";

function verify (input) {
	const expected = referenceCssMinde(input);
	const out = [];
	assert.equal(cssMinde(input, out), out);
	for (let i = 0; i < 3; i++) {
		assert.ok(Number.isFinite(out[i]) && out[i] >= 0 && out[i] <= 1, `${input}: ${out}`);
		assert.ok(Math.abs(out[i] - expected.rgb[i]) <= 2e-12, `${input}: ${out} vs ${expected.rgb}`);
	}
	return expected;
}

test("css-minde matches the spec reference over grid and mixed chroma", () => {
	const reasons = new Set();
	let insideSteps = 0;
	for (let li = 1; li < 100; li++) {
		for (let h = 0; h < 360; h++) {
			const ref = verify([li / 100, 0.4, h]);
			reasons.add(ref.reason);
			insideSteps += ref.trace.filter(step => step.action === "in-gamut").length;
		}
	}
	for (let i = 0; i < 8192; i++) {
		const ref = verify([0.001 + 0.998 * ((i * 0.7548776662466927) % 1), 0.5 * ((i * 0.5698402909980532) % 1), (i * 137.50776405003785) % 360]);
		reasons.add(ref.reason);
	}
	assert.deepEqual([...reasons].sort(), ["close-enough", "in-gamut", "initial-clip", "interval"]);
	assert.ok(insideSteps > 0);
});

test("css-minde preserves canonical in-gamut conversion exactly and supports aliased output", () => {
	let accepted = 0;
	for (const l of [0.01, 0.1, 0.5, 0.9, 0.99]) {
		for (const c of [0, 0.001, 0.02, 0.1, 0.4]) {
			for (let h = -360; h <= 720; h += 13.37) {
				const input = [l, c, h], expected = [];
				const actual = cssMinde(input, []);
				if (oklchToP3IfInGamut(l, c, h, expected)) {
					assert.deepEqual(actual, expected);
					accepted++;
				}
				assert.equal(cssMinde(input, input), input);
				assert.deepEqual(input, actual);
			}
		}
	}
	assert.ok(accepted > 1000);
});

test("css-minde handles endpoints, powerless hue, negative chroma, and extreme finite hues", () => {
	for (const l of [-1, 0, 1, 2]) {
		assert.deepEqual(cssMinde([l, 0.4, 30], []), [l <= 0 ? 0 : 1, l <= 0 ? 0 : 1, l <= 0 ? 0 : 1]);
	}
	for (const c of [-0.1, 0]) {
		assert.deepEqual(cssMinde([0.5, c, NaN], []), oklchToClippedP3(0.5, 0, 0, []));
	}
	for (const h of [-Number.MAX_VALUE, -1e21, -1e9, 1e9, 1e21, Number.MAX_VALUE]) {
		assert.deepEqual(cssMinde([0.5, 0.4, h], []), cssMinde([0.5, 0.4, h % 360], []));
	}
});

test("css-minde retains the initial-clip shortcut and last-clip interval result", () => {
	let initial = 0, aboveJnd = 0;
	for (const l of [0.1, 0.3, 0.5, 0.7, 0.9]) {
		for (let h = 0; h < 360; h++) {
			const input = [l, 0.4, h];
			const ref = verify(input);
			if (ref.reason === "interval" && ref.error >= 0.02) aboveJnd++;
			// A candidate just beyond the true gamut edge can be clipped directly.
			const near = [l, 0.02 + 0.001 * h, h];
			const nearRef = verify(near);
			if (nearRef.reason === "initial-clip") {
				initial++;
				const clipped = referenceP3(near).map(v => Math.max(0, Math.min(1, v)));
				assert.deepEqual(nearRef.rgb, clipped);
			}
		}
	}
	assert.ok(initial > 0);
	assert.ok(aboveJnd > 0);
});

test("css-minde shared Rust fixtures match the independent spec reference", () => {
	const csv = readFileSync(new URL("./fixtures/css-minde.csv", import.meta.url), "utf8");
	const reasons = new Set();
	for (const line of csv.trim().split("\n").filter(line => !line.startsWith("#"))) {
		const fields = line.split(","), input = fields.slice(0, 3).map(Number);
		const ref = verify(input);
		assert.equal(ref.reason, fields[6]);
		reasons.add(ref.reason);
		for (let i = 0; i < 3; i++) assert.ok(Math.abs(ref.rgb[i] - Number(fields[i + 3])) < 2e-12);
	}
	assert.deepEqual([...reasons].sort(), ["black", "close-enough", "in-gamut", "initial-clip", "interval", "white"]);
});
