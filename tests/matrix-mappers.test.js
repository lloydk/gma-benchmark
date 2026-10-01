import assert from "node:assert/strict";
import { test } from "node:test";
import { RGB_SPACES } from "../src/rgb-spaces.js";
import { createMatrixMappers } from "../src/matrix-mappers.js";
import { createRgbConversions } from "../src/rgb-convert.js";
import { blueFoldWindow, inBlueFold, evaluateFoldPolynomial } from "../src/matrix-solver-policy.js";
import { firstRoot, firstFaceRoot } from "../src/polynomial.js";
import { matrixSamples } from "./helpers/matrix-samples.js";
import { validateMatrixMethods } from "./helpers/validate-matrix-methods.js";
import { createBoundaryReference } from "./helpers/matrix-reference.js";
const samples = matrixSamples();
for (const space of Object.values(RGB_SPACES)) {
	test(`${space.id}: matrix solvers match independent XYZ geometry and Raytrace policy`, () => {
		for (const checked of [false,true]) console.log(JSON.stringify(validateMatrixMethods(space,[samples],checked)));
	});
	test(`${space.id}: canonical gray/pass-through, aliasing and authored hue`, () => {
		const maps = Object.values(createMatrixMappers(space)), conversion = createRgbConversions(space);
		let inside = 0;
		for (const input of samples.slice(35640,36000).concat([[.5,0,30],[.5,-.1,264.05],[.5,0,-720]])) {
			const canonical = [];
			const preserve = input[1] >= 0 && conversion.oklchToRgbIfInGamut(...input,canonical);
			if (preserve) inside++;
			for (const map of maps) for (const checked of [false,true]) {
				const expected = map(input,[],checked), alias = [...input];
				assert.equal(map(alias,alias,checked),alias);
				assert.deepEqual(alias,expected);
				if (preserve && checked) assert.deepEqual(expected,canonical);
				if (input[1] <= 0) assert.deepEqual(expected,conversion.oklchToClippedRgb(input[0],0,input[2],[]));
			}
		}
		assert.ok(inside > 20);
	});
	test(`${space.id}: fold membership edges and negative representations`, () => {
		const window = blueFoldWindow(space);
		if (!window) { assert.equal(inBlueFold(264.1,window),false); return; }
		for (const h of [window[0],window[1],(window[0]+window[1])/2]) {
			assert.ok(inBlueFold(h,window)); assert.ok(inBlueFold(h-360,window));
		}
		const edges = [];
		for (const edge of window) for (const delta of [-2e-12,0,2e-12]) for (const turns of [-1,0,1]) edges.push([.414,.4,edge+delta+turns*360]);
		validateMatrixMethods(space,[edges]);
	});
}

test("target caches remain independent when factories are interleaved", () => {
 const groups = Object.values(RGB_SPACES).map(space => ({ space, maps: createMatrixMappers(space), ref: createBoundaryReference(space.id) }));
 for (const H of [30,80.12,150,245.07,264.1,301.23]) for (const L of [.2,.5,.8]) {
  const hue = Math.round(H*10)/10;
  // Reverse target order on the second pass so a single shared table cannot
  // appear correct merely because it was prefilled in the same test order.
  for (const order of [groups,[...groups].reverse()]) for (const {space,maps,ref} of order) {
   const input = [L,.4,H], C = Math.min(.4,ref.boundaries(L,hue).first);
   const expected = ref.linearRgb([L,C,hue]);
   const actual = maps["oklch-cubic"](input,[]).map(ref.decode);
   assert.ok(Math.max(...actual.map((v,i) => Math.abs(v-expected[i]))) < 1e-6, `${space.id} ${input}`);
  }
 }
});

test("face identity is explicit and generic zero roots do not imply an upper face", () => {
	assert.equal(firstRoot(0,1,-1,0,0,2),1);
	assert.equal(firstFaceRoot(0,1,-1,0,2,true),1);
	assert.equal(firstFaceRoot(0,1,-1,0,2,false),0);
});

test("sRGB corner reproducer reaches the feasible face, not the active-channel switch", () => {
	const input = [.414,.4,264.0425], space = RGB_SPACES.srgb;
	const ref = createBoundaryReference(space.id), boundary = ref.boundaries(input[0],input[2]);
	assert.ok(boundary.outer > .24 && boundary.outer < .25);
	for (const method of ["oklch-halley","oklch-ostrowski"]) {
		const actual = createMatrixMappers(space)[method](input,[]), lab = ref.lab(actual);
		assert.ok(Math.abs(Math.hypot(lab[1],lab[2])-boundary.outer) < 2e-8);
	}
});

// A saturated near-white result can still be finite and inside the RGB cube.
// Check the physical outcome independently of the reference iteration.
test("Raytrace retains near-white hits across hue and lightness scales", () => {
 for (const space of Object.values(RGB_SPACES)) {
  const map = createMatrixMappers(space).raytrace;
  for (let k = 24; k <= 53; k++) for (let h = 0; h < 360; h += .125) {
   const input = [1-2**-k,.4,h], actual = map(input,[]);
   assert.ok(actual.every(v => Number.isFinite(v) && v > .99999 && v <= 1), `${space.id} ${input}: ${actual}`);
  }
 }
});

test("compensated fold evaluation retains cancellation residuals", () => {
 // (x-1)^3 at exactly representable x: the exact result is 2^(-3*k).
 // Ordinary Horner rounds these to zero; this expectation is algebraic.
 for (const k of [18,20,24]) {
  const x = 1+2**-k;
  assert.equal(evaluateFoldPolynomial([1,-3,3,-1],x),2**(-3*k));
 }
});

test("fold-gap mapping reduces chroma without clipping lightness or hue", () => {
 for (const [space,L,H,lo,hi] of [[RGB_SPACES.srgb,.3,264.053,.17,.215],[RGB_SPACES.rec2020,.2,245.067,.15,.185]]) {
  const ref = createBoundaryReference(space.id), edges = ref.boundaries(L,H), maps = createMatrixMappers(space);
  let gap = 0, reentry = 0;
  const chromas = Array.from({length:301}, (_,i) => lo+(hi-lo)*i/300).concat([edges.outer-1e-7]);
  for (const C of chromas) {
   const input = [L,C,H], raw = ref.linearRgb(input);
   const expectedC = ref.foldChroma(L,C,H,edges), expected = ref.linearRgb([L,expectedC,H]);
   assert.ok(expected.every(v => v >= -2e-14 && v <= 1+2e-14));
   const inside = raw.every(v => v >= 0 && v <= 1);
   if (!inside && C < edges.outer) { gap++; assert.ok(expectedC < C); }
   if (inside && C > edges.first) reentry++;
   for (const name of ["oklch-halley","oklch-ostrowski"]) for (const checked of [false,true]) {
    const rgb = maps[name](input,[],checked), lab = ref.lab(rgb), rad = H*Math.PI/180;
    const wantedLab = [L,expectedC*Math.cos(rad),expectedC*Math.sin(rad)];
    assert.ok(Math.max(...lab.map((v,j) => Math.abs(v-wantedLab[j]))) < 2e-10, `${space.id} ${name} ${input}: ${lab} vs ${wantedLab}`);
    assert.ok(Math.hypot(lab[1],lab[2]) <= C+2e-10);
   }
  }
  assert.ok(gap > 0 && reentry > 0);
 }
});

test("validation uses supplied callbacks and named policies even in reverse order", () => {
 const space = RGB_SPACES.srgb;
 const maps = createMatrixMappers(space), probes = [[.3,.177897,264.053],[.5,.01,31.123]];
 const reversed = Object.fromEntries(Object.entries(maps).reverse());
 validateMatrixMethods(space,[probes],true,reversed);
 assert.throws(() => validateMatrixMethods(space,[probes],false,{ ...maps, "oklch-halley": (input,out) => { out[0]=out[1]=out[2]=0; return out; } }), /oklch-halley/);
 const wrongTarget = createMatrixMappers(RGB_SPACES.rec2020);
 assert.throws(() => validateMatrixMethods(space,[probes],false,wrongTarget));
 // A selected checked callback accidentally wired to plain bucketing must fail.
 assert.throws(() => validateMatrixMethods(space,[probes],true,{...maps, "oklch-cubic": (input,out) => maps["oklch-cubic"](input,out,false)}));
});
