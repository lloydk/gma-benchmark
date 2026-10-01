import { DISPLAY_P3 } from "./rgb-spaces.js";
import { getRgbConversions } from "./rgb-convert.js";
import { firstRoot as firstRootNoCache, firstFaceRoot } from "./polynomial.js";

export function createOklchCubicNoCache (space) {
	const [[, KA0, KB0], [, KA1, KB1], [, KA2, KB2]] = space.oklabToLms;
	const [[RL, RM, RS], [GL, GM, GS], [BL, BM, BS]] = space.lmsToRgb;
	const clampedGamma = space.transfer.encodeClamped;
	const { oklchToClippedRgb, oklchToRgbIfInGamut } = getRgbConversions(space);

	// OKLCh cubic without hue-data caching. This intentionally preserves the cached
	// method's fixed 0.1 degree bucket semantics, but recomputes the bucket data on
	// every call so the benchmark isolates storage/reuse from the cubic algorithm.

	const HUE_SCALE = 10;

	function firstTurnNoCache (D, B, A) {
		return firstRootNoCache(0, D, 2 * B, A, 1e-12, Infinity);
	}

	// `oklch` is [L, C, H]; clipped target RGB is written into `out`.
	function oklchCubicNoCache (oklch, out, checkInGamut = false) {
		const L = oklch[0], C = oklch[1], H = oklch[2];

		if (L <= 0 || L >= 1 || C <= 0) {
			return oklchToClippedRgb(L <= 0 ? 0 : L >= 1 ? 1 : L, 0, H, out);
		}
		if (checkInGamut && oklchToRgbIfInGamut(L, C, H, out)) {
			return out;
		}

		let normalizedH = H % 360;
		if (normalizedH < 0) {
			normalizedH += 360;
		}
		const rad = Math.round(normalizedH * HUE_SCALE) / HUE_SCALE * Math.PI / 180;
		const cos = Math.cos(rad), sin = Math.sin(rad);

		const q0 = KA0 * cos + KB0 * sin;
		const q1 = KA1 * cos + KB1 * sin;
		const q2 = KA2 * cos + KB2 * sin;

		const a0 = RL * q0 + RM * q1 + RS * q2;
		const a1 = GL * q0 + GM * q1 + GS * q2;
		const a2 = BL * q0 + BM * q1 + BS * q2;

		const q0b = q0 * q0, q1b = q1 * q1, q2b = q2 * q2;
		const b0 = RL * q0b + RM * q1b + RS * q2b;
		const b1 = GL * q0b + GM * q1b + GS * q2b;
		const b2 = BL * q0b + BM * q1b + BS * q2b;

		const q0c = q0b * q0, q1c = q1b * q1, q2c = q2b * q2;
		const d0 = RL * q0c + RM * q1c + RS * q2c;
		const d1 = GL * q0c + GM * q1c + GS * q2c;
		const d2 = BL * q0c + BM * q1c + BS * q2c;

		const tLower0 = firstRootNoCache(d0, 3 * b0, 3 * a0, 1, 1e-9, Infinity);
		const tLower1 = firstRootNoCache(d1, 3 * b1, 3 * a1, 1, 1e-9, Infinity);
		const tLower2 = firstRootNoCache(d2, 3 * b2, 3 * a2, 1, 1e-9, Infinity);
		let maxT = Math.min(C / L, tLower0, tLower1, tLower2);
		const turn0 = firstTurnNoCache(d0, b0, a0);
		const turn1 = firstTurnNoCache(d1, b1, a1);
		const turn2 = firstTurnNoCache(d2, b2, a2);

		const L3 = L * L * L;
		const target = 1 / L3;
		const dd = 1 - target;

		if (turn0 <= maxT || (a0 > 0 && !((((d0 * maxT + 3 * b0) * maxT + 3 * a0) * maxT + 1) < target))) {
			maxT = Math.min(maxT, firstFaceRoot(d0, 3 * b0, 3 * a0, dd, maxT, true));
		}
		if (turn1 <= maxT || (a1 > 0 && !((((d1 * maxT + 3 * b1) * maxT + 3 * a1) * maxT + 1) < target))) {
			maxT = Math.min(maxT, firstFaceRoot(d1, 3 * b1, 3 * a1, dd, maxT, true));
		}
		if (turn2 <= maxT || (a2 > 0 && !((((d2 * maxT + 3 * b2) * maxT + 3 * a2) * maxT + 1) < target))) {
			maxT = Math.min(maxT, firstFaceRoot(d2, 3 * b2, 3 * a2, dd, maxT, true));
		}

		out[0] = clampedGamma(L3 * (((d0 * maxT + 3 * b0) * maxT + 3 * a0) * maxT + 1));
		out[1] = clampedGamma(L3 * (((d1 * maxT + 3 * b1) * maxT + 3 * a1) * maxT + 1));
		out[2] = clampedGamma(L3 * (((d2 * maxT + 3 * b2) * maxT + 3 * a2) * maxT + 1));
		return out;
	}
	return oklchCubicNoCache;
}

export const oklchCubicNoCache = createOklchCubicNoCache(DISPLAY_P3);
