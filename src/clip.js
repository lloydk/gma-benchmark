import { oklchToClippedP3 } from "./convert.js";

// Naïve clip: convert OKLCh straight to P3 and clamp each channel into gamut.
// This is the theoretical floor — the cost of the conversion every method must do
// anyway — so it's the baseline the gamut-mapping methods are measured against.
//
// `oklch` is [L, C, H]; the clipped Display-P3 result is written into `out`.
export function clip (oklch, out) {
	return oklchToClippedP3(oklch[0], oklch[1], oklch[2], out);
}
