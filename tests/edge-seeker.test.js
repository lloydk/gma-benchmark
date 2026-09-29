import assert from "node:assert/strict";
import { test } from "node:test";
import { intersectionWithArc, makeEdgeSeeker, makeEdgeSeekerIndexed } from "../src/edge-seeker/makeEdgeSeeker.js";
import { makeLut } from "../src/edge-seeker/makeLut.js";
import { edgeSeeker, edgeSeekerIndexed } from "../src/edge-seeker/index.js";
import { oklchToClippedP3, p3ToOklch } from "../src/convert.js";

const lut = makeLut(p3ToOklch, 400);
const seekers = [makeEdgeSeeker(p3ToOklch), makeEdgeSeekerIndexed(p3ToOklch)];
const storage = new DataView(new ArrayBuffer(8));
function adjacentPositive (x, direction) {
	storage.setFloat64(0, x);
	storage.setBigUint64(0, storage.getBigUint64(0) + BigInt(direction));
	return storage.getFloat64(0);
}

function itemAt (h) {
	h %= 360;
	const hi = lut.findIndex(item => item.h >= h);
	if (lut[hi].h === h) return lut[hi];
	const a = lut[hi - 1], b = lut[hi];
	const t = (h - a.h) / (b.h - a.h);
	const lerp = key => a[key] * (1 - t) + b[key] * t;
	return { l: lerp("l"), c: lerp("c"), curvature: lerp("curvature") };
}

// Independent root search of the scaled circle residual, monotone in y for
// |k| < 1. It does not evaluate the production quadratic formula.
function arcOracle (x, k) {
	const t = Math.sqrt(2 - k * k);
	let lo = 0, hi = 1;
	for (let i = 0; i < 60; i++) {
		const y = (lo + hi) / 2;
		if (y === lo || y === hi) break;
		const residual = k * (x * x + y * y - x - y) + t * (y - x);
		if (residual < 0) lo = y;
		else hi = y;
	}
	return (lo + hi) / 2;
}

test("arc stays on the intended branch at endpoints and near zero curvature", () => {
	for (const k of [-0.54, -0.2, -1e-6, -1e-12, -1e-15, 0, 1e-15, 1e-12, 1e-6, 0.14]) {
		for (const x of [0, Number.MIN_VALUE, Number.EPSILON, 0.1, 0.5, 0.9, adjacentPositive(1, -1), 1]) {
			const y = intersectionWithArc(x, k);
			assert.ok(Number.isFinite(y) && y >= 0 && y <= 1, `${x}, ${k}: ${y}`);
			assert.ok(Math.abs(y - arcOracle(x, k)) <= 2e-15, `${x}, ${k}: ${y}`);
			if (k === 0) assert.equal(y, x);
		}
	}
	assert.equal(intersectionWithArc(-0, 0), -0);
});

test("near-cusp yellow does not become magenta in either mapper or mode", () => {
	const input = [0.8938885222211185, 0.4, 96.03];
	const item = itemAt(input[2]);
	const expectedChroma = item.c * arcOracle((1 - input[0]) / (1 - item.l), item.curvature);
	const expected = oklchToClippedP3(input[0], expectedChroma, input[2], []);
	assert.ok(expected[0] > 0.99 && expected[1] > 0.8 && expected[2] < 0.01);
	for (const seek of seekers) {
		assert.ok(Math.abs(seek(input[0], input[2]) - expectedChroma) <= 2e-14);
	}
	for (const map of [edgeSeeker, edgeSeekerIndexed]) {
		for (const checked of [false, true]) {
			const actual = map(input, [], checked);
			assert.ok(actual.every((v, i) => Math.abs(v - expected[i]) <= 5e-13), `${actual}`);
		}
	}
});

test("both LUT lookups match the arc oracle at cusp and white neighbours", () => {
	// Eight representable neighbours of each endpoint, at every 0.01 degree.
	for (let n = 0; n <= 36000; n++) {
		const h = n / 100, item = itemAt(h);
		assert.ok(Math.abs(item.curvature) < 1);
		let cusp = item.l, white = 1;
		for (let step = 0; step < 8; step++) {
			cusp = adjacentPositive(cusp, 1);
			white = adjacentPositive(white, -1);
			for (const l of [cusp, white]) {
				const expected = item.c * arcOracle((1 - l) / (1 - item.l), item.curvature);
				const a = seekers[0](l, h), b = seekers[1](l, h);
				assert.ok(Number.isFinite(a) && a >= 0 && a <= item.c, `${l}, ${h}: ${a}`);
				assert.equal(a, b);
				assert.ok(Math.abs(a - expected) <= 2e-14, `${l}, ${h}: ${a}, ${expected}`);
			}
		}
	}
});
