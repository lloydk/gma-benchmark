import assert from "node:assert/strict";
import { test } from "node:test";
import { dualray } from "../src/dualray.js";
import { clampedGamma, oklchToClippedP3, KA0, KB0, KA1, KB1, KA2, KB2, RL, RM, RS, GL, GM, GS, BL, BM, BS } from "../src/convert.js";
import { exactBoundary } from "./helpers/first-exit-reference.js";

function verify (l, h) {
	const boundaryOut = [];
	const edge = exactBoundary(l, h, boundaryOut);
	let max = 0;
	for (const c of [edge * 0.5, edge * (1 - 1e-7), edge, edge * (1 + 1e-7), 0.4, 0.6]) {
		const input = [l, c, h];
		const expected = c < edge ? oklchToClippedP3(l, c, h, []) : boundaryOut;
		const out = [];
		assert.equal(dualray(input, out), out);
		// The checked benchmark row calls this same function.
		const checked = dualray(input, [], true);
		assert.deepEqual(out, checked);
		for (let i = 0; i < 3; i++) {
			assert.ok(Number.isFinite(out[i]) && out[i] >= 0 && out[i] <= 1, `${input}: ${out}`);
			const error = Math.abs(out[i] - expected[i]);
			max = Math.max(max, error);
			assert.ok(error <= 1e-8, `${input}: ${out}, expected ${expected}, error ${error}`);
		}
	}
	return max;
}

test("dualray matches an independent boundary oracle inside and outside the gamut", () => {
	let max = 0;
	for (let li = 1; li < 100; li++) {
		const offset = (li * 0.6180339887498949) % 1;
		for (let h = 0; h < 360; h++) {
			max = Math.max(max, verify(li / 100, h), verify(li / 100, h + offset));
		}
	}
	console.log(`dualray boundary oracle max channel error: ${max}`);
});

test("dualray retains guarded upper-first solving and upper-face recovery", () => {
	let max = 0;
	for (const [minL, minH, maxH] of [[700, 32800, 33300], [850, 19200, 19700], [950, 10600, 11200]]) {
		for (let li = minL; li < 1000; li += 5) {
			for (let hi = minH; hi <= maxH; hi += 2) max = Math.max(max, verify(li / 1000, hi / 100));
		}
	}
	// Gate positions from the independent LMS matrices, not the fitted basis.
	for (let hi = 0; hi <= 720; hi++) {
		const h = hi / 2, angle = h * Math.PI / 180;
		const q = [[KA0, KB0], [KA1, KB1], [KA2, KB2]].map(([a, b]) => a * Math.cos(angle) + b * Math.sin(angle));
		const slope = Math.max(...[[RL, RM, RS], [GL, GM, GS], [BL, BM, BS]].map(row => 3 * row.reduce((s, v, i) => s + v * q[i], 0)));
		const gate = 1 / Math.cbrt(1 + 0.15 * slope);
		for (const l of [gate - 1e-12, gate, gate + 1e-12, 0.9999, 1 - 1e-10]) max = Math.max(max, verify(l, h));
	}
	max = Math.max(max, verify(0.25669940977808137, -1562.0080463960767));
	console.log(`dualray handoff oracle max channel error: ${max}`);
});

test("dualray preserves endpoints, achromatic inputs, hue reduction and aliased buffers", () => {
	for (const l of [-0.1, 0, 1, 1.1]) assert.deepEqual(dualray([l, 0.4, 20], []), Array(3).fill(l <= 0 ? 0 : 1));
	for (const c of [-0.1, 0]) assert.deepEqual(dualray([0.5, c, 20], []), Array(3).fill(clampedGamma(0.125)));
	for (const h of [-Number.MAX_VALUE, -1e21, -1e9, 1e9, 1e21, Number.MAX_VALUE]) {
		for (const l of [0.01, 0.5, 0.99]) {
			for (const c of [0.001, 0.1, 0.4]) assert.deepEqual(dualray([l, c, h], []), dualray([l, c, h % 360], []));
		}
	}
	for (const l of [0.01, 0.5, 0.95, 0.9999]) {
		for (const c of [0.001, 0.1, 0.4]) {
			for (const h of [29, 96.03, 145.65, 194.2, 264.05, 330.2]) {
				const input = [l, c, h], expected = dualray(input, []);
				assert.equal(dualray(input, input), input);
				assert.deepEqual(input, expected);
			}
		}
	}
});
