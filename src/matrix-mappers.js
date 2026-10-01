// Setup-time registry shared by the harness and parity tools. No dispatch in
// the mapping loop; each entry is a fixed-target kernel with its own cache.
import { DISPLAY_P3 } from "./rgb-spaces.js";
import { oklchCubic, createOklchCubic } from "./oklch-cubic.js";
import { oklchCubicNoCache, createOklchCubicNoCache } from "./oklch-cubic-no-cache.js";
import { oklchCubicDirect, createOklchCubicDirect } from "./oklch-cubic-direct.js";
import { oklchHalley, createOklchHalley } from "./oklch-halley.js";
import { oklchOstrowski, createOklchOstrowski } from "./oklch-ostrowski.js";
import { raytrace, createRaytrace } from "./raytrace.js";
const entries = [
	["oklch-cubic", oklchCubic, createOklchCubic],
	["oklch-cubic-no-cache", oklchCubicNoCache, createOklchCubicNoCache],
	["oklch-cubic-direct", oklchCubicDirect, createOklchCubicDirect],
	["oklch-halley", oklchHalley, createOklchHalley],
	["oklch-ostrowski", oklchOstrowski, createOklchOstrowski],
	["raytrace", raytrace, createRaytrace],
];
export function createMatrixMappers (space) {
	return Object.fromEntries(entries.map(([name, p3, create]) => [name, space === DISPLAY_P3 ? p3 : create(space)]));
}
