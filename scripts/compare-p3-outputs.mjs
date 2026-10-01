// Separate from timing: compare the archived P3 exports on the timed inputs.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { pathToFileURL } from "node:url";
import { resolve } from "node:path";

const [before, after] = process.argv.slice(2);
const load = (root, path) => import(pathToFileURL(resolve(root, path)).href);
const { buildWorkloads } = await load(after, "benchmark-workloads.js");
const { samples, randomSamples } = buildWorkloads();
const entries = [
  ["clip", "clip"], ["css-minde", "cssMinde"],
  ["oklch-cubic", "oklchCubic"], ["oklch-cubic-no-cache", "oklchCubicNoCache"],
  ["oklch-cubic-direct", "oklchCubicDirect"], ["oklch-halley", "oklchHalley"],
  ["oklch-ostrowski", "oklchOstrowski"], ["dualray", "dualray"],
  ["bottosson-lightness", "bottossonLightness"],
  ["bottosson-lightness-cached", "bottossonLightnessCached", "bottosson-lightness"],
  ["edge-seeker", "edgeSeeker", "edge-seeker/index"],
  ["edge-seeker-indexed", "edgeSeekerIndexed", "edge-seeker/index"],
  ["raytrace", "raytrace"],
];
const rows = [];
for (const [method, name, module = method] of entries) {
  const oldMap = (await load(before, `src/${module}.js`))[name];
  const newMap = (await load(after, `src/${module}.js`))[name];
  for (const checked of [false, true]) {
    for (const [workload, inputs] of [["grid", samples], ["random", randomSamples]]) {
      const a = [0, 0, 0], b = [0, 0, 0];
      const hashes = [createHash("sha256"), createHash("sha256")];
      const buffers = [Buffer.alloc(24), Buffer.alloc(24)];
      let changedInputs = 0, maxEncoded = 0, worstInput = null;
      let beforeChecksum = 0, afterChecksum = 0;
      for (const input of inputs) {
        a.fill(NaN); b.fill(NaN);
        assert.equal(oldMap(input, a, checked), a, `${method}: baseline output identity`);
        assert.equal(newMap(input, b, checked), b, `${method}: current output identity`);
        let changed = false;
        for (let i = 0; i < 3; i++) {
          assert(Number.isFinite(a[i]) && Number.isFinite(b[i]), `${method}: non-finite output`);
          assert(a[i] >= 0 && a[i] <= 1 && b[i] >= 0 && b[i] <= 1, `${method}: output outside gamut`);
          changed ||= !Object.is(a[i], b[i]);
          const error = Math.abs(a[i] - b[i]);
          if (error > maxEncoded) { maxEncoded = error; worstInput = input; }
          buffers[0].writeDoubleLE(a[i], i * 8);
          buffers[1].writeDoubleLE(b[i], i * 8);
        }
        hashes.forEach((hash, i) => hash.update(buffers[i]));
        changedInputs += Number(changed);
        beforeChecksum += a[0] + a[1] + a[2];
        afterChecksum += b[0] + b[1] + b[2];
      }
      rows.push({ method, checked, workload, count: inputs.length,
        inputSha256: createHash("sha256").update(JSON.stringify(inputs)).digest("hex"),
        changedInputs, maxEncoded, worstInput, beforeChecksum, afterChecksum,
        beforeOutputSha256: hashes[0].digest("hex"), afterOutputSha256: hashes[1].digest("hex") });
    }
  }
}
console.log(JSON.stringify({ runtime: process.versions, rows }));
