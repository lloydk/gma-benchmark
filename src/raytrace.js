import {
	KA0, KB0, KA1, KB1, KA2, KB2,
	RL, RM, RS, GL, GM, GS, BL, BM, BS,
	clampedGamma,
} from "./convert.js";

// Optimized scalar port of color.js-org/apps/gamut-mapping/methods/raytrace.js.
// It traces in linear Display-P3, then corrects only the OKLab chroma after each
// hit because L and H are overwritten with the original values.
//
// The ray anchor is always strictly inside the unit box: it starts at gray
// (L^3, L^3, L^3) with 0 < L < 1 (the L <= 0 / L >= 1 cases early-return, and
// an L^3 that underflows to 0 is treated as black), and anchor updates are
// gated on the corrected color being strictly inside (LOW..HIGH). For an
// interior origin the slab method always resolves to the ray's exit distance,
// so the general tnear/tfar bookkeeping collapses to the closed form in exitT.

const DEG_TO_RAD = Math.PI / 180;
const RAY_EPSILON = 1e-12;
const LOW = 1e-12;
const HIGH = 1 - LOW;

function linearP3ToOklabChroma (r, g, b) {
	let l = Math.cbrt(0.4813798527499543 * r + 0.4621183710113182 * g + 0.05650177623872754 * b);
	let m = Math.cbrt(0.2288319418112447 * r + 0.6532168193835677 * g + 0.11795123880518772 * b);
	let s = Math.cbrt(0.08394575232299314 * r + 0.22416527097756647 * g + 0.6918889766994405 * b);
	const a = 1.9779985324311684 * l - 2.4285922420485799 * m + 0.4505937096174110 * s;
	const labB = 0.0259040424655478 * l + 0.7827717124575296 * m - 0.8086757549230774 * s;
	return Math.sqrt(a * a + labB * labB);
}

// Exit distance of a ray from a point strictly inside the unit box:
// min over axes of max((1 - a) / d, -a / d). |d| <= RAY_EPSILON is flushed to
// 0 so that axis contributes max(-Infinity, +Infinity) = +Infinity (no
// constraint), matching the slab method's parallel-axis skip; if all three
// axes are parallel the result is +Infinity, which the caller treats as
// "no hit".
function exitT (ar, ag, ab, dr, dg, db) {
	if (dr <= RAY_EPSILON && dr >= -RAY_EPSILON) {
		dr = 0;
	}
	if (dg <= RAY_EPSILON && dg >= -RAY_EPSILON) {
		dg = 0;
	}
	if (db <= RAY_EPSILON && db >= -RAY_EPSILON) {
		db = 0;
	}
	const ir = 1 / dr, ig = 1 / dg, ib = 1 / db;
	const tr = Math.max((1 - ar) * ir, -ar * ir);
	const tg = Math.max((1 - ag) * ig, -ag * ig);
	const tb = Math.max((1 - ab) * ib, -ab * ib);
	return Math.min(tr, tg, tb);
}

// `oklch` is [L, C, H]; clipped Display-P3 is written into `out`.
export function raytrace (oklch, out, checkInGamut = false) {
	const L = oklch[0], C = oklch[1], H = oklch[2];

	if (L <= 0) {
		out[0] = out[1] = out[2] = 0;
		return out;
	}
	if (L >= 1) {
		out[0] = out[1] = out[2] = 1;
		return out;
	}
	if (C <= 0) {
		const gray = clampedGamma(L * L * L);
		out[0] = out[1] = out[2] = gray;
		return out;
	}
	const hr = H * DEG_TO_RAD;
	const hueA = Math.cos(hr);
	const hueB = Math.sin(hr);

	let a = C * hueA, b = C * hueB;
	let l = L + KA0 * a + KB0 * b;
	let m = L + KA1 * a + KB1 * b;
	let s = L + KA2 * a + KB2 * b;
	let l3 = l * l * l, m3 = m * m * m, s3 = s * s * s;
	let mr = RL * l3 + RM * m3 + RS * s3;
	let mg = GL * l3 + GM * m3 + GS * s3;
	let mb = BL * l3 + BM * m3 + BS * s3;

	if (checkInGamut && mr >= 0 && mr <= 1 && mg >= 0 && mg <= 1 && mb >= 0 && mb <= 1) {
		out[0] = clampedGamma(mr);
		out[1] = clampedGamma(mg);
		out[2] = clampedGamma(mb);
		return out;
	}

	const anchor = L * L * L;
	// L^3 underflows to 0 for L below ~1.7e-108; the anchor then sits on the
	// cube corner, breaking the strictly-inside invariant exitT relies on
	// (a flushed-parallel axis would yield -0 * Infinity = NaN). Lightness that
	// underflows in linear space is black, same as the L <= 0 early-return.
	if (anchor === 0) {
		out[0] = out[1] = out[2] = 0;
		return out;
	}
	let ar = anchor, ag = anchor, ab = anchor;
	let lastR = mr, lastG = mg, lastB = mb;

	for (let i = 0; i < 4; i++) {
		if (i) {
			const correctedC = linearP3ToOklabChroma(mr, mg, mb);
			a = correctedC * hueA;
			b = correctedC * hueB;
			l = L + KA0 * a + KB0 * b;
			m = L + KA1 * a + KB1 * b;
			s = L + KA2 * a + KB2 * b;
			l3 = l * l * l;
			m3 = m * m * m;
			s3 = s * s * s;
			mr = RL * l3 + RM * m3 + RS * s3;
			mg = GL * l3 + GM * m3 + GS * s3;
			mb = BL * l3 + BM * m3 + BS * s3;
		}

		const t = exitT(ar, ag, ab, mr - ar, mg - ag, mb - ab);
		if (!Number.isFinite(t)) {
			mr = lastR;
			mg = lastG;
			mb = lastB;
			break;
		}

		const hitR = ar + (mr - ar) * t;
		const hitG = ag + (mg - ag) * t;
		const hitB = ab + (mb - ab) * t;

		if (i && mr > LOW && mr < HIGH && mg > LOW && mg < HIGH && mb > LOW && mb < HIGH) {
			ar = mr;
			ag = mg;
			ab = mb;
		}

		lastR = mr = hitR;
		lastG = mg = hitG;
		lastB = mb = hitB;
	}

	out[0] = clampedGamma(mr);
	out[1] = clampedGamma(mg);
	out[2] = clampedGamma(mb);
	return out;
}
