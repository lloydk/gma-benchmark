import {
	KA0, KB0, KA1, KB1, KA2, KB2,
	RL, RM, RS, GL, GM, GS, BL, BM, BS,
	clampedGamma,
} from "./convert.js";

// Bjorn Ottosson constant-lightness gamut clipping, specialized for OKLCh -> P3.
// Constants are hoisted so the math stays auditable; modern JS engines still
// constant-fold these module bindings in hot optimized code.

const EPSILON = 1e-12;
const DEG_TO_RAD = Math.PI / 180;

const P3_RED1 = -1.772343927512981;
const P3_RED2 = -0.8207587433674072;
const P3_GREEN1 = 1.8031987175305495;
const P3_GREEN2 = -1.1932813966558915;

const P3_RED_K0 = 1.1941401817282744;
const P3_RED_K1 = 1.7629811997119493;
const P3_RED_K2 = 0.5958599382477117;
const P3_RED_K3 = 0.7575999740542505;
const P3_RED_K4 = 0.5681684967813678;
const P3_GREEN_K0 = 0.7395668192259771;
const P3_GREEN_K1 = -0.45954279991477065;
const P3_GREEN_K2 = 0.08285308768965816;
const P3_GREEN_K3 = 0.1254116495192955;
const P3_GREEN_K4 = -0.14503290744357106;
const P3_BLUE_K0 = 1.3650944117698118;
const P3_BLUE_K1 = -0.013962295571040945;
const P3_BLUE_K2 = -1.1452305089885595;
const P3_BLUE_K3 = -0.5025987876721942;
const P3_BLUE_K4 = 0.003174713114731378;

const linearScratch = [0, 0, 0];
const cuspScratch = [0, 0];

function clamp01 (x) {
	return x < 0 ? 0 : x > 1 ? 1 : x;
}

function oklabToLinearP3 (L, a, b, out) {
	const l = L + KA0 * a + KB0 * b;
	const m = L + KA1 * a + KB1 * b;
	const s = L + KA2 * a + KB2 * b;
	const l3 = l * l * l;
	const m3 = m * m * m;
	const s3 = s * s * s;
	out[0] = RL * l3 + RM * m3 + RS * s3;
	out[1] = GL * l3 + GM * m3 + GS * s3;
	out[2] = BL * l3 + BM * m3 + BS * s3;
	return out;
}

function linearP3ToClippedP3 (linear, out) {
	out[0] = clampedGamma(linear[0]);
	out[1] = clampedGamma(linear[1]);
	out[2] = clampedGamma(linear[2]);
	return out;
}

function oklabToP3IfInGamut (L, a, b, out) {
	oklabToLinearP3(L, a, b, linearScratch);
	const r = linearScratch[0], g = linearScratch[1], bl = linearScratch[2];
	if (r < 0 || r > 1 || g < 0 || g > 1 || bl < 0 || bl > 1) {
		return false;
	}
	return linearP3ToClippedP3(linearScratch, out);
}

function computeMaxSaturationP3 (a, b) {
	let k0, k1, k2, k3, k4;
	let wl, wm, ws;

	if (a * P3_RED1 + b * P3_RED2 > 1) {
		k0 = P3_RED_K0;
		k1 = P3_RED_K1;
		k2 = P3_RED_K2;
		k3 = P3_RED_K3;
		k4 = P3_RED_K4;
		wl = RL;
		wm = RM;
		ws = RS;
	}
	else if (a * P3_GREEN1 + b * P3_GREEN2 > 1) {
		k0 = P3_GREEN_K0;
		k1 = P3_GREEN_K1;
		k2 = P3_GREEN_K2;
		k3 = P3_GREEN_K3;
		k4 = P3_GREEN_K4;
		wl = GL;
		wm = GM;
		ws = GS;
	}
	else {
		k0 = P3_BLUE_K0;
		k1 = P3_BLUE_K1;
		k2 = P3_BLUE_K2;
		k3 = P3_BLUE_K3;
		k4 = P3_BLUE_K4;
		wl = BL;
		wm = BM;
		ws = BS;
	}

	const a2 = a * a;
	let sat = k0 + k1 * a + k2 * b + k3 * a2 + k4 * a * b;
	const kl = KA0 * a + KB0 * b;
	const km = KA1 * a + KB1 * b;
	const ks = KA2 * a + KB2 * b;
	const l = 1 + sat * kl;
	const m = 1 + sat * km;
	const s = 1 + sat * ks;
	const l2 = l * l;
	const m2 = m * m;
	const s2 = s * s;
	const f = wl * l2 * l + wm * m2 * m + ws * s2 * s;
	const f1 = 3 * (wl * kl * l2 + wm * km * m2 + ws * ks * s2);
	const f2 = 6 * (wl * kl * kl * l + wm * km * km * m + ws * ks * ks * s);

	return sat - (f * f1) / (f1 * f1 - 0.5 * f * f2);
}

function findCuspP3 (a, b, out) {
	const sCusp = computeMaxSaturationP3(a, b);
	const l = 1 + sCusp * (KA0 * a + KB0 * b);
	const m = 1 + sCusp * (KA1 * a + KB1 * b);
	const s = 1 + sCusp * (KA2 * a + KB2 * b);
	const l3 = l * l * l;
	const m3 = m * m * m;
	const s3 = s * s * s;
	const r = RL * l3 + RM * m3 + RS * s3;
	const g = GL * l3 + GM * m3 + GS * s3;
	const blue = BL * l3 + BM * m3 + BS * s3;
	const lCusp = Math.cbrt(1 / Math.max(r, g, blue));

	out[0] = lCusp;
	out[1] = lCusp * sCusp;
	return out;
}

function findGamutIntersectionP3 (a, b, l1, c1, l0, cusp) {
	let t;

	if ((l1 - l0) * cusp[1] - (cusp[0] - l0) * c1 <= 0) {
		const denom = c1 * cusp[0] + cusp[1] * (l0 - l1);
		t = denom === 0 ? 0 : (cusp[1] * l0) / denom;
	}
	else {
		const denom = c1 * (cusp[0] - 1) + cusp[1] * (l0 - l1);
		t = denom === 0 ? 0 : (cusp[1] * (l0 - 1)) / denom;

		const dl = l1 - l0;
		const kl = a * KA0 + b * KB0;
		const km = a * KA1 + b * KB1;
		const ks = a * KA2 + b * KB2;
		const ldtBase = dl + c1 * kl;
		const mdtBase = dl + c1 * km;
		const sdtBase = dl + c1 * ks;
		const L = l0 * (1 - t) + t * l1;
		const C = t * c1;
		const l = L + C * kl;
		const m = L + C * km;
		const s = L + C * ks;
		const l2 = l * l;
		const m2 = m * m;
		const s2 = s * s;
		const l3 = l2 * l;
		const m3 = m2 * m;
		const s3 = s2 * s;
		const ldt = 3 * ldtBase * l2;
		const mdt = 3 * mdtBase * m2;
		const sdt = 3 * sdtBase * s2;
		const ldt2 = 6 * ldtBase * ldtBase * l;
		const mdt2 = 6 * mdtBase * mdtBase * m;
		const sdt2 = 6 * sdtBase * sdtBase * s;

		const r = RL * l3 + RM * m3 + RS * s3 - 1;
		const r1 = RL * ldt + RM * mdt + RS * sdt;
		const r2 = RL * ldt2 + RM * mdt2 + RS * sdt2;
		const ur = r1 / (r1 * r1 - 0.5 * r * r2);
		const tr = ur >= 0 ? -r * ur : Number.MAX_VALUE;

		const g = GL * l3 + GM * m3 + GS * s3 - 1;
		const g1 = GL * ldt + GM * mdt + GS * sdt;
		const g2 = GL * ldt2 + GM * mdt2 + GS * sdt2;
		const ug = g1 / (g1 * g1 - 0.5 * g * g2);
		const tg = ug >= 0 ? -g * ug : Number.MAX_VALUE;

		const blue = BL * l3 + BM * m3 + BS * s3 - 1;
		const blue1 = BL * ldt + BM * mdt + BS * sdt;
		const blue2 = BL * ldt2 + BM * mdt2 + BS * sdt2;
		const ub = blue1 / (blue1 * blue1 - 0.5 * blue * blue2);
		const tb = ub >= 0 ? -blue * ub : Number.MAX_VALUE;

		t += Math.min(tr, Math.min(tg, tb));
	}

	return t;
}

// ── Cached variant ───────────────────────────────────────────────────────────
// The cusp and the LMS' hue slopes depend only on hue, so they are memoized in
// 0.1° buckets (same bucketed-hue semantics as oklch-cubic's hue structure).
// Each bucket stores [cuspL, cuspC, q0, q1, q2] where qᵢ is the LMS' slope for
// the hue direction: LMS'ᵢ(L, C) = L + C·qᵢ. With those cached, the per-call
// path needs no trig at all — the intersection's kl/km/ks are the qᵢ, and the
// final conversion rebuilds LMS' from the slopes. A flat Float64Array keeps
// the cache contiguous (~144 KB) instead of a pointer-chased object per hue.

const CUSP_HUE_SCALE = 10;
const cuspCache = new Float64Array((360 * CUSP_HUE_SCALE + 1) * 5);

function cachedCuspData (H) {
	H %= 360;
	if (H < 0) {
		H += 360;
	}
	const key = Math.round(H * CUSP_HUE_SCALE) * 5;
	if (cuspCache[key] === 0) { // cusp lightness is never 0 once filled
		const rad = (key / 5) / CUSP_HUE_SCALE * DEG_TO_RAD;
		const unitA = Math.cos(rad);
		const unitB = Math.sin(rad);
		findCuspP3(unitA, unitB, cuspScratch);
		cuspCache[key] = cuspScratch[0];
		cuspCache[key + 1] = cuspScratch[1];
		cuspCache[key + 2] = unitA * KA0 + unitB * KB0;
		cuspCache[key + 3] = unitA * KA1 + unitB * KB1;
		cuspCache[key + 4] = unitA * KA2 + unitB * KB2;
	}
	return key;
}

// findGamutIntersectionP3 with the hue expressed as LMS' slopes q0..q2 (the
// kl/km/ks it would otherwise recompute) and the cusp as scalars.
function findGamutIntersectionQ (q0, q1, q2, l1, c1, l0, cuspL, cuspC) {
	let t;

	if ((l1 - l0) * cuspC - (cuspL - l0) * c1 <= 0) {
		const denom = c1 * cuspL + cuspC * (l0 - l1);
		t = denom === 0 ? 0 : (cuspC * l0) / denom;
	}
	else {
		const denom = c1 * (cuspL - 1) + cuspC * (l0 - l1);
		t = denom === 0 ? 0 : (cuspC * (l0 - 1)) / denom;

		const dl = l1 - l0;
		const ldtBase = dl + c1 * q0;
		const mdtBase = dl + c1 * q1;
		const sdtBase = dl + c1 * q2;
		const L = l0 * (1 - t) + t * l1;
		const C = t * c1;
		const l = L + C * q0;
		const m = L + C * q1;
		const s = L + C * q2;
		const l2 = l * l;
		const m2 = m * m;
		const s2 = s * s;
		const l3 = l2 * l;
		const m3 = m2 * m;
		const s3 = s2 * s;
		const ldt = 3 * ldtBase * l2;
		const mdt = 3 * mdtBase * m2;
		const sdt = 3 * sdtBase * s2;
		const ldt2 = 6 * ldtBase * ldtBase * l;
		const mdt2 = 6 * mdtBase * mdtBase * m;
		const sdt2 = 6 * sdtBase * sdtBase * s;

		const r = RL * l3 + RM * m3 + RS * s3 - 1;
		const r1 = RL * ldt + RM * mdt + RS * sdt;
		const r2 = RL * ldt2 + RM * mdt2 + RS * sdt2;
		const ur = r1 / (r1 * r1 - 0.5 * r * r2);
		const tr = ur >= 0 ? -r * ur : Number.MAX_VALUE;

		const g = GL * l3 + GM * m3 + GS * s3 - 1;
		const g1 = GL * ldt + GM * mdt + GS * sdt;
		const g2 = GL * ldt2 + GM * mdt2 + GS * sdt2;
		const ug = g1 / (g1 * g1 - 0.5 * g * g2);
		const tg = ug >= 0 ? -g * ug : Number.MAX_VALUE;

		const blue = BL * l3 + BM * m3 + BS * s3 - 1;
		const blue1 = BL * ldt + BM * mdt + BS * sdt;
		const blue2 = BL * ldt2 + BM * mdt2 + BS * sdt2;
		const ub = blue1 / (blue1 * blue1 - 0.5 * blue * blue2);
		const tb = ub >= 0 ? -blue * ub : Number.MAX_VALUE;

		t += Math.min(tr, Math.min(tg, tb));
	}

	return t;
}

// LMS'-slope form of oklabToLinearP3 + clip: LMS'ᵢ = L + C·qᵢ.
function lmsSlopesToClippedP3 (L, C, q0, q1, q2, out) {
	const l0 = L + C * q0;
	const m0 = L + C * q1;
	const s0 = L + C * q2;
	const l = l0 * l0 * l0;
	const m = m0 * m0 * m0;
	const s = s0 * s0 * s0;
	out[0] = clampedGamma(RL * l + RM * m + RS * s);
	out[1] = clampedGamma(GL * l + GM * m + GS * s);
	out[2] = clampedGamma(BL * l + BM * m + BS * s);
	return out;
}

function lmsSlopesToP3IfInGamut (L, C, q0, q1, q2, out) {
	const l0 = L + C * q0;
	const m0 = L + C * q1;
	const s0 = L + C * q2;
	const l = l0 * l0 * l0;
	const m = m0 * m0 * m0;
	const s = s0 * s0 * s0;
	const r = RL * l + RM * m + RS * s;
	const g = GL * l + GM * m + GS * s;
	const bl = BL * l + BM * m + BS * s;
	if (r < 0 || r > 1 || g < 0 || g > 1 || bl < 0 || bl > 1) {
		return false;
	}
	out[0] = clampedGamma(r);
	out[1] = clampedGamma(g);
	out[2] = clampedGamma(bl);
	return true;
}

// `oklch` is [L, C, H]; clipped Display-P3 is written into `out`.
export function bottossonLightnessCached (oklch, out, checkInGamut = false) {
	let L = oklch[0];
	const C = Math.max(0, oklch[1]);
	const H = oklch[2];

	if (C <= EPSILON) {
		L = clamp01(L);
		const gray = clampedGamma(L * L * L);
		out[0] = gray;
		out[1] = gray;
		out[2] = gray;
		return out;
	}

	const k = cachedCuspData(H);
	const cuspL = cuspCache[k];
	const cuspC = cuspCache[k + 1];
	const q0 = cuspCache[k + 2];
	const q1 = cuspCache[k + 3];
	const q2 = cuspCache[k + 4];

	if (checkInGamut && lmsSlopesToP3IfInGamut(L, C, q0, q1, q2, out)) {
		return out;
	}

	const L0 = clamp01(L);
	const t = findGamutIntersectionQ(q0, q1, q2, L, C, L0, cuspL, cuspC);
	const mappedL = L0 * (1 - t) + t * L;
	const mappedC = t * C;

	return lmsSlopesToClippedP3(mappedL, mappedC, q0, q1, q2, out);
}

// `oklch` is [L, C, H]; clipped Display-P3 is written into `out`.
export function bottossonLightness (oklch, out, checkInGamut = false) {
	let L = oklch[0];
	const C = Math.max(0, oklch[1]);
	const H = oklch[2];

	if (C <= EPSILON) {
		L = clamp01(L);
		const gray = clampedGamma(L * L * L);
		out[0] = gray;
		out[1] = gray;
		out[2] = gray;
		return out;
	}

	const hRad = H * DEG_TO_RAD;
	const unitA = Math.cos(hRad);
	const unitB = Math.sin(hRad);
	const labA = C * unitA;
	const labB = C * unitB;

	if (checkInGamut && oklabToP3IfInGamut(L, labA, labB, out)) {
		return out;
	}

	const cusp = findCuspP3(unitA, unitB, cuspScratch);
	const L0 = clamp01(L);
	const t = findGamutIntersectionP3(unitA, unitB, L, C, L0, cusp);
	const mappedL = L0 * (1 - t) + t * L;
	const mappedC = t * C;

	oklabToLinearP3(mappedL, mappedC * unitA, mappedC * unitB, linearScratch);
	return linearP3ToClippedP3(linearScratch, out);
}
