import {
	oklchToClippedP3, oklchToP3IfInGamut, clampedGamma,
	KA0, KB0, KA1, KB1, KA2, KB2,
	RL, RM, RS, GL, GM, GS, BL, BM, BS,
} from "./convert.js";

// OKLCh cubic without hue-data caching. This intentionally preserves the cached
// method's fixed 0.1 degree bucket semantics, but recomputes the bucket data on
// every call so the benchmark isolates storage/reuse from the cubic algorithm.

const HUE_SCALE = 10;

// Cardano solver for the hue-normalized t = C/L polynomials used below. Its
// fixed discriminant band is part of the existing cubic's numerical behavior.
// The direct cubic must not reuse it unchanged: once L, L², and L³ are folded
// into the coefficients, valid discriminants can fall inside this fixed band.
function firstRootNoCache (a, b, c, d, lo, hi) {
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
		const disc = q * q / 4 + p * p * p / 27;

		if (disc > 1e-14) {
			const s = Math.sqrt(disc);
			r0 = Math.cbrt(-q / 2 + s) + Math.cbrt(-q / 2 - s) + off;
		}
		else if (disc > -1e-14) {
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
	if (r0 > lo && r0 < hi) {
		best = r0;
	}
	if (r1 > lo && r1 < hi && r1 < best) {
		best = r1;
	}
	if (r2 > lo && r2 < hi && r2 < best) {
		best = r2;
	}
	return best;
}

function firstTurnNoCache (D, B, A) {
	return firstRootNoCache(0, D, 2 * B, A, 1e-12, Infinity);
}

// `oklch` is [L, C, H]; clipped Display-P3 is written into `out`.
export function oklchCubicNoCache (oklch, out, checkInGamut = false) {
	const L = oklch[0], C = oklch[1], H = oklch[2];

	if (L <= 0 || L >= 1 || C <= 0) {
		return oklchToClippedP3(L <= 0 ? 0 : L >= 1 ? 1 : L, 0, H, out);
	}
	if (checkInGamut && oklchToP3IfInGamut(L, C, H, out)) {
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
		maxT = Math.min(maxT, firstRootNoCache(d0, 3 * b0, 3 * a0, dd, 1e-9, maxT));
	}
	if (turn1 <= maxT || (a1 > 0 && !((((d1 * maxT + 3 * b1) * maxT + 3 * a1) * maxT + 1) < target))) {
		maxT = Math.min(maxT, firstRootNoCache(d1, 3 * b1, 3 * a1, dd, 1e-9, maxT));
	}
	if (turn2 <= maxT || (a2 > 0 && !((((d2 * maxT + 3 * b2) * maxT + 3 * a2) * maxT + 1) < target))) {
		maxT = Math.min(maxT, firstRootNoCache(d2, 3 * b2, 3 * a2, dd, 1e-9, maxT));
	}

	out[0] = clampedGamma(L3 * (((d0 * maxT + 3 * b0) * maxT + 3 * a0) * maxT + 1));
	out[1] = clampedGamma(L3 * (((d1 * maxT + 3 * b1) * maxT + 3 * a1) * maxT + 1));
	out[2] = clampedGamma(L3 * (((d2 * maxT + 3 * b2) * maxT + 3 * a2) * maxT + 1));
	return out;
}
