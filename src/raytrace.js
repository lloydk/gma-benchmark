import { AL, AM, AS, BL as LAB_BL, BM as LAB_BM, BS as LAB_BS } from "./oklab.js";
import { DISPLAY_P3 } from "./rgb-spaces.js";
import { getRgbConversions } from "./rgb-convert.js";

export function createRaytrace (space) {
	const [[, KA0, KB0], [, KA1, KB1], [, KA2, KB2]] = space.oklabToLms;
	const [[RL, RM, RS], [GL, GM, GS], [BL, BM, BS]] = space.lmsToRgb;
	const clampedGamma = space.transfer.encodeClamped;
	const { oklchToClippedRgb } = getRgbConversions(space);

	// Optimized scalar port of color.js-org/apps/gamut-mapping/methods/raytrace.js.
	// It traces in linear target RGB, then corrects only the OKLab chroma after each
	// hit because L and H are overwritten with the original values.
	//
	// The ray anchor is always strictly inside the unit box: it starts at gray
	// (L^3, L^3, L^3) with 0 < L < 1 (the L <= 0 / L >= 1 cases early-return, and
	// an L^3 that underflows to 0 is treated as black), and anchor updates are
	// gated on the corrected color being strictly inside (LOW..HIGH). For an
	// interior origin the slab method always resolves to the ray's exit distance,
	// so the general tnear/tfar bookkeeping collapses to the closed form in exitT.

	const LOW = 1e-12;
	const HIGH = 1 - LOW;

	const [[LR, LG, LB], [MR, MG, MB], [SR, SG, SB]] = space.rgbToLms;

	function linearRgbToOklabChroma (r, g, b) {
		let l = Math.cbrt(LR * r + LG * g + LB * b);
		let m = Math.cbrt(MR * r + MG * g + MB * b);
		let s = Math.cbrt(SR * r + SG * g + SB * b);
		const a = AL * l + AM * m + AS * s;
		const labB = LAB_BL * l + LAB_BM * m + LAB_BS * s;
		return Math.sqrt(a * a + labB * labB);
	}

	// Direct division retains tiny nonzero directions at near-black/white faces.
	function exitT (ar, ag, ab, dr, dg, db) {
		const tr = dr > 0 ? (1-ar)/dr : dr < 0 ? -ar/dr : Infinity;
		const tg = dg > 0 ? (1-ag)/dg : dg < 0 ? -ag/dg : Infinity;
		const tb = db > 0 ? (1-ab)/db : db < 0 ? -ab/db : Infinity;
		return Math.min(tr,tg,tb);
	}

	// `oklch` is [L, C, H]; clipped target RGB is written into `out`.
	function raytrace (oklch, out, checkInGamut = false) {
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
			return oklchToClippedRgb(L, 0, H, out);
		}
		const hr = H * Math.PI / 180;
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
		// cube corner, so there is no interior ray anchor. Lightness that
		// underflows in linear space is black, same as the L <= 0 early-return.
		if (anchor === 0) {
			out[0] = out[1] = out[2] = 0;
			return out;
		}
		let ar = anchor, ag = anchor, ab = anchor;
		let lastR = mr, lastG = mg, lastB = mb;

		for (let i = 0; i < 4; i++) {
			if (i) {
				const correctedC = linearRgbToOklabChroma(mr, mg, mb);
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

			// A converged ray has only roundoff direction left. Keep the last hit
			// instead of amplifying that noise into another full cube crossing.
			if (i && (Math.max(Math.abs(mr - ar), Math.abs(mg - ag), Math.abs(mb - ab)) <= 32 * Number.EPSILON * Math.max(Math.abs(ar), Math.abs(ag), Math.abs(ab))
				|| Math.max(Math.abs(mr - lastR), Math.abs(mg - lastG), Math.abs(mb - lastB)) <= 32 * Number.EPSILON * Math.max(Math.abs(lastR), Math.abs(lastG), Math.abs(lastB)))) {
				mr = lastR; mg = lastG; mb = lastB;
				break;
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
	return raytrace;
}

export const raytrace = createRaytrace(DISPLAY_P3);
