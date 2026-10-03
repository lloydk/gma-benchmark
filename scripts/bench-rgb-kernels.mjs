import { buildWorkloads } from "../benchmark-workloads.js";
// Controlled, output-consuming companion to the full Mitata harness.
// One target/method per process keeps JIT call sites monomorphic.
import { parseArgs } from "node:util";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
const { values } = parseArgs({ options: {
	gamut: { type: "string", default: "display-p3" },
	method: { type: "string", default: "clip" },
	input: { type: "string" },
	workload: { type: "string", default: "external" },
	"in-gamut-check": { type: "boolean", default: false },
	"validate-only": { type: "boolean", default: false },
} });
const names = {
 "dualray": ["dualray", "createDualray"],
 "dualray-fast": ["dualrayFast", "createDualrayFast"],
 "edge-seeker": ["edgeSeeker", "createEdgeSeeker"],
 "edge-seeker-indexed": ["edgeSeekerIndexed", "createEdgeSeekerIndexed"],
 "bottosson-lightness": ["bottossonLightness", "createBottossonLightness"],
 "bottosson-lightness-cached": ["bottossonLightnessCached", "createBottossonLightnessCached"],
 "clip": ["clip", "createClip"], "css-minde": ["cssMinde", "createCssMinde"],
 "oklch-cubic": ["oklchCubic", "createOklchCubic"],
 "oklch-cubic-no-cache": ["oklchCubicNoCache", "createOklchCubicNoCache"],
 "oklch-cubic-direct": ["oklchCubicDirect", "createOklchCubicDirect"],
 "oklch-halley": ["oklchHalley", "createOklchHalley"],
 "oklch-ostrowski": ["oklchOstrowski", "createOklchOstrowski"],
 "raytrace": ["raytrace", "createRaytrace"],
};
if (!Object.hasOwn(names, values.method)) throw new RangeError("unsupported method");
const { getRgbSpace } = await import("../src/rgb-spaces.js");
const space = getRgbSpace(values.gamut);
let moduleName = values.method.startsWith("edge-seeker") ? "edge-seeker/index"
 : values.method === "bottosson-lightness-cached" ? "bottosson-lightness"
 : values.method;
if (values.gamut !== "display-p3") {
 const factoryModules = { dualray: "dualray-factory", "edge-seeker": "edge-seeker/factory",
  "edge-seeker-indexed": "edge-seeker/factory", "bottosson-lightness": "bottosson-factory",
  "bottosson-lightness-cached": "bottosson-factory" };
 moduleName = factoryModules[values.method] ?? moduleName;
}
const module = await import(`../src/${moduleName}.js`);
const [name, factory] = names[values.method];
let map;
// The P3 branch also runs against the pre-factory baseline unchanged; Dualray
// Fast has only its factory.
if (values.gamut === "display-p3" && module[name]) map = module[name];
else {
 map = module[factory](space);
}
if (values["in-gamut-check"] && !["clip", "css-minde", "dualray", "dualray-fast"].includes(values.method)) {
 const unchecked = map;
 map = (input, out) => unchecked(input, out, true);
}
let workloads;
let binaryInputSha256;
if (values.input) {
 const bytes = readFileSync(values.input);
 if (!bytes.length || bytes.length % 24) throw new Error("Expected nonempty little-endian f64 triples");
 const inputs = Array.from({ length: bytes.length / 24 }, (_, i) =>
  Array.from({ length: 3 }, (_, j) => bytes.readDoubleLE(i * 24 + j * 8)));
 if (!inputs.every(row => row.every(Number.isFinite))) throw new Error("Nonfinite input");
 binaryInputSha256 = createHash("sha256").update(bytes).digest("hex");
 workloads = [[values.workload, inputs]];
} else {
 const { samples, randomSamples } = buildWorkloads();
 workloads = [["grid",samples],["random",randomSamples]];
}
if(!values.input && values.method.startsWith("dualray") && values.gamut !== "display-p3") {
 const {blueFoldWindow}=await import("../src/matrix-solver-policy.js");
 const [lo,hi]=blueFoldWindow(space);
 for(const fold of [false,true])for(const interior of [true,false]) {
  const inputs=Array.from({length:4096},(_,i)=>{
   const l=.1+.8*((i*.7548776662466927)%1);
   return [l,interior?.01*l:.6,lo+(hi-lo)*(i+.37)/4096+(fold?0:15)];
  });
  workloads.push([`${fold?'fold':'ordinary'}-${interior?'interior':'mapped'}`,inputs]);
 }
}
const rows = [];
for (const [workload, inputs] of workloads) {
	const out = [0, 0, 0];
	let sink = 0;
	if (values["validate-only"]) {
		for (const input of inputs) {
			if (map(input, out) !== out || !out.every(x => Number.isFinite(x) && x >= 0 && x <= 1)) throw new Error(`Invalid output at ${input}`);
			sink += out[0] + out[1] + out[2];
		}
		rows.push({ workload, count: inputs.length, checksum: sink });
		continue;
	}
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
console.log(JSON.stringify({ runtime: process.versions, gamut: values.gamut, method: values.method,
 checked: values["in-gamut-check"], binaryInputSha256, warmup: 50, measured: 25, rows }));
