import { LL, LM, LS, AL, AM, AS, BL as LAB_BL, BM as LAB_BM, BS as LAB_BS } from "./oklab.js";
// Bind a conversion kernel once. Mapping calls allocate no color objects.
export function createRgbConversions (space) {
	const [[, KA0, KB0], [, KA1, KB1], [, KA2, KB2]] = space.oklabToLms;
	const [[RL, RM, RS], [GL, GM, GS], [BL, BM, BS]] = space.lmsToRgb;
	const [[LR, LG, LB], [MR, MG, MB], [SR, SG, SB]] = space.rgbToLms;
	const { encodeClamped, decode } = space.transfer;
	// ── OKLab → clipped target RGB, written into `out` (no allocation) ──
	// Cubes are explicit multiplications: V8 compiles `x ** 3` to a full pow call
	// (~7 ns each) while JSC and rustc reduce it to multiplies.
	function oklabToClippedRgb (L, a, b, out) {
		const l0 = L + KA0 * a + KB0 * b;
		const m0 = L + KA1 * a + KB1 * b;
		const s0 = L + KA2 * a + KB2 * b;
		const l = l0 * l0 * l0;
		const m = m0 * m0 * m0;
		const s = s0 * s0 * s0;
		out[0] = encodeClamped(RL * l + RM * m + RS * s);
		out[1] = encodeClamped(GL * l + GM * m + GS * s);
		out[2] = encodeClamped(BL * l + BM * m + BS * s);
		return out;
	}

	// ── OKLCh → clipped target RGB, written into `out` ──
	function oklchToClippedRgb (L, C, H, out) {
		const hr = H * Math.PI / 180;
		return oklabToClippedRgb(L, C * Math.cos(hr), C * Math.sin(hr), out);
	}

	// ── OKLCh → target RGB only when already in gamut ──
	function oklchToRgbIfInGamut (L, C, H, out) {
		const hr = H * Math.PI / 180;
		const a = C * Math.cos(hr);
		const b = C * Math.sin(hr);
		// @bottosson-precheck-begin
		const l0 = L + KA0 * a + KB0 * b;
		const m0 = L + KA1 * a + KB1 * b;
		const s0 = L + KA2 * a + KB2 * b;
		const l = l0 * l0 * l0;
		const m = m0 * m0 * m0;
		const s = s0 * s0 * s0;
		const r = RL * l + RM * m + RS * s;
		const g = GL * l + GM * m + GS * s;
		const bl = BL * l + BM * m + BS * s;
		if (!(r >= 0 && r <= 1 && g >= 0 && g <= 1 && bl >= 0 && bl <= 1)) {
			return false;
		}
		out[0] = encodeClamped(r);
		out[1] = encodeClamped(g);
		out[2] = encodeClamped(bl);
		return true;
		// @bottosson-precheck-end
	}

	// ── Target RGB (gamma) → OKLCh, returning { l, c, h } ──
	// Used only when building the Edge Seeker LUT, so clarity over speed.
	function rgbToOklch (r, g, b) {
		const rl = decode(r), gl = decode(g), bl = decode(b);
		const l = Math.cbrt(LR * rl + LG * gl + LB * bl);
		const m = Math.cbrt(MR * rl + MG * gl + MB * bl);
		const s = Math.cbrt(SR * rl + SG * gl + SB * bl);
		const L = LL * l + LM * m + LS * s;
		const A = AL * l + AM * m + AS * s;
		const B = LAB_BL * l + LAB_BM * m + LAB_BS * s;
		let h = Math.atan2(B, A) * 180 / Math.PI;
		if (h < 0) {
			h += 360;
		}
		return { l: L, c: Math.sqrt(A * A + B * B), h };
	}
	return Object.freeze({ oklabToClippedRgb, oklchToClippedRgb, oklchToRgbIfInGamut, rgbToOklch });
}

// Immutable conversion closures are shared by every mapper for this descriptor.
// Cache identity by object, not name, so custom descriptors cannot alias kernels.
const conversions = new WeakMap();
export function getRgbConversions (space) {
 let conversion = conversions.get(space);
 if (!conversion) {
  conversion = createRgbConversions(space);
  conversions.set(space, conversion);
 }
 return conversion;
}
