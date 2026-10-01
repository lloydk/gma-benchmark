import { makeEdgeSeeker, makeEdgeSeekerIndexed } from "./makeEdgeSeeker.js";
import { getRgbConversions } from "../rgb-convert.js";

// Target choice, table generation and interval-index setup stay outside mapping.
// Both variants share immutable data, but have independent conversion closures
// for different descriptors. The index locates intervals; it never rounds hue.
function create (space, indexed) {
	const { rgbToOklch, oklchToClippedRgb, oklchToRgbIfInGamut } = getRgbConversions(space);
	const getMaxChroma = indexed ? makeEdgeSeekerIndexed(rgbToOklch) : makeEdgeSeeker(rgbToOklch);
	return function map (oklch, out, checkInGamut = false) {
		const l = oklch[0], c = oklch[1], h = oklch[2];
		// Canonical authored-coordinate conversion precedes table lookup and
		// endpoint handling, matching the Rust checked policy.
		if (checkInGamut && oklchToRgbIfInGamut(l, c, h, out)) return out;
		if (l <= 0) {
			out[0] = out[1] = out[2] = 0;
			return out;
		}
		if (l >= 1) {
			out[0] = out[1] = out[2] = 1;
			return out;
		}
		const maxChroma = getMaxChroma(l, h);
		return oklchToClippedRgb(l, c > maxChroma ? maxChroma : c, h, out);
	};
}
export const createEdgeSeeker = space => create(space, false);
export const createEdgeSeekerIndexed = space => create(space, true);
export function createEdgeSeekerMappers (space) {
	return {
		"edge-seeker": createEdgeSeeker(space),
		"edge-seeker-indexed": createEdgeSeekerIndexed(space),
	};
}
