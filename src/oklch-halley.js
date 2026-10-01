import { DISPLAY_P3 } from "./rgb-spaces.js";
import { getRgbConversions } from "./rgb-convert.js";

import { blueFoldWindow, inBlueFold, outerFoldBoundary } from "./matrix-solver-policy.js";

export function createOklchHalley (space) {
	const [[, KA0, KB0], [, KA1, KB1], [, KA2, KB2]] = space.oklabToLms;
	const [[RL, RM, RS], [GL, GM, GS], [BL, BM, BS]] = space.lmsToRgb;
	const clampedGamma = space.transfer.encodeClamped;
	const { oklchToClippedRgb, oklchToRgbIfInGamut } = getRgbConversions(space);
	const foldWindow = blueFoldWindow(space);

	// Bracketed iteration outside the blue fold. Inside it, select the outer
	// feasible exit, matching the Rust mapping policy.
	const RGB_ROWS = [
		RL, RM, RS,
		GL, GM, GS,
		BL, BM, BS,
	];

	function solve (L, q0, q1, q2) {
		let lo = 0;
		let hi = 0.5; // Above the maximum target OKLCh chroma.

		// Seed with the earliest linearized channel crossing from neutral gray.
		const L3 = L * L * L;
		const L2x3 = 3 * L * L;
		let c = hi;
		for (let i = 0; i < 9; i += 3) {
			const w0 = RGB_ROWS[i], w1 = RGB_ROWS[i + 1], w2 = RGB_ROWS[i + 2];
			const slope = L2x3 * (w0 * q0 + w1 * q1 + w2 * q2);
			const crossing = slope > 0 ? (1 - L3) / slope : slope < 0 ? -L3 / slope : Infinity;
			if (crossing < c) {
				c = crossing;
			}
		}

		let best = c;
		let bestErr = Infinity;

		for (let iter = 0; iter < 16; iter++) {
			const l = L + c * q0;
			const m = L + c * q1;
			const s = L + c * q2;
			const l2 = l * l, m2 = m * m, s2 = s * s;
			const l3 = l2 * l, m3 = m2 * m, s3 = s2 * s;

			// Solve the most-violated lower or upper channel bound.
			let g = -Infinity, g1 = 0, g2 = 0;
			for (let i = 0; i < 9; i += 3) {
				const w0 = RGB_ROWS[i], w1 = RGB_ROWS[i + 1], w2 = RGB_ROWS[i + 2];
				const v = w0 * l3 + w1 * m3 + w2 * s3;
				const d1 = 3 * (w0 * q0 * l2 + w1 * q1 * m2 + w2 * q2 * s2);
				const d2 = 6 * (w0 * q0 * q0 * l + w1 * q1 * q1 * m + w2 * q2 * q2 * s);
				if (v - 1 > g) {
					g = v - 1;
					g1 = d1;
					g2 = d2;
				}
				if (-v > g) {
					g = -v;
					g1 = -d1;
					g2 = -d2;
				}
			}

			if (g > 0) {
				hi = c;
			}
			else {
				lo = c;
			}

			const err = g1 !== 0 ? Math.abs(g / g1) : Math.abs(g);
			if (err < bestErr) {
				bestErr = err;
				best = c;
			}

			const denom = 2 * g1 * g1 - g * g2;
			const step = denom !== 0 ? (2 * g * g1) / denom : g1 !== 0 ? g / g1 : 0;
			if (step < 1e-9 && step > -1e-9) {
				return c;
			}

			const next = c - step;
			c = next > lo && next < hi ? next : (lo + hi) / 2;
		}

		return best;
	}

	// `oklch` is [L, C, H]; clipped target RGB is written into `out`.
	function oklchHalley (oklch, out, checkInGamut = false) {
		const L = oklch[0], C = oklch[1], H = oklch[2];

		if (L <= 0 || L >= 1 || C <= 0) {
			return oklchToClippedRgb(L <= 0 ? 0 : L >= 1 ? 1 : L, 0, H, out);
		}
		if (checkInGamut && oklchToRgbIfInGamut(L, C, H, out)) {
			return out;
		}

		const rad = H * Math.PI / 180;
		const cos = Math.cos(rad), sin = Math.sin(rad);
		const q0 = KA0 * cos + KB0 * sin;
		const q1 = KA1 * cos + KB1 * sin;
		const q2 = KA2 * cos + KB2 * sin;
		const mappedC = Math.min(C, foldWindow && inBlueFold(H, foldWindow)
			? outerFoldBoundary(space.lmsToRgb, L, q0, q1, q2, C) : solve(L, q0, q1, q2));

		// Reuse the LMS' hue slopes from the solve for the final conversion.
		const l0 = L + mappedC * q0;
		const m0 = L + mappedC * q1;
		const s0 = L + mappedC * q2;
		const l = l0 * l0 * l0;
		const m = m0 * m0 * m0;
		const s = s0 * s0 * s0;
		out[0] = clampedGamma(RL * l + RM * m + RS * s);
		out[1] = clampedGamma(GL * l + GM * m + GS * s);
		out[2] = clampedGamma(BL * l + BM * m + BS * s);
		return out;
	}
	return oklchHalley;
}

export const oklchHalley = createOklchHalley(DISPLAY_P3);
