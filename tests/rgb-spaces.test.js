import assert from "node:assert/strict";
import { test } from "node:test";
import { SRGB, DISPLAY_P3, REC2020, RGB_SPACES, getRgbSpace } from "../src/rgb-spaces.js";
import { createRgbConversions, getRgbConversions } from "../src/rgb-convert.js";
import { createClip, clip } from "../src/clip.js";
import { createCssMinde, cssMinde } from "../src/css-minde.js";
import { createReference } from "./helpers/css-minde-reference.js";
import { validateRgbMethods } from "./helpers/validate-rgb-methods.js";

const grid = [], mixed = [];
for (let l = 1; l < 100; l++) for (let h = 0; h < 360; h++) grid.push([l / 100, 0.4, h]);
for (let i = 0; i < 8192; i++) mixed.push([0.001 + 0.998 * ((i * 0.7548776662466927) % 1), 0.5 * ((i * 0.5698402909980532) % 1), (i * 137.50776405003785) % 1080 - 360]);
for (const l of [0, 1e-12, 0.1, 0.414, 0.49, 0.9, 1 - 2 ** -30, 1]) {
	for (const c of [0, 1e-10, 0.02, 0.4]) for (const h of [-360, -114.9, -95.9, 0, 104, 245.1, 264.05, 360, 720]) mixed.push([l, c, h]);
}

for (const space of Object.values(RGB_SPACES)) {
	test(`${space.id}: Clip and MINDE match independent XYZ reference`, () => {
		console.log(JSON.stringify(validateRgbMethods(space, [grid, mixed])));
	});
	test(`${space.id}: canonical pass-through, inverse conversion, endpoints and aliasing`, () => {
		const conversion = createRgbConversions(space), min = createCssMinde(space), clip = createClip(space), ref = createReference(space.id);
		let inside = 0;
		for (const input of mixed) {
			const canonical = [], actual = min(input, []);
			if (input[0] > 0 && input[0] < 1 && conversion.oklchToRgbIfInGamut(...input, canonical)) {
				assert.deepEqual(actual, canonical);
				inside++;
			}
			for (const map of [min, clip]) {
				const expected = map(input, []), alias = [...input];
				assert.equal(map(alias, alias), alias);
				assert.deepEqual(alias, expected);
			}
		}
		assert.ok(inside > 1000);
		for (const rgb of [[0,0,0],[1,1,1],[1,0,0],[0,1,0],[0,0,1],[0.2,0.4,0.7],[-0.1,0.1,1.2]]) {
			const { l, c, h } = conversion.rgbToOklch(...rgb), lab = ref.lab(rgb);
			const actual = [l, c * Math.cos(h * Math.PI / 180), c * Math.sin(h * Math.PI / 180)];
			assert.ok(Math.max(...actual.map((v,i) => Math.abs(v-lab[i]))) < 2e-14);
		}
		for (const l of [-1, 0, 1, 2]) assert.deepEqual(min([l, .4, 30], []), Array(3).fill(l <= 0 ? 0 : 1));
		for (const c of [-.1, 0]) assert.deepEqual(min([.5, c, NaN], []), clip([.5, 0, 0], []));
		for (const h of [-Number.MAX_VALUE, -1e21, -1e9, 1e9, 1e21, Number.MAX_VALUE]) assert.deepEqual(min([.5,.4,h], []), min([.5,.4,h % 360], []));
		const sentinel = [7,8,9];
		assert.equal(conversion.oklchToRgbIfInGamut(.5, .4, NaN, sentinel), false);
		assert.deepEqual(sentinel, [7,8,9]);
	});
}

test("immutable physical descriptors, signed transfers and CSS Rec.2020 gamma", () => {
	assert.deepEqual(Object.keys(RGB_SPACES), ["srgb", "display-p3", "rec2020"]);
	for (const id of ["p3", "xyz", "all", "toString", "__proto__"]) assert.throws(() => getRgbSpace(id), RangeError);
	assert.throws(() => { SRGB.lmsToRgb[0][0] = 0; }, TypeError);
	assert.equal(SRGB.transfer, DISPLAY_P3.transfer);
	assert.equal(REC2020.transfer.encode(.25), .25 ** (1 / 2.4));
	for (const space of Object.values(RGB_SPACES)) {
		assert.equal(getRgbSpace(space.id), space);
		let previous = -Infinity;
		for (const x of [-2,-1,-.1,0,.001,.0031308,.1,.5,1,2]) {
			const encoded = space.transfer.encode(x);
			assert.ok(encoded >= previous); previous = encoded;
			assert.ok(Math.abs(space.transfer.decode(encoded) - x) < 1e-14);
			assert.ok(Math.abs(space.transfer.encodeClamped(x) - space.transfer.encode(Math.max(0, Math.min(1, x)))) < 1e-15);
		}
	}
});

test("interleaved target factories remain independent and P3 aliases agree", () => {
	const maps = Object.values(RGB_SPACES).map(space => [createClip(space), createCssMinde(space)]);
	const samples = mixed.slice(0,128);
	const expected = maps.map(pair => pair.map(map => samples.map(input => map(input, []))));
	for (let i = 0; i < samples.length; i++) for (let g = 0; g < maps.length; g++) for (let m = 0; m < 2; m++) {
		assert.deepEqual(maps[g][m](samples[i], []), expected[g][m][i]);
	}
	for (const input of samples) {
		assert.deepEqual(clip(input, []), maps[1][0](input, []));
		assert.deepEqual(cssMinde(input, []), maps[1][1](input, []));
	}
});

test("conversion lookup shares immutable kernels by descriptor identity", () => {
 const instances = Object.values(RGB_SPACES).map(getRgbConversions);
 assert.equal(new Set(instances).size,3);
 for (const [i,space] of Object.values(RGB_SPACES).entries()) {
  assert.equal(getRgbConversions(space),instances[i]);
  assert.ok(Object.isFrozen(instances[i]));
  assert.notEqual(getRgbConversions({...space}),instances[i]);
 }
});

test("MINDE input normalization is checked independently of the search reference", () => {
 for (const space of Object.values(RGB_SPACES)) {
  const map = createCssMinde(space), ref = createReference(space.id);
  assert.throws(() => ref.cssMinde([.5,-.1,30]),RangeError);
  assert.throws(() => ref.cssMinde([.5,.4,1e21]),RangeError);
  for (const c of [-.4,-.01,0]) for (const h of [NaN,-1e21,33,1e21]) {
   const expected = ref.encoded([.5,0,0]);
   const actual = map([.5,c,h],[]);
   assert.ok(actual.every((v,i) => Math.abs(v-expected[i]) < 2e-14));
  }
  for (const h of [-Number.MAX_VALUE,-1e21,-1e9,1e9,1e21,Number.MAX_VALUE]) {
   // These doubles are integers. BigInt computes their exact modulo, without
   // reusing the production Number remainder or its normalization branch.
   const reduced = Number(BigInt(h)%360n);
   const expected = ref.cssMinde([.5,.4,reduced]).rgb, actual = map([.5,.4,h],[]);
   assert.ok(actual.every((v,i) => Math.abs(v-expected[i]) < 2e-12));
  }
 }
});
