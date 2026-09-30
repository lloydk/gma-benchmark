import assert from "node:assert/strict";
import test from "node:test";
import { oklchCubic } from "../src/oklch-cubic.js";
import { oklchCubicNoCache } from "../src/oklch-cubic-no-cache.js";
import { faceExit } from "../src/polynomial.js";
import { oklchCubicDirect } from "../src/oklch-cubic-direct.js";
import { exactBoundary } from "./helpers/first-exit-reference.js";
import { referenceLab } from "./helpers/css-minde-reference.js";

const delta = (a, b) => {
	const left = referenceLab(a), right = referenceLab(b);
	return Math.hypot(...left.map((v, i) => v - right[i]));
};

test("direct cubic retains tiny upper-face roots near white", () => {
	// Includes the formerly blue result at [1 - EPSILON, 0.4, 266], plus
	// adjacent lightness scales and every quarter-degree hue.
	for (let n = 28; n <= 53; n++) {
		for (let h = 0; h < 360; h += 0.25) {
			const input = [1 - 2 ** -n, 0.4, h];
			const output = oklchCubicDirect(input, [0, 0, 0]);
			assert.ok(output.every(v => Number.isFinite(v) && v >= 0 && v <= 1));
			assert.ok(delta(output, [1, 1, 1]) < 2e-8, `${input}: ${output}`);
		}
	}
});

test("Newton refinement cannot jump inward from a nearly double root", () => {
	const fold = 264.53667577034344;
	for (const l of [0.01, 0.1, 0.4, 0.5, 0.9]) {
		for (const dh of [-1e-6, -1e-10, -1e-12, 0, 1e-12, 1e-10, 1e-6]) {
			const input = [l, 0.5, fold + dh];
			const expected = [0, 0, 0];
			exactBoundary(l, input[2], expected);
			for (const checked of [false, true]) {
				const output = oklchCubicDirect(input, [0, 0, 0], checked);
				assert.ok(delta(output, expected) < 1e-8, `${input}: ${output}`);
			}
		}
	}
});

test("all cubic variants retain the first exit across near-white scales", () => {
	for (let n = 14; n <= 53; n++) {
		for (let h = 0; h < 360; h += 0.25) {
			const input = [1 - 2 ** -n, 0.4, h];
			for (const method of [oklchCubic, oklchCubicNoCache, oklchCubicDirect]) {
				const expected = [];
				const hue = method === oklchCubicDirect ? h : Math.round(h * 10) / 10;
				exactBoundary(input[0], hue, expected);
				for (const checked of [false, true]) {
					const actual = method(input, [], checked);
					assert.ok(actual.every((v, i) => Number.isFinite(v) && Math.abs(v - expected[i]) < 2e-12), `${method.name} ${input}: ${actual} vs ${expected}`);
				}
			}
		}
	}
});

test("a face contact only exits when pointing outwards", () => {
	assert.equal(faceExit(0, 1, -1, true, 2), 1);
	assert.equal(faceExit(0, -1, 1, false, 2), 1);
	assert.equal(faceExit(0, 1, 1, true, 2), 0);
	assert.equal(faceExit(0, -1, -1, false, 2), 0);
	assert.equal(faceExit(0, 0, 0, true, 2), Infinity);
});
