// The derivative bound between 0 and 2*x proves a unique root there.
// Test conditioning rather than an arbitrary tiny-root cutoff.
export function conditionedFirstRoot (a, b, c, d, hi) {
	if (d === 0 || c === 0) return;
	// A conservative necessary condition avoids division on clear failures.
	if (Math.abs(b * d) > 0.25 * c * c) return;
	let x = -d / c;
	if (!(x !== 0 && Number.isFinite(x)) || (12 * Math.abs(a) * Math.abs(x) + 4 * Math.abs(b)) * Math.abs(x) > 0.5 * Math.abs(c)) return;
	for (let i = 0; i < 6; i++) {
		const next = x - (((a * x + b) * x + c) * x + d) / ((3 * a * x + 2 * b) * x + c);
		if (next === x) break;
		x = next;
	}
	if (x > 0) return x <= hi ? x : Infinity;
	// Deflate a negative root before selecting the next positive root.
	const bb = b + a * x, cc = c + bb * x;
	let r0 = Infinity, r1 = Infinity;
	if (a === 0) r0 = -cc / bb;
	else {
		const disc = bb * bb - 4 * a * cc;
		if (disc >= 0) {
			const q = -0.5 * (bb + (bb < 0 ? -Math.sqrt(disc) : Math.sqrt(disc)));
			r0 = q / a; r1 = cc / q;
		}
	}
	return Math.min(r0 > 0 && r0 <= hi ? r0 : Infinity, r1 > 0 && r1 <= hi ? r1 : Infinity);
}

export function faceExit (a, b, c, upper, hi) {
	const direction = c !== 0 ? c : b !== 0 ? b : a;
	if (upper ? direction > 0 : direction < 0) return 0;
	return firstRoot(0, a, b, c, Number.MIN_VALUE, hi);
}

// Smallest real root of a·t³ + b·t² + c·t + d in (lo, hi), or Infinity if none.
export function firstRoot (a, b, c, d, lo, hi) {
	if (lo === 0) {
		if (d === 0) return faceExit(a, b, c, true, hi);
		const root = conditionedFirstRoot(a, b, c, d, hi);
		if (root !== undefined) return root;
	}
	let r0 = Infinity, r1 = Infinity, r2 = Infinity;

	if (Math.abs(a) < 1e-12) {
		if (Math.abs(b) < 1e-12) {
			if (Math.abs(c) >= 1e-12) {
				r0 = -d / c;
			}
		}
		else {
			const disc = c * c - 4 * b * d;
			if (disc >= 0) {
				const s = Math.sqrt(disc);
				r0 = (-c + s) / (2 * b);
				r1 = (-c - s) / (2 * b);
			}
		}
	}
	else {
		b /= a; c /= a; d /= a;
		const p = c - b * b / 3;
		const q = 2 * b * b * b / 27 - b * c / 3 + d;
		const off = -b / 3;
		const disc = q * q / 4 + p * p * p / 27;

		if (disc > 1e-14) {
			const s = Math.sqrt(disc);
			r0 = Math.cbrt(-q / 2 + s) + Math.cbrt(-q / 2 - s) + off;
		}
		else if (disc > -1e-14) {
			const u = Math.cbrt(-q / 2);
			r0 = 2 * u + off;
			r1 = -u + off;
		}
		else {
			const m = 2 * Math.sqrt(-p / 3);
			const phi = Math.acos(Math.max(-1, Math.min(1, 3 * q / (p * m))));
			r0 = m * Math.cos(phi / 3) + off;
			r1 = m * Math.cos((phi - 2 * Math.PI) / 3) + off;
			r2 = m * Math.cos((phi - 4 * Math.PI) / 3) + off;
		}
	}

	let best = Infinity;
	if (r0 > lo && r0 < hi) {
		best = r0;
	}
	if (r1 > lo && r1 < hi && r1 < best) {
		best = r1;
	}
	if (r2 > lo && r2 < hi && r2 < best) {
		best = r2;
	}
	return best;
}
