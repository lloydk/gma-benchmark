// Independent first-exit reference in physical chroma: isolate stationary
// intervals and bisect. No production sector, fitted seed, or root iteration.
import { KA0, KB0, KA1, KB1, KA2, KB2, RL, RM, RS, GL, GM, GS, BL, BM, BS, clampedGamma } from "../../src/convert.js";
const DEG_TO_RAD = Math.PI / 180;
const MAX_CHROMA = 0.5;
const BISECTION_STEPS = 56;
const ROWS = [[RL, RM, RS], [GL, GM, GS], [BL, BM, BS]];

function polynomial (a, b, c, d, x) {
	return ((a * x + b) * x + c) * x + d;
}

function evaluationTolerance (a, b, c, d, x) {
	const scale = Math.abs(a * x * x * x) + Math.abs(b * x * x) + Math.abs(c * x) + Math.abs(d);
	return Number.EPSILON * 64 * scale;
}

// Smallest root in [0, hi]. Derivative roots partition a cubic into monotonic
// intervals, so sign-changing interval endpoints can be safely bisected. A
// root at a stationary endpoint is retained as a tangent boundary contact.
function firstRootByBisection (a, b, c, d, hi) {
	const points = [0];
	if (a !== 0) {
		const discriminant = 4 * b * b - 12 * a * c;
		if (discriminant >= 0) {
			const root = Math.sqrt(discriminant);
			const p0 = (-2 * b - root) / (6 * a);
			const p1 = (-2 * b + root) / (6 * a);
			if (p0 > 0 && p0 < hi) points.push(p0);
			if (p1 > 0 && p1 < hi && p1 !== p0) points.push(p1);
		}
	}
	else if (b !== 0) {
		const p = -c / (2 * b);
		if (p > 0 && p < hi) points.push(p);
	}
	points.push(hi);
	points.sort((x, y) => x - y);

	let left = points[0];
	let leftValue = polynomial(a, b, c, d, left);
	for (let index = 1; index < points.length; index++) {
		let right = points[index];
		let rightValue = polynomial(a, b, c, d, right);
		if (Math.abs(leftValue) <= evaluationTolerance(a, b, c, d, left)) return left;
		if (leftValue * rightValue < 0 || Math.abs(rightValue) <= evaluationTolerance(a, b, c, d, right)) {
			if (Math.abs(rightValue) <= evaluationTolerance(a, b, c, d, right)) return right;
			for (let iteration = 0; iteration < BISECTION_STEPS; iteration++) {
				const middle = (left + right) * 0.5;
				const middleValue = polynomial(a, b, c, d, middle);
				if ((leftValue < 0) === (middleValue < 0)) {
					left = middle;
					leftValue = middleValue;
				}
				else {
					right = middle;
					rightValue = middleValue;
				}
			}
			return (left + right) * 0.5;
		}
		left = right;
		leftValue = rightValue;
	}
	return Infinity;
}

export function exactBoundary (L, H, out) {
	const radians = H * DEG_TO_RAD;
	const hueA = Math.cos(radians), hueB = Math.sin(radians);
	const q0 = KA0 * hueA + KB0 * hueB;
	const q1 = KA1 * hueA + KB1 * hueB;
	const q2 = KA2 * hueA + KB2 * hueB;
	const q0Squared = q0 * q0, q1Squared = q1 * q1, q2Squared = q2 * q2;
	const q0Cubed = q0Squared * q0, q1Cubed = q1Squared * q1, q2Cubed = q2Squared * q2;
	const L2 = L * L, L3 = L2 * L;
	let boundary = MAX_CHROMA;

	for (const [w0, w1, w2] of ROWS) {
		const a = w0 * q0Cubed + w1 * q1Cubed + w2 * q2Cubed;
		const b = 3 * L * (w0 * q0Squared + w1 * q1Squared + w2 * q2Squared);
		const c = 3 * L2 * (w0 * q0 + w1 * q1 + w2 * q2);
		const d = L3 * (w0 + w1 + w2);
		boundary = Math.min(
			boundary,
			firstRootByBisection(a, b, c, d, boundary),
			firstRootByBisection(a, b, c, d - 1, boundary),
		);
	}
	if (!(boundary < MAX_CHROMA)) {
		throw new Error(`reference found no boundary below C=${MAX_CHROMA} at L=${L}, H=${H}`);
	}

	const l0 = L + boundary * q0;
	const m0 = L + boundary * q1;
	const s0 = L + boundary * q2;
	const l = l0 * l0 * l0, m = m0 * m0 * m0, s = s0 * s0 * s0;
	out[0] = clampedGamma(RL * l + RM * m + RS * s);
	out[1] = clampedGamma(GL * l + GM * m + GS * s);
	out[2] = clampedGamma(BL * l + BM * m + BS * s);
	return boundary;
}
