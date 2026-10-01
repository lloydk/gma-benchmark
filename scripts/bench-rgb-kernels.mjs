import { buildWorkloads } from "../benchmark-workloads.js";
// Controlled, output-consuming companion to the full Mitata harness.
// One target/method per process keeps JIT call sites monomorphic.
import { parseArgs } from "node:util";
import { createHash } from "node:crypto";
const { values } = parseArgs({ options: {
	gamut: { type: "string", default: "display-p3" },
	method: { type: "string", default: "clip" },
} });
const names = {
 "clip": ["clip", "createClip"], "css-minde": ["cssMinde", "createCssMinde"],
 "oklch-cubic": ["oklchCubic", "createOklchCubic"],
 "oklch-cubic-no-cache": ["oklchCubicNoCache", "createOklchCubicNoCache"],
 "oklch-cubic-direct": ["oklchCubicDirect", "createOklchCubicDirect"],
 "oklch-halley": ["oklchHalley", "createOklchHalley"],
 "oklch-ostrowski": ["oklchOstrowski", "createOklchOstrowski"],
 "raytrace": ["raytrace", "createRaytrace"],
};
if (!Object.hasOwn(names, values.method)) throw new RangeError("unsupported method");
const module = await import(`../src/${values.method}.js`);
const [name, factory] = names[values.method];
let map;
// The P3 branch also runs against the pre-factory baseline unchanged.
if (values.gamut === "display-p3") map = module[name];
else {
 const { getRgbSpace } = await import("../src/rgb-spaces.js");
 map = module[factory](getRgbSpace(values.gamut));
}
const { samples, randomSamples } = buildWorkloads();

const rows = [];
for (const [workload, inputs] of [["grid", samples], ["random", randomSamples]]) {
	const out = [0, 0, 0];
	let sink = 0;
	function batch () {
		let sum = 0;
		for (let i = 0; i < inputs.length; i++) {
			map(inputs[i], out);
			sum += out[0] + out[1] + out[2];
		}
		return sum;
	}
	for (let i = 0; i < 50; i++) sink += batch();
	const passes = [];
	for (let i = 0; i < 25; i++) {
		const start = process.hrtime.bigint();
		sink += batch();
		passes.push(Number(process.hrtime.bigint() - start) / inputs.length);
	}
	if (!Number.isFinite(sink)) throw new Error("non-finite checksum");
	const sorted = [...passes].sort((a,b) => a-b);
	rows.push({ workload, count: inputs.length, inputSha256: createHash("sha256").update(JSON.stringify(inputs)).digest("hex"), ns: sorted[12], passes, checksum: sink });
}
console.log(JSON.stringify({ runtime: process.versions, gamut: values.gamut, method: values.method, warmup: 50, measured: 25, rows }));
