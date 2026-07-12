import {
	oklchToClippedP3, oklchToP3IfInGamut, clampedGamma,
	KA0, KB0, KA1, KB1, KA2, KB2,
	RL, RM, RS, GL, GM, GS, BL, BM, BS,
} from "./convert.js";

// Reduce OKLCh chroma to the exact Display-P3 boundary with the bracketed
// Halley iteration from color-js/apps#44. At fixed L and H, each linear-P3
// channel is cubic in chroma:
//
//   channel(c) = sum(row[k] * (L + Q[k] * c)^3)
//
// The Halley step is accepted only while it remains inside the bracket between
// an in-gamut chroma and an out-of-gamut chroma; otherwise the solver bisects.
//
// Differences from the pull-request implementation:
//
//   - This benchmark targets only Display-P3. Its LMS³→linear-RGB matrix is the
//     hand-composed constant matrix from convert.js, stored flat below, instead
//     of being composed at runtime from Color.js space objects.
//   - Display-P3 has no blue fold, so the PR's per-gamut fold windows and second
//     outside-in solve for sRGB/Rec.2020 are omitted. `solve` is consequently
//     specialized to the analytic inside seed and fixed [0, 0.5] bracket rather
//     than accepting seed/lo/hi parameters.
//   - The PR mutates and returns a Color.js OKLCh color, leaving conversion and
//     final clipping to the method registry. This port accepts [L, C, H], writes
//     encoded Display-P3 into a reused output vector, and reuses the solver's
//     LMS' hue slopes for that conversion so it remains allocation-free.
//   - `checkInGamut` is a benchmark-specific optional fast path shared with the
//     other methods in this repository; it is not part of the PR implementation.
//   - The benchmark API assumes finite numeric L/C/H inputs. The PR accepts a
//     generic Color.js object, converts it to OKLCh, and uses H || 0 when
//     calling its boundary helper.

const P3_ROWS = [
	RL, RM, RS,
	GL, GM, GS,
	BL, BM, BS,
];

function solve (L, q0, q1, q2) {
	let lo = 0;
	let hi = 0.5; // Above the maximum real Display-P3 OKLCh chroma.

	// Seed with the earliest linearized channel crossing from neutral gray.
	const L3 = L * L * L;
	const L2x3 = 3 * L * L;
	let c = hi;
	for (let i = 0; i < 9; i += 3) {
		const w0 = P3_ROWS[i], w1 = P3_ROWS[i + 1], w2 = P3_ROWS[i + 2];
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
			const w0 = P3_ROWS[i], w1 = P3_ROWS[i + 1], w2 = P3_ROWS[i + 2];
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

// `oklch` is [L, C, H]; clipped Display-P3 is written into `out`.
export function oklchHalley (oklch, out, checkInGamut = false) {
	const L = oklch[0], C = oklch[1], H = oklch[2];

	if (L <= 0 || L >= 1 || C <= 0) {
		return oklchToClippedP3(L <= 0 ? 0 : L >= 1 ? 1 : L, 0, H, out);
	}
	if (checkInGamut && oklchToP3IfInGamut(L, C, H, out)) {
		return out;
	}

	const rad = H * Math.PI / 180;
	const cos = Math.cos(rad), sin = Math.sin(rad);
	const q0 = KA0 * cos + KB0 * sin;
	const q1 = KA1 * cos + KB1 * sin;
	const q2 = KA2 * cos + KB2 * sin;
	const mappedC = Math.min(C, solve(L, q0, q1, q2));

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
