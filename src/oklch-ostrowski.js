import {
	oklchToClippedP3, oklchToP3IfInGamut, clampedGamma,
	KA0, KB0, KA1, KB1, KA2, KB2,
	RL, RM, RS, GL, GM, GS, BL, BM, BS,
} from "./convert.js";

// Reduce OKLCh chroma to the exact Display-P3 boundary with the bracketed
// Ostrowski iteration proposed in color-js/apps#44 issuecomment-4964705945.
// This is the same specialized, allocation-free Display-P3 port as
// oklch-halley.js, with only its solve function replaced by the fourth-order
// Ostrowski method from the follow-up comment.

const P3_ROWS = [
	RL, RM, RS,
	GL, GM, GS,
	BL, BM, BS,
];

function solve (L, q0, q1, q2) {
	let lo = 0;
	let hi = 0.5; // Above the maximum real Display-P3 OKLCh chroma.

	// Earliest linearized crossing of a channel with 0 or 1: channel_i(0) = L^3
	// with slope 3L^2*A_i, so the crossing is (1 - L^3)/slope or -L^3/slope.
	const L3 = L * L * L;
	const L2x3 = 3 * L * L;
	let c = hi;
	for (let i = 0; i < 9; i += 3) {
		const w0 = P3_ROWS[i], w1 = P3_ROWS[i + 1], w2 = P3_ROWS[i + 2];
		const d1 = L2x3 * (w0 * q0 + w1 * q1 + w2 * q2);
		const crossing = d1 > 0 ? (1 - L3) / d1 : d1 < 0 ? -L3 / d1 : Infinity;
		if (crossing < c) {
			c = crossing;
		}
	}

	let best = c;
	let bestErr = Infinity;

	for (let iter = 0; iter < 16; iter++) {
		let l = L + c * q0;
		let m = L + c * q1;
		let s = L + c * q2;
		let l2 = l * l, m2 = m * m, s2 = s * s;
		let l3 = l2 * l, m3 = m2 * m, s3 = s2 * s;

		// The most-violated channel bound is the active constraint.
		let g = -Infinity;
		let g1 = 0;
		for (let i = 0; i < 9; i += 3) {
			const w0 = P3_ROWS[i], w1 = P3_ROWS[i + 1], w2 = P3_ROWS[i + 2];
			const v = w0 * l3 + w1 * m3 + w2 * s3;
			const d1 = 3 * (w0 * q0 * l2 + w1 * q1 * m2 + w2 * q2 * s2);
			if (v - 1 > g) {
				g = v - 1;
				g1 = d1;
			}
			if (-v > g) {
				g = -v;
				g1 = -d1;
			}
		}

		if (g > 0) {
			hi = c;
		}
		else {
			lo = c;
		}

		// The first derivative has no solution; bisect the bracket instead.
		if (g1 === 0) {
			c = (lo + hi) / 2;
			continue;
		}

		let step = g / g1;
		if (Math.abs(step) < bestErr) {
			bestErr = Math.abs(step);
			best = c;
		}

		if (step < 1e-9 && step > -1e-9) {
			return c;
		}

		// Newton step.
		let next = c - step;

		l = L + next * q0;
		m = L + next * q1;
		s = L + next * q2;
		l2 = l * l;
		m2 = m * m;
		s2 = s * s;
		l3 = l2 * l;
		m3 = m2 * m;
		s3 = s2 * s;

		let gAtNext = -Infinity;
		for (let i = 0; i < 9; i += 3) {
			const w0 = P3_ROWS[i], w1 = P3_ROWS[i + 1], w2 = P3_ROWS[i + 2];
			const v = w0 * l3 + w1 * m3 + w2 * s3;
			if (v - 1 > gAtNext) {
				gAtNext = v - 1;
			}
			if (-v > gAtNext) {
				gAtNext = -v;
			}
		}

		const twiceGAtNext = 2 * gAtNext;
		if (twiceGAtNext !== g) {
			step = g / (g - twiceGAtNext) * (gAtNext / g1);

			if (Math.abs(step) < bestErr) {
				bestErr = Math.abs(step);
				best = next;
			}

			if (step < 1e-9 && step > -1e-9) {
				return next;
			}

			// Ostrowski correction step.
			next -= step;
		}

		c = next > lo && next < hi ? next : (lo + hi) / 2;
	}

	return best;
}

// `oklch` is [L, C, H]; clipped Display-P3 is written into `out`.
export function oklchOstrowski (oklch, out, checkInGamut = false) {
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
