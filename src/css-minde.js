import { LL, LM, LS, AL, AM, AS, BL as LAB_BL, BM as LAB_BM, BS as LAB_BS } from "./oklab.js";
// CSS Color 4 Local MINDE; target selection happens once, outside mapping.
import { DISPLAY_P3 } from "./rgb-spaces.js";
export function createCssMinde (space) {
	const [[, KA0, KB0], [, KA1, KB1], [, KA2, KB2]] = space.oklabToLms;
	const [[RL, RM, RS], [GL, GM, GS], [BL, BM, BS]] = space.lmsToRgb;
	const [[LR, LG, LB], [MR, MG, MB], [SR, SG, SB]] = space.rgbToLms;
	const clampedGamma = space.transfer.encodeClamped;
	const JND = 0.02;
	const EPSILON = 0.0001;

	// Clipping in linear RGB commutes with its monotonic transfer function. Measure
	// deltaEOK directly from the clipped linear channels, encoding only on return.
	function deltaFromClipped (L, a, b, r, g, blue) {
		const l = Math.cbrt(LR * r + LG * g + LB * blue);
		const m = Math.cbrt(MR * r + MG * g + MB * blue);
		const s = Math.cbrt(SR * r + SG * g + SB * blue);
		const dL = LL * l + LM * m + LS * s - L;
		const da = AL * l + AM * m + AS * s - a;
		const db = LAB_BL * l + LAB_BM * m + LAB_BS * s - b;
		return Math.sqrt(dL * dL + da * da + db * db);
	}

	// The spec's in-gamut check is intrinsic: both benchmark modes use this function.
	function cssMinde (oklch, out) {
		const L = oklch[0], C = Math.max(0, oklch[1]);
		if (L <= 0 || L >= 1) {
			out[0] = out[1] = out[2] = L <= 0 ? 0 : 1;
			return out;
		}
		let H = C === 0 ? 0 : oklch[2];
		if (H <= -1e9 || H >= 1e9) H %= 360;
		const hr = H * Math.PI / 180;
		const hueA = Math.cos(hr), hueB = Math.sin(hr);
		let a = C * hueA, b = C * hueB;
		let l = L + KA0 * a + KB0 * b;
		let m = L + KA1 * a + KB1 * b;
		let s = L + KA2 * a + KB2 * b;
		let l3 = l * l * l, m3 = m * m * m, s3 = s * s * s;
		let r = RL * l3 + RM * m3 + RS * s3;
		let g = GL * l3 + GM * m3 + GS * s3;
		let blue = BL * l3 + BM * m3 + BS * s3;

		// Zero-tolerance membership and the usual conversion arithmetic preserve
		// in-gamut inputs. EPSILON applies only to search termination / JND proximity.
		if (!(r >= 0 && r <= 1 && g >= 0 && g <= 1 && blue >= 0 && blue <= 1)) {
			r = Math.min(1, Math.max(0, r));
			g = Math.min(1, Math.max(0, g));
			blue = Math.min(1, Math.max(0, blue));
			// The original clip can already be within a JND; do not reduce chroma then.
			if (deltaFromClipped(L, a, b, r, g, blue) >= JND) {
				let min = 0, max = C, minInGamut = true;
				while (max - min > EPSILON) {
					const chroma = (min + max) / 2;
					a = chroma * hueA;
					b = chroma * hueB;
					l = L + KA0 * a + KB0 * b;
					m = L + KA1 * a + KB1 * b;
					s = L + KA2 * a + KB2 * b;
					l3 = l * l * l;
					m3 = m * m * m;
					s3 = s * s * s;
					const cr = RL * l3 + RM * m3 + RS * s3;
					const cg = GL * l3 + GM * m3 + GS * s3;
					const cb = BL * l3 + BM * m3 + BS * s3;
					if (minInGamut && cr >= 0 && cr <= 1 && cg >= 0 && cg <= 1 && cb >= 0 && cb <= 1) {
						min = chroma;
						continue;
					}
					r = Math.min(1, Math.max(0, cr));
					g = Math.min(1, Math.max(0, cg));
					blue = Math.min(1, Math.max(0, cb));
					const E = deltaFromClipped(L, a, b, r, g, blue);
					if (E < JND) {
						if (JND - E < EPSILON) break;
						minInGamut = false;
						min = chroma;
					}
					else max = chroma;
				}
			}
		}
		// Return the last clip, including when the interval exhausts above the JND.
		out[0] = clampedGamma(r);
		out[1] = clampedGamma(g);
		out[2] = clampedGamma(blue);
		return out;
	}
	return cssMinde;
}
export const cssMinde = createCssMinde(DISPLAY_P3);
