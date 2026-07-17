import {
	oklchToClippedP3, oklchToP3IfInGamut, clampedGamma,
	KA0, KB0, KA1, KB1, KA2, KB2,
	RL, RM, RS, GL, GM, GS, BL, BM, BS,
} from "./convert.js";

// OKLCh cubic direct — solve each linear-P3 channel's complete cubic in chroma
// at the input lightness and exact hue. Unlike the cached cubic formulation,
// this keeps L in the coefficients from the outset: each channel needs only one
// Cardano solve, for whichever of 0 or 1 it approaches at C = 0.5.
//
// Ported from color-js/apps#44 issuecomment-4998357355. Display-P3's real OKLCh
// boundary is below C = 0.5, so that probe lies beyond the first boundary.

const P3_ROWS = [
	RL, RM, RS,
	GL, GM, GS,
	BL, BM, BS,
];

// Smallest nonnegative real root of a*x^3 + b*x^2 + c*x + d no greater than
// hi, or Infinity if none. This adapts the Cardano solver used by oklch-cubic.
// Do not replace it with firstRootNoCache unchanged: that solver's fixed 1e-14
// discriminant band is appropriate for its hue-normalized t = C/L polynomial,
// whereas these coefficients include L, L², and L³. Near black, a legitimately
// positive discriminant can be smaller than 1e-14, so this version tests zero
// relative to the two discriminant terms and polishes the selected root once.
function firstRootDirect (a, b, c, d, hi) {
	const aa = a, bb = b, cc = c, dd = d;
	let r0 = Infinity, r1 = Infinity, r2 = Infinity;

	if (Math.abs(a) < 1e-12) {
		if (Math.abs(b) < 1e-12) {
			if (Math.abs(c) >= 1e-12) {
				r0 = -d / c;
			}
		}
		else {
			const disc = c * c - 4 * b * d;
			if (disc >= 0) {
				const s = Math.sqrt(disc);
				r0 = (-c + s) / (2 * b);
				r1 = (-c - s) / (2 * b);
			}
		}
	}
	else {
		b /= a; c /= a; d /= a;
		const p = c - b * b / 3;
		const q = 2 * b * b * b / 27 - b * c / 3 + d;
		const off = -b / 3;
		const qTerm = q * q / 4;
		const pTerm = p * p * p / 27;
		const disc = qTerm + pTerm;
		const discTolerance = Number.EPSILON * 32 * (Math.abs(qTerm) + Math.abs(pTerm));

		if (disc > discTolerance) {
			const s = Math.sqrt(disc);
			r0 = Math.cbrt(-q / 2 + s) + Math.cbrt(-q / 2 - s) + off;
		}
		else if (disc >= -discTolerance) {
			const u = Math.cbrt(-q / 2);
			r0 = 2 * u + off;
			r1 = -u + off;
		}
		else {
			const m = 2 * Math.sqrt(-p / 3);
			const phi = Math.acos(Math.max(-1, Math.min(1, 3 * q / (p * m))));
			r0 = m * Math.cos(phi / 3) + off;
			r1 = m * Math.cos((phi - 2 * Math.PI) / 3) + off;
			r2 = m * Math.cos((phi - 4 * Math.PI) / 3) + off;
		}
	}

	let best = Infinity;
	if (r0 >= 0 && r0 <= hi) {
		best = r0;
	}
	if (r1 >= 0 && r1 <= hi && r1 < best) {
		best = r1;
	}
	if (r2 >= 0 && r2 <= hi && r2 < best) {
		best = r2;
	}
	// The direct coefficients span a much wider scale across lightness than the
	// hue-normalized cubic. Polish the selected Cardano root to recover the
	// boundary accuracy lost to cancellation in the closed-form expression.
	for (let i = 0; i < 1 && Number.isFinite(best); i++) {
		const derivative = (3 * aa * best + 2 * bb) * best + cc;
		if (derivative === 0) {
			break;
		}
		const next = best - (((aa * best + bb) * best + cc) * best + dd) / derivative;
		if (next < 0 || next > hi) {
			break;
		}
		best = next;
	}
	return best;
}

function maxChroma (L, q0, q1, q2) {
	const L2 = L * L;
	const L3 = L * L2;
	const Lx3 = L * 3;
	const L2x3 = L2 * 3;
	const q0b = q0 * q0, q1b = q1 * q1, q2b = q2 * q2;
	const q0c = q0b * q0, q1c = q1b * q1, q2c = q2b * q2;

	let best = Infinity;
	for (let i = 0; i < 9; i += 3) {
		const w0 = P3_ROWS[i], w1 = P3_ROWS[i + 1], w2 = P3_ROWS[i + 2];
		const c1 = q0c * w0 + q1c * w1 + q2c * w2;
		const c2 = Lx3 * (q0b * w0 + q1b * w1 + q2b * w2);
		const c3 = L2x3 * (q0 * w0 + q1 * w1 + q2 * w2);
		const c4 = L3 * (w0 + w1 + w2);

		// At C = 0.5, choose the nearer end of the channel's [0, 1] interval.
		const atHalf = ((c1 * 0.5 + c2) * 0.5 + c3) * 0.5 + c4;
		const target = Math.abs(atHalf - 1) < Math.abs(atHalf) ? 1 : 0;
		best = Math.min(best, firstRootDirect(c1, c2, c3, c4 - target, best));
	}
	return best;
}

// `oklch` is [L, C, H]; clipped Display-P3 is written into `out`.
export function oklchCubicDirect (oklch, out, checkInGamut = false) {
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
	const mappedC = Math.min(C, maxChroma(L, q0, q1, q2));

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
