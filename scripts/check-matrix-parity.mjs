// Production Rust f64 parity supplements the independent boundary oracle.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { createHash } from "node:crypto";
import { createMatrixMappers } from "../src/matrix-mappers.js";
import { RGB_SPACES } from "../src/rgb-spaces.js";
import { createReference } from "../tests/helpers/css-minde-reference.js";
import { matrixSamples } from "../tests/helpers/matrix-samples.js";
const samples = matrixSamples();
const root = fileURLToPath(new URL("../", import.meta.url));
const report = [];
for (const space of Object.values(RGB_SPACES)) {
	const input = samples.map(v => v.join(",")).join("\n") + "\n";
	const lines = execFileSync("cargo", ["run", "--quiet", "--release", "--manifest-path", "rust/Cargo.toml", "--example", "matrix-mapping-probes", "--", space.id], { cwd: root, input, encoding: "utf8", maxBuffer: 100*1024*1024 }).trim().split("\n");
	assert.equal(lines.length, samples.length);
	const ref = createReference(space.id), maps = Object.entries(createMatrixMappers(space));
	const maxima = maps.map(([method]) => ({ method, linear: 0, delta: 0, encoded: 0 }));
	for (let i = 0; i < samples.length; i++) {
		const rust = JSON.parse(lines[i]);
		assert.deepEqual(Object.keys(rust).sort(), maps.map(([name]) => name).sort());
		for (let m = 0; m < maps.length; m++) for (const checked of [false,true]) {
			const js = maps[m][1](samples[i], [], checked), expected = rust[maps[m][0]][checked ? "checked" : "plain"];
			const linear = Math.max(...js.map((v,c) => Math.abs(ref.decode(v) - ref.decode(expected[c]))));
			const lab = ref.lab(js), wanted = ref.lab(expected);
			const delta = Math.hypot(...lab.map((v,c) => v - wanted[c]));
			assert.ok(js.every(Number.isFinite) && expected.every(Number.isFinite) && linear <= 2e-8 && delta <= 2e-8, `${space.id} ${maps[m][0]} checked=${checked} ${samples[i]}: linear ${linear}, delta ${delta}; ${js} vs ${expected}`);
			const max = maxima[m];
			if (linear > max.linear) { max.linear = linear; max.input = samples[i]; }
			max.delta = Math.max(max.delta, delta);
			max.encoded = Math.max(max.encoded, ...js.map((v,c) => Math.abs(v - expected[c])));
		}
	}
	report.push({ gamut: space.id, samples: samples.length, maxima });
}
console.log(JSON.stringify({ runtime: process.versions, inputSha256: createHash("sha256").update(JSON.stringify(samples)).digest("hex"), rustF64: report }, null, 2));
