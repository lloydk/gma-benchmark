import { makeEdgeSeeker } from "./makeEdgeSeeker.js";
import { oklchToClippedP3, oklchToP3IfInGamut, p3ToOklch } from "../convert.js";

// Edge Seeker — reduce chroma to a precomputed LUT of the gamut edge. The LUT is
// built once at import using the hand-rolled P3→OKLCh converter.
const getMaxChroma = makeEdgeSeeker((r, g, b) => p3ToOklch(r, g, b));

// `oklch` is [L, C, H]; clipped Display-P3 is written into `out`.
export function edgeSeeker (oklch, out, checkInGamut = false) {
	const l = oklch[0], c = oklch[1], h = oklch[2];
	if (l <= 0) {
		out[0] = out[1] = out[2] = 0;
		return out;
	}
	if (l >= 1) {
		out[0] = out[1] = out[2] = 1;
		return out;
	}
	if (checkInGamut && oklchToP3IfInGamut(l, c, h, out)) {
		return out;
	}
	const maxChroma = getMaxChroma(l, h);
	return oklchToClippedP3(l, c > maxChroma ? maxChroma : c, h, out);
}
