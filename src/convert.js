// Hand-rolled OKLCh ↔ Display-P3 conversions — no color library.
//
// Matrices are the standard OKLab ones composed with the Display-P3 primaries
// (verified against colorjs.io to ~1e-14). Display-P3 shares sRGB's transfer
// function. The forward path clips into gamut (clamp linear → [0,1] before the
// transfer function, which is monotonic and fixes 0 and 1), so it always yields a
// displayable P3 color.

// ── OKLab → LMS' (a,b columns; the L column is all 1s, so L passes through) ──
export const KA0 = 0.3963377773761749, KB0 = 0.2158037573099136;
export const KA1 = -0.1055613458156586, KB1 = -0.0638541728258133;
export const KA2 = -0.0894841775298119, KB2 = -1.2914855480194092;

// ── LMS (cubed) → linear Display-P3 ──
export const RL = 3.127768971361874, RM = -2.2571357625916395, RS = 0.12936679122976516;
export const GL = -1.0910090184377979, GM = 2.413331710306922, GS = -0.32232269186912466;
export const BL = -0.02601080193857028, BM = -0.508041331704167, BS = 1.5340521336427373;

// ── sRGB / Display-P3 transfer functions ──
// Linear → gamma, clamped to [0, 1] (inputs are clipped to gamut first).
export function clampedGamma (x) {
	x = x < 0 ? 0 : x > 1 ? 1 : x;
	return x <= 0.0031308 ? x * 12.92 : 1.055 * x ** (1 / 2.4) - 0.055;
}

// Gamma → linear (sign-extended for completeness; only used in the LUT build).
function gammaToLinear (x) {
	const a = Math.abs(x);
	return a <= 0.04045 ? x / 12.92 : Math.sign(x) * ((a + 0.055) / 1.055) ** 2.4;
}

// ── OKLab → clipped Display-P3, written into `out` (no allocation) ──
export function oklabToClippedP3 (L, a, b, out) {
	const l = (L + KA0 * a + KB0 * b) ** 3;
	const m = (L + KA1 * a + KB1 * b) ** 3;
	const s = (L + KA2 * a + KB2 * b) ** 3;
	out[0] = clampedGamma(RL * l + RM * m + RS * s);
	out[1] = clampedGamma(GL * l + GM * m + GS * s);
	out[2] = clampedGamma(BL * l + BM * m + BS * s);
	return out;
}

// ── OKLCh → clipped Display-P3, written into `out` ──
export function oklchToClippedP3 (L, C, H, out) {
	const hr = H * Math.PI / 180;
	return oklabToClippedP3(L, C * Math.cos(hr), C * Math.sin(hr), out);
}

// ── OKLCh → Display-P3 only when already in gamut ──
export function oklchToP3IfInGamut (L, C, H, out) {
	const hr = H * Math.PI / 180;
	const a = C * Math.cos(hr);
	const b = C * Math.sin(hr);
	const l = (L + KA0 * a + KB0 * b) ** 3;
	const m = (L + KA1 * a + KB1 * b) ** 3;
	const s = (L + KA2 * a + KB2 * b) ** 3;
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

// ── Display-P3 (gamma) → OKLCh, returning { l, c, h } ──
// Used only when building the Edge Seeker LUT, so clarity over speed.
export function p3ToOklch (r, g, b) {
	const rl = gammaToLinear(r), gl = gammaToLinear(g), bl = gammaToLinear(b);
	const l = Math.cbrt(0.4813798527499543 * rl + 0.4621183710113182 * gl + 0.05650177623872754 * bl);
	const m = Math.cbrt(0.2288319418112447 * rl + 0.6532168193835677 * gl + 0.11795123880518772 * bl);
	const s = Math.cbrt(0.08394575232299314 * rl + 0.22416527097756647 * gl + 0.6918889766994405 * bl);
	const L = 0.2104542683093140 * l + 0.7936177747023054 * m - 0.0040720430116193 * s;
	const A = 1.9779985324311684 * l - 2.4285922420485799 * m + 0.4505937096174110 * s;
	const B = 0.0259040424655478 * l + 0.7827717124575296 * m - 0.8086757549230774 * s;
	let h = Math.atan2(B, A) * 180 / Math.PI;
	if (h < 0) {
		h += 360;
	}
	return { l: L, c: Math.sqrt(A * A + B * B), h };
}
