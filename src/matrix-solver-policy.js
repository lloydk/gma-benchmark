// Algorithm policy, separate from physical RGB descriptors. Endpoints match
// Rust rgb_spaces::blue_fold_window's binary32 endpoints in both lanes.
import { blueFoldWindows as WINDOWS } from "./generated/matrix-solver-policy.js";
export function blueFoldWindow (space) {
	if (!Object.hasOwn(WINDOWS, space.id)) throw new RangeError(`No solver policy for ${space.id}`);
	return WINDOWS[space.id];
}
export function inBlueFold (h, window) {
	if (!window) return false;
	if (!(h > -360 && h < 360)) h %= 360;
	return (h >= window[0] && h <= window[1]) || (h >= window[0] - 360 && h <= window[1] - 360);
}

// Compensated Horner: cancellation must not pick a disconnected blue interval.
// Dekker's split recovers the product residual using binary64 arithmetic only.
export function evaluateFoldPolynomial (p, x) {
	let value = p[0], error = 0;
	for (let i = 1; i < 4; i++) {
		const coefficient = p[i];
		const product = value * x;
		const av = 134217729 * value, ax = 134217729 * x;
		const vh = av - (av - value), xh = ax - (ax - x);
		const vl = value - vh, xl = x - xh;
		const residual = ((vh * xh - product) + vh * xl + vl * xh) + vl * xl;
		const sum = product + coefficient;
		const z = sum - product;
		error = error * x + (residual + ((product - (sum - z)) + (coefficient - z)));
		value = sum;
	}
	return value + error;
}

// Port of Rust's outer_fold_boundary. Partition at derivative roots, then
// consider outward face crossings up to the input, retaining the last feasible one.
// The ordinary iteration stays allocation-free; this rare fold path uses
// small temporary arrays outside the timed integer-hue grid's hot path.
export function outerFoldBoundary (rows, L, q0, q1, q2, inputC = 0.5) {
	const polynomials = rows.map(([r, g, b]) => [
		r * q0 * q0 * q0 + g * q1 * q1 * q1 + b * q2 * q2 * q2,
		3 * (r * q0 * q0 + g * q1 * q1 + b * q2 * q2),
		3 * (r * q0 + g * q1 + b * q2), r + g + b,
	]);
	const white = 1 / (L * L * L), limit = Math.min(0.5, inputC) / L;
	// Greatest feasible chroma <= input, including disconnected re-entry islands.
	// An input in a gap must fall back to the preceding outward exit.
	if (polynomials.every(p => {
		const v = evaluateFoldPolynomial(p, limit);
		return v >= 0 && v <= white;
	})) return Math.min(0.5, inputC);
	let best = 0;
	for (const [a, b, c, d] of polynomials) {
		const points = [0, limit];
		const disc = b * b - 3 * a * c;
		if (a !== 0 && disc >= 0) {
			const v = -b - (b < 0 ? -Math.sqrt(disc) : Math.sqrt(disc));
			for (const x of [v / (3 * a), c / v]) if (x > 0 && x < limit) points.push(x);
		}
		else if (a === 0 && b !== 0) {
			const x = -c / (2 * b);
			if (x > 0 && x < limit) points.push(x);
		}
		points.sort((a, b) => a - b);
		for (const face of [0, white]) {
			const p = [a, b, c, d - face];
			for (let i = 1; i < points.length; i++) {
				let lo = points[i - 1], hi = points[i];
				const fl = evaluateFoldPolynomial(p, lo), fh = evaluateFoldPolynomial(p, hi);
				if (face === 0 ? !(fl >= 0 && fh < 0) : !(fl <= 0 && fh > 0)) continue;
				// The 1/L bracket needs exponent-range halvings near black,
				// then a mantissa of refinement; normally adjacency stops early.
				for (let n = 0; n < 1077; n++) {
					const mid = lo + (hi - lo) * 0.5;
					if (mid === lo || mid === hi) break;
					if ((evaluateFoldPolynomial(p, mid) < 0) === (fl < 0)) lo = mid;
					else hi = mid;
				}
				const x = lo + (hi - lo) * 0.5;
				if (polynomials.every(p => {
					const v = evaluateFoldPolynomial(p, x);
					const scale = ((Math.abs(p[0]) * x + Math.abs(p[1])) * x + Math.abs(p[2])) * x + Math.abs(p[3]);
					const slack = 8 * Number.EPSILON * scale;
					return v >= -slack && v <= white + slack;
				})) best = Math.max(best, x);
			}
		}
	}
	return L * best;
}
