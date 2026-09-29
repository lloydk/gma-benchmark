// Dualray: guarded upper-first shortcut and upper-face retry.
import { clampedGamma } from "./convert.js";

// Display-P3 solver with direction fits, one lower Halley step, two upper Householder steps,
// convergence/face guards, and derivative-isolated first-root fallback.
// Basis constants precompose the Display-P3 profile. Seed polynomials use
// balanced evaluation instead of a serial Horner chain in t².
// No angle-based fit is used.
// Seed evaluation is inline; the Householder correction cancels common factors.
//
// Benchmark adaptation: direct OKLCh trig and edge cases match Householder.
// There is no source-conversion or normalized-RGB in-gamut precheck. Instead,
// compare input C/L with the lower or validated upper boundary and evaluate
// the input when it lies inside, as the other dual-ray variants do. The checked
// benchmark row aliases this function. No LUT or per-hue state is retained.

const DEG_TO_RAD = Math.PI / 180;
const HUE_FAST_LIMIT = 1e9;
const rootLimit = 4;
const red1 = -1.772343927512981, red2 = -0.8207587433674072;
const green1 = 1.8031987175305495, green2 = -1.1932813966558915;

const rD3 = 0.0791058979743933;
const rD2 = 0.5655398231442658;
const rD1 = 0.11818565248860147;
const rD0 = -0.24664826663200515;
const rB0 = 1.0567091736627137;
const rB2 = 0.34490732484069164;
const rB1 = 1.6035484701795497;
const rA1 = 4.39902903039207;
const rA0 = 1.9561094754786617;
const gD3 = -0.1513261575650534;
const gD2 = -0.7888313324352596;
const gD1 = 0.0807940995543469;
const gD0 = 0.6827265304693584;
const gB0 = -1.7357487868412962;
const gB2 = 1.2945435832569667;
const gB1 = -0.6857891074040978;
const gA1 = -1.974959555170497;
const gA0 = 0.08018985838156922;
const bD3 = 0.685552592139929;
const bD2 = 3.255498281543475;
const bD1 = -0.687673578309416;
const bD0 = -3.304652083761079;
const bB0 = 7.666248909056808;
const bB2 = -7.658638747824462;
const bB1 = 1.0298244006069064;
const bA1 = -0.28185879057092095;
const bA0 = -5.863136490898882;

function value (d, b, a, x) {
	return ((d * x + b) * x + a) * x + 1;
}

function polish (x, d, b, a) {
	const f = value(d, b, a, x);
	const f1 = (3 * d * x + 2 * b) * x + a;
	const f2 = 6 * d * x + 2 * b;
	const denominator = 2 * f1 * f1 - f * f2;
	return denominator !== 0 ? x - (2 * f * f1) / denominator : x;
}

// Cold path. Partition at derivative roots so a shallow negative interval
// cannot be skipped by endpoint bracketing. Used for both lower-root
// guard failures and upper-face/convergence failures.
function firstRoot (d, b, a, constant, limit) {
	let s0 = limit;
	let s1 = limit;
	if (d === 0) {
		const s = -a / (2 * b);
		if (s > 0 && s < limit) s0 = s;
	} else {
		const discriminant = b * b - 3 * d * a;
		if (discriminant >= 0) {
			const q = -b - (b < 0 ? -1 : 1) * Math.sqrt(discriminant);
			const t0 = q / (3 * d);
			const t1 = q === 0 ? 0 : a / q;
			const low = Math.min(t0, t1);
			const high = Math.max(t0, t1);
			if (low > 0 && low < limit) s0 = low;
			if (high > 0 && high < limit) {
				if (s0 === limit) s0 = high;
				else s1 = high;
			}
		}
	}
	let lo = 0;
	let flo = constant;
	for (let interval = 0; interval < 3; interval++) {
		let hi = interval === 0 ? s0 : interval === 1 ? s1 : limit;
		const fhi = ((d * hi + b) * hi + a) * hi + constant;
		if (fhi === 0) return hi;
		if (flo < 0 !== fhi < 0) {
			for (let step = 0; step < 64; step++) {
				const mid = lo + (hi - lo) * 0.5;
				if (mid === lo || mid === hi) break;
				const f = ((d * mid + b) * mid + a) * mid + constant;
				if (f < 0 === flo < 0) {
					lo = mid;
					flo = f;
				} else hi = mid;
			}
			return lo + (hi - lo) * 0.5;
		}
		lo = hi;
		flo = fhi;
		if (hi === limit) break;
	}
	return Infinity;
}

export function dualray (oklch, out) {
	const L = oklch[0], C = oklch[1], H = oklch[2];
	if (L <= 0) { out[0] = out[1] = out[2] = 0; return out; }
	if (L >= 1) { out[0] = out[1] = out[2] = 1; return out; }
	if (C <= 0) { const gray = clampedGamma(L * L * L); out[0] = out[1] = out[2] = gray; return out; }
	const hue = H < HUE_FAST_LIMIT && H > -HUE_FAST_LIMIT ? H : H % 360;
	const radians = hue * DEG_TO_RAD;
	const A = Math.cos(radians), B = Math.sin(radians);
	const L3 = L * L * L;
	const A2 = A * A,
		AB = A * B,
		A3 = A2 * A,
		A2B = A2 * B;
	const rd = rD3 * A3 + rD2 * A2B + rD1 * A + rD0 * B;
	const rb = rB0 + rB2 * A2 + rB1 * AB;
	const ra = rA1 * A + rA0 * B;
	const gd = gD3 * A3 + gD2 * A2B + gD1 * A + gD0 * B;
	const gb = gB0 + gB2 * A2 + gB1 * AB;
	const ga = gA1 * A + gA0 * B;
	const bd = bD3 * A3 + bD2 * A2B + bD1 * A + bD0 * B;
	const bb = bB0 + bB2 * A2 + bB1 * AB;
	const ba = bA1 * A + bA0 * B;
	const invL = 1 / L;
	const inputU = C * invL;
	const target = invL * invL * invL;

	// face < 0: the input is in gamut. Otherwise that channel is exactly
	// faceValue: 0 on a lower face, 1 on an upper face.
	let face = -1, faceValue = 0, r = 0, g = 0, blue = 0;
	mapped: {
		// Near white, predict one upper face from its slope and invert its quadratic.
		// This gate is only a performance heuristic; every candidate is guarded.
		const delta = target - 1;
		const maxSlope = Math.max(ra, ga, ba);
		if (delta > 0 && delta < 0.15 * maxSlope) {
			const upperFace = ra === maxSlope ? 0 : ga === maxSlope ? 1 : 2;
			const wd = upperFace === 0 ? rd : upperFace === 1 ? gd : bd;
			const wb = upperFace === 0 ? rb : upperFace === 1 ? gb : bb;
			const wa = maxSlope;
			let u = (2 * delta) / (wa + Math.sqrt(wa * wa + 4 * wb * delta));
			for (let step = 0; step < 2; step++) {
				const f = ((wd * u + wb) * u + wa) * u + 1 - target;
				if (step === 1 && Math.abs(f) <= target * 1e-12) break;
				const f1 = (3 * wd * u + 2 * wb) * u + wa;
				const halfSecond = 3 * wd * u + wb;
				const f1Squared = f1 * f1;
				const product = f * halfSecond;
				// Cancel the common factor of six in the Householder correction.
				const denominator = f1 * (f1Squared - 2 * product) + f * f * wd;
				if (denominator === 0) break;
				u -= (f * (f1Squared - product)) / denominator;
			}
			const upperR = ((rd * u + rb) * u + ra) * u + 1;
			const upperG = ((gd * u + gb) * u + ga) * u + 1;
			const upperB = ((bd * u + bb) * u + ba) * u + 1;
			const guard = target * (1 + 1e-12);
			if (u >= 0 && u < rootLimit &&
				upperR >= -1e-12 && upperG >= -1e-12 && upperB >= -1e-12 &&
				upperR <= guard && upperG <= guard && upperB <= guard &&
				Math.abs((upperFace === 0 ? upperR : upperFace === 1 ? upperG : upperB) - target) <= target * 1e-12) {
				// The two interior Bernstein controls bound each cubic between neutral and u.
				// Endpoint guards alone cannot rule out crossing another face and reentering.
				// Controls and guard are scaled by three, avoiding six divisions.
				const limit = 3 * guard;
				const linearR = ra * u, linearG = ga * u, linearB = ba * u;
				const r1 = 3 + linearR, r2 = 3 + 2 * linearR + rb * u * u;
				const g1 = 3 + linearG, g2 = 3 + 2 * linearG + gb * u * u;
				const blue1 = 3 + linearB, blue2 = 3 + 2 * linearB + bb * u * u;
				if (r1 >= 0 && r1 <= limit && r2 >= 0 && r2 <= limit &&
					g1 >= 0 && g1 <= limit && g2 >= 0 && g2 <= limit &&
					blue1 >= 0 && blue1 <= limit && blue2 >= 0 && blue2 <= limit) {
					// u is the validated first exit; classify the input against it.
					if (inputU < u) break mapped;
					face = upperFace;
					faceValue = 1;
					r = upperR;
					g = upperG;
					blue = upperB;
					break mapped;
				}
			}
			// Rejections retain the lower-first path.
		}

		face = A * red1 + B * red2 > 1 ? 0 : A * green1 + B * green2 > 1 ? 1 : 2;
		const d = face === 0 ? rd : face === 1 ? gd : bd;
		const b = face === 0 ? rb : face === 1 ? gb : bb;
		const a = face === 0 ? ra : face === 1 ? ga : ba;
		// The direction fits need one Halley step. Display-P3 has no
		// disconnected fold window; retain its invalid-root and sector guards.
		// Evaluate the seed here so JIT inlining limits do not add a helper call.
		// Retain all 18 coefficients, paired and grouped in powers of t^4 and t^8.
		let lowerSeed;
		if (face === 1) {
			const t = (B * 0.8339347326195725 - A * -0.5518630824133121 - 0) * 1.127863385513941;
			const t2 = t * t;
			const t4 = t2 * t2;
			const t8 = t4 * t4;
			const low =
				((0.5055203051802346 + -0.08974151391142265 * t) + (0.003935796602735819 + -0.0031950200183823965 * t) * t2) +
				((0.049812213498918206 + -0.014259466994389595 * t) + (-0.02608821946157984 + 0.008463197421688648 * t) * t2) * t4;
			const high =
				((0.1286484638505835 + -0.05256623072311778 * t) + (-0.2965630803407685 + 0.12662941851675352 * t) * t2) +
				((0.4318476522414585 + -0.18578934240509345 * t) + (-0.32435480435116104 + 0.13996762708411048 * t) * t2) * t4;
			lowerSeed = low + (high + (0.10458491835889717 + -0.04509353597445119 * t) * t8) * t8;
		} else if (face === 2) {
			const t = (B * 0.047079529957363205 - A * 0.9988911441488475 - 0) * 1.1747996078456162;
			const t2 = t * t;
			const t4 = t2 * t2;
			const t8 = t4 * t4;
			const low =
				((0.23589963446491738 + -0.0038895135379108737 * t) + (0.08496374354387375 + -0.0027636516463751272 * t) * t2) +
				((0.05350299282638939 + -0.00278232147399756 * t) + (-0.04400786078609473 + 0.006189289363385801 * t) * t2) * t4;
			const high =
				((0.3630035541828265 + -0.03766350558486446 * t) + (-0.8898431315961659 + 0.09460387079467442 * t) * t2) +
				((1.3164319733061864 + -0.13851221494888616 * t) + (-0.9972958875572052 + 0.10472689772454183 * t) * t2) * t4;
			lowerSeed = low + (high + (0.3252989657819146 + -0.03370793968835844 * t) * t8) * t8;
		} else if (A * -0.9400027760422874 - B * -0.3411667935670076 > 0) {
			const t = (B * -0.9518703435618234 - A * -0.3065009772374244 - 0) * 1.2655139197164653;
			const t2 = t * t;
			const t4 = t2 * t2;
			const t8 = t4 * t4;
			const low =
				((0.2282479542436939 + -0.011978541363502394 * t) + (0.07677819272243186 + -0.007787115544658625 * t) * t2) +
				((0.041419417874723675 + -0.005416858161601942 * t) + (-0.003312430161982946 + 0.00011000659405603977 * t) * t2) * t4;
			const high =
				((0.13437401745475963 + -0.018667398971906446 * t) + (-0.3108563944891688 + 0.04236335317690669 * t) * t2) +
				((0.47142535235365 + -0.06569767262273747 * t) + (-0.35860024295996273 + 0.05052013023156279 * t) * t2) * t4;
			lowerSeed = low + (high + (0.12054184701587978 + -0.017586285475974565 * t) * t8) * t8;
		} else {
			const t =
				(Math.sqrt(Math.max(0, A * -0.9954573465411122 - B * -0.09520856693243564)) - 0.29604590453444324) *
				4.900165144269026;
			const t2 = t * t;
			const t4 = t2 * t2;
			const t8 = t4 * t4;
			const low =
				((0.501535748298442 + -0.16297049180671477 * t) + (0.028564672193241766 + -0.0006869552267263243 * t) * t2) +
				((-0.0007112199414283341 + 0.00013975671324474424 * t) + (0.000014732112336762131 + -0.000008637716697358479 * t) * t2) * t4;
			const high =
				((0.0000011908160691594576 + 2.202000201931595e-7 * t) + (-7.327939657544548e-8 + 5.350645753626143e-9 * t) * t2) +
				((3.5070115700364113e-9 + -7.488678319431428e-10 * t) + (6.265407945546839e-12 + 1.4208328341030413e-10 * t) * t2) * t4;
			lowerSeed = low + (high + (-5.35591324377391e-12 + -2.2838422511186864e-11 * t) * t8) * t8;
		}
		let saturation = polish(lowerSeed, d, b, a);
		let lowerR = face === 0 ? 0 : ((rd * saturation + rb) * saturation + ra) * saturation + 1;
		let lowerG = face === 1 ? 0 : ((gd * saturation + gb) * saturation + ga) * saturation + 1;
		let lowerB = face === 2 ? 0 : ((bd * saturation + bb) * saturation + ba) * saturation + 1;
		if (
			!(saturation > 0 && saturation < rootLimit) ||
			lowerR < -1e-12 ||
			lowerG < -1e-12 ||
			lowerB < -1e-12
		) {
			const r = firstRoot(rd, rb, ra, 1, rootLimit);
			const g = firstRoot(gd, gb, ga, 1, rootLimit);
			const blue = firstRoot(bd, bb, ba, 1, rootLimit);
			saturation = Math.min(r, g, blue);
			face = saturation === r ? 0 : saturation === g ? 1 : 2;
			lowerR = ((rd * saturation + rb) * saturation + ra) * saturation + 1;
			lowerG = ((gd * saturation + gb) * saturation + ga) * saturation + 1;
			lowerB = ((bd * saturation + bb) * saturation + ba) * saturation + 1;
		}

		if (!(lowerR > target || lowerG > target || lowerB > target)) {
			if (inputU < saturation) {
				face = -1;
				break mapped;
			}
			r = lowerR;
			g = lowerG;
			blue = lowerB;
			break mapped;
		}

		// Predict the upper face from the largest channel at the lower root.
		face = lowerR > lowerG ? (lowerR > lowerB ? 0 : 2) : lowerG > lowerB ? 1 : 2;
		const wd = face === 0 ? rd : face === 1 ? gd : bd;
		const wb = face === 0 ? rb : face === 1 ? gb : bb;
		const wa = face === 0 ? ra : face === 1 ? ga : ba;
		const maxLower = face === 0 ? lowerR : face === 1 ? lowerG : lowerB;
		let u = (saturation * (target - 1)) / (maxLower - 1);
		// Two fourth-order Householder steps from the neutral-to-lower chord.
		// Each step uses one division. The guard below checks both convergence
		// and the other faces; the predictor is never an accuracy guarantee.
		for (let step = 0; step < 2; step++) {
			const f = ((wd * u + wb) * u + wa) * u + 1 - target;
			const f1 = (3 * wd * u + 2 * wb) * u + wa;
			const halfSecond = 3 * wd * u + wb;
			const f1Squared = f1 * f1;
			const product = f * halfSecond;
			// Cancel the common factor of six in the Householder correction.
			const denominator = f1 * (f1Squared - 2 * product) + f * f * wd;
			if (denominator === 0) break;
			u -= (f * (f1Squared - product)) / denominator;
		}
		r = ((rd * u + rb) * u + ra) * u + 1;
		g = ((gd * u + gb) * u + ga) * u + 1;
		blue = ((bd * u + bb) * u + ba) * u + 1;
		const guard = target * (1 + 1e-12);
		if (
			!(
				u >= 0 &&
				u <= saturation &&
				r >= -1e-12 &&
				g >= -1e-12 &&
				blue >= -1e-12 &&
				r <= guard &&
				g <= guard &&
				blue <= guard &&
				Math.abs((face === 0 ? r : face === 1 ? g : blue) - target) <= target * 1e-12
			)
		) {
			// Retry only a converged, in-range prediction rejected by another upper face.
			// Keep the first-root fallback for every other failure and failed retries.
			if (u >= 0 && u <= saturation && r >= -1e-12 && g >= -1e-12 && blue >= -1e-12 &&
				Math.abs((face === 0 ? r : face === 1 ? g : blue) - target) <= target * 1e-12 &&
				(r > guard || g > guard || blue > guard)) {
				// UPPER_FACE_RETRY
				face = r > g ? (r > blue ? 0 : 2) : g > blue ? 1 : 2;
				const retryD = face === 0 ? rd : face === 1 ? gd : bd;
				const retryB = face === 0 ? rb : face === 1 ? gb : bb;
				const retryA = face === 0 ? ra : face === 1 ? ga : ba;
				const previousU = u;
				for (let step = 0; step < 2; step++) {
					const f = ((retryD * u + retryB) * u + retryA) * u + 1 - target;
					const f1 = (3 * retryD * u + 2 * retryB) * u + retryA;
					const halfSecond = 3 * retryD * u + retryB;
					const f1Squared = f1 * f1;
					const product = f * halfSecond;
					const denominator = f1 * (f1Squared - 2 * product) + f * f * retryD;
					if (denominator === 0) break;
					u -= f * (f1Squared - product) / denominator;
				}
				r = ((rd * u + rb) * u + ra) * u + 1;
				g = ((gd * u + gb) * u + ga) * u + 1;
				blue = ((bd * u + bb) * u + ba) * u + 1;
				// A retry must move toward neutral, never past the rejected candidate.
				if (!(u >= 0 && u <= previousU)) u = NaN;
			}
			if (!(u >= 0 && u <= saturation && r >= -1e-12 && g >= -1e-12 && blue >= -1e-12 &&
				r <= guard && g <= guard && blue <= guard &&
				Math.abs((face === 0 ? r : face === 1 ? g : blue) - target) <= target * 1e-12)) {
				const ru = firstRoot(rd, rb, ra, 1 - target, saturation);
				const gu = firstRoot(gd, gb, ga, 1 - target, saturation);
				const bu = firstRoot(bd, bb, ba, 1 - target, saturation);
				u = Math.min(ru, gu, bu);
				face = u === ru ? 0 : u === gu ? 1 : 2;
				r = ((rd * u + rb) * u + ra) * u + 1;
				g = ((gd * u + gb) * u + ga) * u + 1;
				blue = ((bd * u + bb) * u + ba) * u + 1;
			}
		}
		// Classify against the validated boundary, including after fallback.
		if (inputU < u) face = -1;
		else faceValue = 1;
	}

	if (face < 0) {
		r = ((rd * inputU + rb) * inputU + ra) * inputU + 1;
		g = ((gd * inputU + gb) * inputU + ga) * inputU + 1;
		blue = ((bd * inputU + bb) * inputU + ba) * inputU + 1;
	}
	out[0] = face === 0 ? faceValue : clampedGamma(L3 * r);
	out[1] = face === 1 ? faceValue : clampedGamma(L3 * g);
	out[2] = face === 2 ? faceValue : clampedGamma(L3 * blue);
	return out;
}
