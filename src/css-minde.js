// CSS Color 4 binary search with Local MINDE, specialized to OKLCh → Display-P3.
// https://www.w3.org/TR/css-color-4/#binsearch
import {
	KA0, KB0, KA1, KB1, KA2, KB2,
	RL, RM, RS, GL, GM, GS, BL, BM, BS,
	clampedGamma,
} from "./convert.js";

const JND = 0.02;
const EPSILON = 0.0001;

// Clipping in linear P3 commutes with its monotonic transfer function. Measure
// deltaEOK directly from the clipped linear channels, encoding only on return.
function deltaFromClipped (L, a, b, r, g, blue) {
	const l = Math.cbrt(0.4813798527499543 * r + 0.4621183710113182 * g + 0.05650177623872754 * blue);
	const m = Math.cbrt(0.2288319418112447 * r + 0.6532168193835677 * g + 0.11795123880518772 * blue);
	const s = Math.cbrt(0.08394575232299314 * r + 0.22416527097756647 * g + 0.6918889766994405 * blue);
	const dL = 0.2104542683093140 * l + 0.7936177747023054 * m - 0.0040720430116193 * s - L;
	const da = 1.9779985324311684 * l - 2.4285922420485799 * m + 0.4505937096174110 * s - a;
	const db = 0.0259040424655478 * l + 0.7827717124575296 * m - 0.8086757549230774 * s - b;
	return Math.sqrt(dL * dL + da * da + db * db);
}

// The spec's in-gamut check is intrinsic: both benchmark modes use this function.
export function cssMinde (oklch, out) {
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
