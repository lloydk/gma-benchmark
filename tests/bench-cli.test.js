import assert from "node:assert/strict";
import { test } from "node:test";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { tmpdir } from "node:os";
const script = fileURLToPath(new URL("../bench.js", import.meta.url));
const run = args => {
	const result = spawnSync(process.execPath, [script, ...args], {
		encoding: "utf8", cwd: tmpdir(), timeout: 120_000,
		env: { ...process.env, NO_COLOR: "1" }, maxBuffer: 4*1024*1024,
	});
	assert.ifError(result.error);
	return result;
};

test("benchmark CLI rejects missing/unsupported gamuts and conflicting modes", () => {
	for (const args of [["--gamut"], ["--gamut", "--validate-only"], ["--gamut="], ["--gamut", "xyz"], ["--validate-only", "--timing-only"]]) {
		assert.notEqual(run(args).status, 0, args.join(" "));
	}
	const help = run(["--help"]);
	assert.equal(help.status, 0);
	assert.match(help.stdout, /all 13 methods/);
	assert.match(help.stdout, /sRGB and Rec.2020: 12 methods/);
});

// Run the real validation -> timing child flow, from an unrelated working
// directory. Dropping --gamut from either child's arguments must fail this.
for (const gamut of ["srgb","rec2020"]) test(`CLI forwards ${gamut} to validation and timing children`, { timeout: 120_000 }, () => {
	const checked = gamut === "rec2020";
	const result = run(["--gamut",gamut,"--warmup","1", ...(checked ? ["--in-gamut-check"] : [])]);
	assert.equal(result.status,0,result.stderr+result.stdout);
	assert.deepEqual([...result.stdout.matchAll(/^gamut: (.*)$/gm)].map(m => m[1]),[gamut,gamut,gamut]);
	assert.ok(result.stdout.includes(`${gamut} / oklch-halley`));
	assert.ok(result.stdout.includes(`${gamut} / raytrace`));
	assert.ok(result.stdout.includes(`${gamut} / bottosson-lightness (cached)`));
	assert.ok(result.stdout.includes(`${gamut} / edge-seeker (indexed)`));
	assert.doesNotMatch(result.stdout,/display-p3 \/|dualray/);
	assert.match(result.stdout,/benchmark checksum: /);
	assert.equal([...result.stdout.matchAll(/in-gamut precheck: ENABLED/g)].length,checked ? 2 : 0);
});
