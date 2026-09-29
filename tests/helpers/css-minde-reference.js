// Literal CSS Color 4 Local MINDE reference for tests, read 2026-09-28:
// https://www.w3.org/TR/css-color-4/#binsearch
// Matrices from the spec's sample conversions (kept uncomposed through XYZ):
// https://github.com/w3c/csswg-drafts/blob/main/css-color-4/conversions.js
// No production conversions, scalar clipping shortcut, or solver are used.
const LAB_TO_LMS = [
	[1, 0.3963377773761749, 0.2158037573099136],
	[1, -0.1055613458156586, -0.0638541728258133],
	[1, -0.0894841775298119, -1.2914855480194092],
];
const LMS_TO_XYZ = [
	[1.2268798758459243, -0.5578149944602171, 0.2813910456659647],
	[-0.0405757452148008, 1.1122868032803170, -0.0717110580655164],
	[-0.0763729366746601, -0.4214933324022432, 1.5869240198367816],
];
const XYZ_TO_P3 = [
	[446124 / 178915, -333277 / 357830, -72051 / 178915],
	[-14852 / 17905, 63121 / 35810, 423 / 17905],
	[11844 / 330415, -50337 / 660830, 316169 / 330415],
];
const P3_TO_XYZ = [
	[608311 / 1250200, 189793 / 714400, 198249 / 1000160],
	[35783 / 156275, 247089 / 357200, 198249 / 2500400],
	[0, 32229 / 714400, 5220557 / 5000800],
];
const XYZ_TO_LMS = [
	[0.8190224379967030, 0.3619062600528904, -0.1288737815209879],
	[0.0329836539323885, 0.9292868615863434, 0.0361446663506424],
	[0.0481771893596242, 0.2642395317527308, 0.6335478284694309],
];
const LMS_TO_LAB = [
	[0.2104542683093140, 0.7936177747023054, -0.0040720430116193],
	[1.9779985324311684, -2.4285922420485799, 0.4505937096174110],
	[0.0259040424655478, 0.7827717124575296, -0.8086757549230774],
];
const multiply = (matrix, vector) => matrix.map(row => row.reduce((sum, v, i) => sum + v * vector[i], 0));
const encode = x => Math.abs(x) <= 0.0031308 ? 12.92 * x : Math.sign(x) * (1.055 * Math.abs(x) ** (1 / 2.4) - 0.055);
const decode = x => Math.abs(x) <= 0.04045 ? x / 12.92 : Math.sign(x) * ((Math.abs(x) + 0.055) / 1.055) ** 2.4;
const lab = ([l, c, h]) => [l, c * Math.cos(h * Math.PI / 180), c * Math.sin(h * Math.PI / 180)];
const inGamut = rgb => rgb.every(v => v >= 0 && v <= 1);
const clip = rgb => rgb.map(v => Math.max(0, Math.min(1, v)));

export function referenceP3 (lch) {
	const lms = multiply(LAB_TO_LMS, lab(lch)).map(v => v ** 3);
	return multiply(XYZ_TO_P3, multiply(LMS_TO_XYZ, lms)).map(encode);
}

export function referenceLab (rgb) {
	const xyz = multiply(P3_TO_XYZ, rgb.map(decode));
	return multiply(LMS_TO_LAB, multiply(XYZ_TO_LMS, xyz).map(Math.cbrt));
}

export function referenceCssMinde (origin) {
	const current = [...origin];
	const trace = [];
	const result = (rgb, reason, error = 0) => ({ rgb, reason, error, chroma: current[1], trace });
	if (current[0] >= 1) return result([1, 1, 1], "white");
	if (current[0] <= 0) return result([0, 0, 0], "black");
	const converted = referenceP3(current);
	if (inGamut(converted)) return result(converted, "in-gamut");
	const delta = rgb => {
		const one = referenceLab(rgb), two = lab(current);
		return Math.hypot(...one.map((v, i) => v - two[i]));
	};
	let clipped = clip(converted);
	let E = delta(clipped);
	if (E < 0.02) return result(clipped, "initial-clip", E);
	let min = 0, max = current[1], minInGamut = true;
	while (max - min > 0.0001) {
		const chroma = (min + max) / 2;
		current[1] = chroma;
		const rgb = referenceP3(current);
		if (minInGamut && inGamut(rgb)) {
			min = chroma;
			trace.push({ chroma, action: "in-gamut" });
			continue;
		}
		clipped = clip(rgb);
		E = delta(clipped);
		trace.push({ chroma, error: E, action: E < 0.02 ? "min" : "max" });
		if (E < 0.02) {
			if (0.02 - E < 0.0001) return result(clipped, "close-enough", E);
			minInGamut = false;
			min = chroma;
		}
		else max = chroma;
	}
	return result(clipped, "interval", E);
}
