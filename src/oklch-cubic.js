import {
	oklchToClippedP3, oklchToP3IfInGamut, clampedGamma,
	KA0, KB0, KA1, KB1, KA2, KB2,
	RL, RM, RS, GL, GM, GS, BL, BM, BS,
} from "./convert.js";

// OKLCh cubic — reduce chroma to the exact P3 gamut boundary by solving, in closed
// form, the cubic each linear-P3 channel traces as a function of chroma. The
// per-hue structure is memoized (cached variant).

// Smallest real root of a·t³ + b·t² + c·t + d in (lo, hi), or Infinity if none.
function firstRoot (a, b, c, d, lo, hi) {
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

// Smallest t > 0 where a channel turns: first positive root of D·t² + 2B·t + A.
function firstTurn (D, B, A) {
	return firstRoot(0, D, 2 * B, A, 1e-12, Infinity);
}

// Per-hue cubic structure. At fixed L,H each linear-P3 channel is exactly cubic in
// chroma: channelᵢ(c) = L³·Pᵢ(t), t = c/L, Pᵢ(t) = 1 + 3Aᵢt + 3Bᵢt² + Dᵢt³.
function getHueData (H) {
	const rad = H * Math.PI / 180;
	const cos = Math.cos(rad), sin = Math.sin(rad);
	// Q = OKLab→LMS · [0, cos, sin] (the L column contributes nothing here).
	const q0 = KA0 * cos + KB0 * sin;
	const q1 = KA1 * cos + KB1 * sin;
	const q2 = KA2 * cos + KB2 * sin;
	// A = Q·(LMS→RGB), B = Q²·(LMS→RGB), D = Q³·(LMS→RGB).
	const A = [RL * q0 + RM * q1 + RS * q2, GL * q0 + GM * q1 + GS * q2, BL * q0 + BM * q1 + BS * q2];
	const q0b = q0 * q0, q1b = q1 * q1, q2b = q2 * q2;
	const B = [RL * q0b + RM * q1b + RS * q2b, GL * q0b + GM * q1b + GS * q2b, BL * q0b + BM * q1b + BS * q2b];
	const q0c = q0b * q0, q1c = q1b * q1, q2c = q2b * q2;
	const D = [RL * q0c + RM * q1c + RS * q2c, GL * q0c + GM * q1c + GS * q2c, BL * q0c + BM * q1c + BS * q2c];

	let tLower = Infinity;
	const turn = [];
	for (let i = 0; i < 3; i++) {
		tLower = Math.min(tLower, firstRoot(D[i], 3 * B[i], 3 * A[i], 1, 1e-9, Infinity));
		turn[i] = firstTurn(D[i], B[i], A[i]);
	}
	return { A, B, D, tLower, turn };
}

// Cache the per-hue structure in fixed 0.1° buckets indexed into a pre-sized array.
const HUE_SCALE = 10;
const hueCache = new Array(360 * HUE_SCALE + 1);
function cachedHueData (H) {
	H %= 360;
	if (H < 0) {
		H += 360;
	}
	const key = Math.round(H * HUE_SCALE);
	let data = hueCache[key];
	if (data === undefined) {
		data = getHueData(key / HUE_SCALE);
		hueCache[key] = data;
	}
	return data;
}

// `oklch` is [L, C, H]; clipped Display-P3 is written into `out`.
export function oklchCubic (oklch, out, checkInGamut = false) {
	const L = oklch[0], C = oklch[1], H = oklch[2];

	// Achromatic / white / black.
	if (L <= 0 || L >= 1 || C <= 0) {
		return oklchToClippedP3(L <= 0 ? 0 : L >= 1 ? 1 : L, 0, H, out);
	}
	if (checkInGamut && oklchToP3IfInGamut(L, C, H, out)) {
		return out;
	}

	const { A, B, D, tLower, turn } = cachedHueData(H);

	// Work in t = c/L. Cap at the input chroma and the (hue-only) lower exit; the
	// white bound below can only pull it lower.
	const t0 = C / L;
	let maxT = Math.min(t0, tLower);
	const L3 = L * L * L;
	const target = 1 / L3; // Pᵢ value at the white bound
	const d = 1 - target;
	for (let i = 0; i < 3; i++) {
		if (turn[i] > maxT) {
			if (A[i] <= 0) {
				continue;
			}
			const PmaxT = ((D[i] * maxT + 3 * B[i]) * maxT + 3 * A[i]) * maxT + 1;
			if (PmaxT < target) {
				continue;
			}
		}
		maxT = Math.min(maxT, firstRoot(D[i], 3 * B[i], 3 * A[i], d, 1e-9, maxT));
	}

	// linear-P3 straight from the hue cubic (channelᵢ = L³·Pᵢ(maxT)),
	// reusing A,B,D — no second trig + matrix conversion.
	out[0] = clampedGamma(L3 * (((D[0] * maxT + 3 * B[0]) * maxT + 3 * A[0]) * maxT + 1));
	out[1] = clampedGamma(L3 * (((D[1] * maxT + 3 * B[1]) * maxT + 3 * A[1]) * maxT + 1));
	out[2] = clampedGamma(L3 * (((D[2] * maxT + 3 * B[2]) * maxT + 3 * A[2]) * maxT + 1));
	return out;
}
