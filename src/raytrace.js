import {
	KA0, KB0, KA1, KB1, KA2, KB2,
	RL, RM, RS, GL, GM, GS, BL, BM, BS,
	clampedGamma,
} from "./convert.js";

// Optimized scalar port of color.js-org/apps/gamut-mapping/methods/raytrace.js.
// It traces in linear Display-P3, then corrects only the OKLab chroma after each
// hit because L and H are overwritten with the original values.

const DEG_TO_RAD = Math.PI / 180;
const RAY_EPSILON = 1e-12;
const LOW = 1e-12;
const HIGH = 1 - LOW;

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

function linearP3ToOklabChroma (r, g, b) {
	let l = Math.cbrt(0.4813798527499543 * r + 0.4621183710113182 * g + 0.05650177623872754 * b);
	let m = Math.cbrt(0.2288319418112447 * r + 0.6532168193835677 * g + 0.11795123880518772 * b);
	let s = Math.cbrt(0.08394575232299314 * r + 0.22416527097756647 * g + 0.6918889766994405 * b);
	const a = 1.9779985324311684 * l - 2.4285922420485799 * m + 0.4505937096174110 * s;
	const labB = 0.0259040424655478 * l + 0.7827717124575296 * m - 0.8086757549230774 * s;
	return Math.sqrt(a * a + labB * labB);
}

function raytraceUnitBoxT (ar, ag, ab, mr, mg, mb) {
	let tnear = -Infinity;
	let tfar = Infinity;

	let d = mr - ar;
	if (d > RAY_EPSILON || d < -RAY_EPSILON) {
		const invD = 1 / d;
		const t1 = -ar * invD;
		const t2 = (1 - ar) * invD;
		if (t1 < t2) {
			if (t1 > tnear) {
				tnear = t1;
			}
			if (t2 < tfar) {
				tfar = t2;
			}
		}
		else {
			if (t2 > tnear) {
				tnear = t2;
			}
			if (t1 < tfar) {
				tfar = t1;
			}
		}
	}
	else if (ar < 0 || ar > 1) {
		return NaN;
	}

	d = mg - ag;
	if (d > RAY_EPSILON || d < -RAY_EPSILON) {
		const invD = 1 / d;
		const t1 = -ag * invD;
		const t2 = (1 - ag) * invD;
		if (t1 < t2) {
			if (t1 > tnear) {
				tnear = t1;
			}
			if (t2 < tfar) {
				tfar = t2;
			}
		}
		else {
			if (t2 > tnear) {
				tnear = t2;
			}
			if (t1 < tfar) {
				tfar = t1;
			}
		}
	}
	else if (ag < 0 || ag > 1) {
		return NaN;
	}

	d = mb - ab;
	if (d > RAY_EPSILON || d < -RAY_EPSILON) {
		const invD = 1 / d;
		const t1 = -ab * invD;
		const t2 = (1 - ab) * invD;
		if (t1 < t2) {
			if (t1 > tnear) {
				tnear = t1;
			}
			if (t2 < tfar) {
				tfar = t2;
			}
		}
		else {
			if (t2 > tnear) {
				tnear = t2;
			}
			if (t1 < tfar) {
				tfar = t1;
			}
		}
	}
	else if (ab < 0 || ab > 1) {
		return NaN;
	}

	if (tnear > tfar || tfar < 0) {
		return NaN;
	}
	if (tnear < 0) {
		tnear = tfar;
	}
	return tnear > -Infinity && tnear < Infinity ? tnear : NaN;
}

const linearScratch = [0, 0, 0];

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
	oklabToLinearP3(L, C * hueA, C * hueB, linearScratch);
	let mr = linearScratch[0], mg = linearScratch[1], mb = linearScratch[2];
	if (checkInGamut && mr >= 0 && mr <= 1 && mg >= 0 && mg <= 1 && mb >= 0 && mb <= 1) {
		out[0] = clampedGamma(mr);
		out[1] = clampedGamma(mg);
		out[2] = clampedGamma(mb);
		return out;
	}

	const anchor = L * L * L;
	let ar = anchor, ag = anchor, ab = anchor;
	let lastR = mr, lastG = mg, lastB = mb;

	for (let i = 0; i < 4; i++) {
		if (i) {
			const correctedC = linearP3ToOklabChroma(mr, mg, mb);
			oklabToLinearP3(L, correctedC * hueA, correctedC * hueB, linearScratch);
			mr = linearScratch[0];
			mg = linearScratch[1];
			mb = linearScratch[2];
		}

		const t = raytraceUnitBoxT(ar, ag, ab, mr, mg, mb);
		if (t !== t) {
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
