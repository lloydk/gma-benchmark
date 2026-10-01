import { DISPLAY_P3 } from "./rgb-spaces.js";
import { getRgbConversions } from "./rgb-convert.js";

// The target is selected at setup. The returned mapper reuses the caller's out.
export function createClip (space) {
	const { oklchToClippedRgb } = getRgbConversions(space);
	return function clip (oklch, out) {
		return oklchToClippedRgb(oklch[0], oklch[1], oklch[2], out);
	};
}
export const clip = createClip(DISPLAY_P3);
