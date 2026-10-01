// Dedicated latency/throughput probe for the rare fold path. The ordinary
// integer-hue grid never enters these windows. Compare policies via the
// accuracy suite, not by expecting the old clipped-gap checksums to match.
import { parseArgs } from "node:util";
import { createHash } from "node:crypto";
import { getRgbSpace } from "../src/rgb-spaces.js";
import { blueFoldWindow } from "../src/matrix-solver-policy.js";
const { values } = parseArgs({ options: {
	gamut: { type: "string", default: "srgb" },
	method: { type: "string", default: "oklch-halley" },
} });
const factory = { "oklch-halley": "createOklchHalley", "oklch-ostrowski": "createOklchOstrowski" }[values.method];
if (!factory) throw new RangeError("Expected Halley or Ostrowski");
const space = getRgbSpace(values.gamut), window = blueFoldWindow(space);
if (!window) throw new RangeError("Target has no blue-fold window");
const map = (await import(`../src/${values.method}.js`))[factory](space);
const rows = [];
for (const [workload,chroma] of [["interior",.01],["mixed",.18],["exterior",.4]]) {
	const inputs = [];
	for (const l of [.1,.3,.5,.8]) for (let i = 0; i < 32; i++) inputs.push([l,chroma,window[0]+(i+.5)*(window[1]-window[0])/32]);
	const out = [0,0,0];
	const batch = () => {
		let sum = 0;
		for (const input of inputs) { map(input,out); sum += out[0]+out[1]+out[2]; }
		return sum;
	};
	let checksum = 0;
	for (let i = 0; i < 50; i++) checksum += batch();
	const passes = [];
	for (let i = 0; i < 25; i++) {
		const start = process.hrtime.bigint(); checksum += batch();
		passes.push(Number(process.hrtime.bigint()-start)/inputs.length);
	}
	if (!Number.isFinite(checksum)) throw new Error("Nonfinite checksum");
	rows.push({ workload, count: inputs.length, inputSha256: createHash("sha256").update(JSON.stringify(inputs)).digest("hex"), ns: [...passes].sort((a,b) => a-b)[12], passes, checksum });
}
console.log(JSON.stringify({ runtime: process.versions, gamut: space.id, method: values.method, warmup: 50, measured: 25, rows }));
