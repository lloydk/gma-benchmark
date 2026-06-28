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
