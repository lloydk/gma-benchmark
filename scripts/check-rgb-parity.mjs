// Compare actual Rust f64 kernels with JavaScript. This complements the
// independent XYZ reference tests; sharing profiles is not an independent oracle.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { createClip } from "../src/clip.js";
import { createCssMinde } from "../src/css-minde.js";
import { RGB_SPACES } from "../src/rgb-spaces.js";
import { createReference } from "../tests/helpers/css-minde-reference.js";
const samples = [];
for (let l = 1; l < 100; l++) for (let h = 0; h < 360; h++) samples.push([l / 100, .4, h]);
for (let i = 0; i < 8192; i++) samples.push([.001 + .998 * ((i * .7548776662466927) % 1), .5 * ((i * .5698402909980532) % 1), (i * 137.50776405003785) % 1080 - 360]);
for (const l of [-.1, 0, 1e-12, .1, .414, .49, .9, 1 - 2 ** -30, 1, 1.1]) for (const c of [-.1, 0, .02, .4]) for (const h of [-360, -114.9, -95.9, 0, 104, 245.1, 264.05, 360, 720]) samples.push([l,c,h]);
const root = fileURLToPath(new URL("../", import.meta.url));
const report = [];
for (const space of Object.values(RGB_SPACES)) {
	const input = samples.map(v => `${space.id},${v.join(",")}`).join("\n") + "\n";
	const lines = execFileSync("cargo", ["run", "--quiet", "--release", "--manifest-path", "rust/Cargo.toml", "--example", "rgb-mapping-probes"], { cwd: root, input, encoding: "utf8", maxBuffer: 32 * 1024 * 1024 }).trim().split("\n");
	assert.equal(lines.length, samples.length);
	const ref = createReference(space.id);
	const maps = [createClip(space), createCssMinde(space)];
	const maxima = maps.map(() => ({ linear: 0, delta: 0, encoded: 0 }));
	for (let i = 0; i < samples.length; i++) {
		const rust = JSON.parse(lines[i]);
		for (let m = 0; m < 2; m++) {
			const js = maps[m](samples[i], []), expected = rust[m];
			const linear = Math.max(...js.map((v,c) => Math.abs(ref.decode(v) - ref.decode(expected[c]))));
			const lab = ref.lab(js), wanted = ref.lab(expected);
			const delta = Math.hypot(...lab.map((v,c) => v - wanted[c]));
			assert.ok(js.every(Number.isFinite) && expected.every(Number.isFinite) && linear <= 2e-11 && delta <= 2e-11, `${space.id} method ${m} ${samples[i]}: linear ${linear}, delta ${delta}`);
			maxima[m].linear = Math.max(maxima[m].linear, linear);
			maxima[m].delta = Math.max(maxima[m].delta, delta);
			maxima[m].encoded = Math.max(maxima[m].encoded, ...js.map((v,c) => Math.abs(v - expected[c])));
		}
	}
	report.push({ gamut: space.id, samples: samples.length, clip: maxima[0], cssMinde: maxima[1] });
}
console.log(JSON.stringify({ runtime: process.versions, rustF64: report }, null, 2));
