import { RGB_SPACES } from "./rgb-spaces.js";
import { getRgbConversions } from "./rgb-convert.js";
import { bottossonFits } from "./generated/bottosson.js";

// Constant-lightness approximation, matching Rust f64. The unchecked policy
// projects even interior inputs to its fitted boundary; checked calls preserve
// canonical authored-hue conversions before consulting the quantized cache.
function createBottosson (space, cached) {
	if (RGB_SPACES[space.id] !== space || !Object.hasOwn(bottossonFits, space.id)) throw new RangeError(`No Bottosson fits for ${space.id}`);
	const data = bottossonFits[space.id];
	const [[, KA0, KB0], [, KA1, KB1], [, KA2, KB2]] = space.oklabToLms;
	const [[RL, RM, RS], [GL, GM, GS], [BL, BM, BS]] = space.lmsToRgb;
	const clampedGamma = space.transfer.encodeClamped;
	const { oklabToClippedRgb, oklchToRgbIfInGamut } = getRgbConversions(space);
	const EPSILON = 1e-12;

	const [RED1, RED2] = data.red;
	const [GREEN1, GREEN2] = data.green;
	const [[RED_K0, RED_K1, RED_K2, RED_K3, RED_K4],
		[GREEN_K0, GREEN_K1, GREEN_K2, GREEN_K3, GREEN_K4],
		[BLUE_K0, BLUE_K1, BLUE_K2, BLUE_K3, BLUE_K4]] = data.fits;
	const hues = data.primaryHues;
	const cuspScratch = [0, 0];

	function clamp01 (x) {
		return x < 0 ? 0 : x > 1 ? 1 : x;
	}

	// Resolve roundoff at shared primary contacts using authored hue, as in Rust.
	function sector (a, b, h) {
		const red = a * RED1 + b * RED2;
		const green = a * GREEN1 + b * GREEN2;
		if (Math.abs(red - 1) <= 16 * Number.EPSILON || Math.abs(green - 1) <= 16 * Number.EPSILON) {
			h %= 360;
			const offset = h < 0 ? 360 : 0;
			if (h >= hues[1] - offset && h < hues[2] - offset) return 0;
			if (h < hues[0] - offset || h >= hues[2] - offset) return 1;
			return 2;
		}
		return red > 1 ? 0 : green > 1 ? 1 : 2;
	}

	function computeMaxSaturationRgb (a, b, h) {
		let k0, k1, k2, k3, k4;
		let wl, wm, ws;

		const channel = sector(a, b, h);
		if (channel === 0) {
			k0 = RED_K0;
			k1 = RED_K1;
			k2 = RED_K2;
			k3 = RED_K3;
			k4 = RED_K4;
			wl = RL;
			wm = RM;
			ws = RS;
		}
		else if (channel === 1) {
			k0 = GREEN_K0;
			k1 = GREEN_K1;
			k2 = GREEN_K2;
			k3 = GREEN_K3;
			k4 = GREEN_K4;
			wl = GL;
			wm = GM;
			ws = GS;
		}
		else {
			k0 = BLUE_K0;
			k1 = BLUE_K1;
			k2 = BLUE_K2;
			k3 = BLUE_K3;
			k4 = BLUE_K4;
			wl = BL;
			wm = BM;
			ws = BS;
		}

		const a2 = a * a;
		const sat = k0 + k1 * a + k2 * b + k3 * a2 + k4 * a * b;
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

	function findCuspRgb (a, b, h, out) {
		const sCusp = computeMaxSaturationRgb(a, b, h);
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

	// ── Cached variant ───────────────────────────────────────────────────────────
	// The cusp and the LMS' hue slopes depend only on hue, so they are memoized in
	// 0.1° buckets (same bucketed-hue semantics as oklch-cubic's hue structure).
	// Each bucket stores [cuspL, cuspC, q0, q1, q2] where qᵢ is the LMS' slope for
	// the hue direction: LMS'ᵢ(L, C) = L + C·qᵢ. With those cached, the per-call
	// unchecked path needs no trig — the intersection's kl/km/ks are the qᵢ, and the
	// final conversion rebuilds LMS' from the slopes. A flat Float64Array keeps
	// the cache contiguous (~144 KB) instead of a pointer-chased object per hue.

	const CUSP_HUE_SCALE = 10;
	const cuspCache = cached ? new Float64Array((360 * CUSP_HUE_SCALE + 1) * 5) : null;

	function cachedCuspData (H) {
		H %= 360;
		if (H < 0) {
			H += 360;
		}
		const key = Math.round(H * CUSP_HUE_SCALE) * 5;
		if (cuspCache[key] === 0) { // cusp lightness is never 0 once filled
			const rad = (key / 5) / CUSP_HUE_SCALE * Math.PI / 180;
			const unitA = Math.cos(rad);
			const unitB = Math.sin(rad);
			findCuspRgb(unitA, unitB, key / 5 / CUSP_HUE_SCALE, cuspScratch);
			cuspCache[key] = cuspScratch[0];
			cuspCache[key + 1] = cuspScratch[1];
			cuspCache[key + 2] = unitA * KA0 + unitB * KB0;
			cuspCache[key + 3] = unitA * KA1 + unitB * KB1;
			cuspCache[key + 4] = unitA * KA2 + unitB * KB2;
		}
		return key;
	}

	// Intersection with hue expressed as LMS' slopes and the cusp as scalars.
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
			const L = l0 + t * (l1 - l0);
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

	// LMS'-slope conversion: LMS'ᵢ = L + C·qᵢ.
	function lmsSlopesToClippedRgb (L, C, q0, q1, q2, out) {
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

	// `oklch` is [L, C, H]; encoded target RGB is written into `out`.
	function bottossonLightnessCached (oklch, out, checkInGamut = false) {
		let L = oklch[0];
		const C = Math.max(0, oklch[1]);
		const H = oklch[2];

		if (checkInGamut && oklchToRgbIfInGamut(L, oklch[1], H, out)) return out;

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


		const L0 = clamp01(L);
		const t = findGamutIntersectionQ(q0, q1, q2, L, C, L0, cuspL, cuspC);
		const mappedL = L0 + t * (L - L0);
		const mappedC = t * C;

		return lmsSlopesToClippedRgb(mappedL, mappedC, q0, q1, q2, out);
	}

	// `oklch` is [L, C, H]; encoded target RGB is written into `out`.
	function bottossonLightness (oklch, out, checkInGamut = false) {
		let L = oklch[0];
		const C = Math.max(0, oklch[1]);
		const H = oklch[2];

		if (checkInGamut && oklchToRgbIfInGamut(L, oklch[1], H, out)) return out;

		if (C <= EPSILON) {
			L = clamp01(L);
			const gray = clampedGamma(L * L * L);
			out[0] = gray;
			out[1] = gray;
			out[2] = gray;
			return out;
		}

		const hRad = H * Math.PI / 180;
		const unitA = Math.cos(hRad);
		const unitB = Math.sin(hRad);

		const cusp = findCuspRgb(unitA, unitB, H, cuspScratch);
		const L0 = clamp01(L);
		const t = findGamutIntersectionQ(unitA * KA0 + unitB * KB0, unitA * KA1 + unitB * KB1, unitA * KA2 + unitB * KB2, L, C, L0, cusp[0], cusp[1]);
		const mappedL = L0 + t * (L - L0);
		const mappedC = t * C;

		return oklabToClippedRgb(mappedL, mappedC * unitA, mappedC * unitB, out);
	}
	return cached ? bottossonLightnessCached : bottossonLightness;
}
export const createBottossonLightness = space => createBottosson(space, false);
export const createBottossonLightnessCached = space => createBottosson(space, true);
export function createBottossonMappers (space) {
	return {
		"bottosson-lightness": createBottossonLightness(space),
		"bottosson-lightness-cached": createBottossonLightnessCached(space),
	};
}
