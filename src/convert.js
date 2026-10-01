// Compatibility exports for the existing P3 solvers and LUT builder.
import { DISPLAY_P3 } from "./rgb-spaces.js";
import { getRgbConversions } from "./rgb-convert.js";
export const [[, KA0, KB0], [, KA1, KB1], [, KA2, KB2]] = DISPLAY_P3.oklabToLms;
export const [[RL, RM, RS], [GL, GM, GS], [BL, BM, BS]] = DISPLAY_P3.lmsToRgb;
export const clampedGamma = DISPLAY_P3.transfer.encodeClamped;
export const {
	oklabToClippedRgb: oklabToClippedP3,
	oklchToClippedRgb: oklchToClippedP3,
	oklchToRgbIfInGamut: oklchToP3IfInGamut,
	rgbToOklch: p3ToOklch,
} = getRgbConversions(DISPLAY_P3);
