// Shared numeric helpers for the generated Dualray mappers.
export function value (d, b, a, x) {
	return ((d * x + b) * x + a) * x + 1;
}

export function polish (x, d, b, a) {
	const dx=d*x,q=dx+b,r=q*x+a;
	const f=r*x+1,halfSecond=(dx+dx)+q,f1=(dx+q)*x+r;
	const denominator=f1*f1-f*halfSecond;
	return denominator !== 0 ? x-(f*f1)/denominator : x;
}

// First exit along u in [0, limit] from 0 <= channel <= target, for rows
// [d, b, a] of normalized cubics. Every channel is monotone between
// consecutive stationary points, so the first infeasible breakpoint and the
// breakpoint before it bracket the exit, and feasibility is monotone inside.
// A tangent touch stays feasible: it does not leave the gamut. Returns
// [last feasible u, first infeasible u], the second undefined when nothing
// exits. Matches Rust.
export function firstExit (rows, target, limit) {
	const points = [limit];
	for (let k = 0; k < 3; k++) {
		// Roots of the derivative 3d·u² + 2b·u + a.
		const d = rows[k][0], b = rows[k][1], a = rows[k][2];
		const disc = b * b - 3 * d * a;
		if (d !== 0 && disc >= 0) {
			const root = Math.sqrt(disc);
			points.push((-b - root) / (3 * d), (-b + root) / (3 * d));
		} else if (d === 0 && b !== 0) points.push(-a / (2 * b));
	}
	let lo = 0, hi = Infinity;
	for (const u of points) if (u > 0 && u < hi && u <= limit && !feasible(rows, target, u)) hi = u;
	if (hi === Infinity) return [limit, undefined];
	for (const u of points) if (u > lo && u < hi) lo = u;
	for (;;) {
		const mid = lo + (hi - lo) * 0.5;
		if (mid <= lo || mid >= hi) return [lo, hi];
		if (feasible(rows, target, mid)) lo = mid;
		else hi = mid;
	}
}

function feasible (rows, target, u) {
	for (let k = 0; k < 3; k++) {
		const v = value(rows[k][0], rows[k][1], rows[k][2], u);
		if (!(v >= 0 && v <= target)) return false;
	}
	return true;
}

// Cold path: the exact first exit bounded by the input, for blue-fold hues
// (where re-entry islands exist and fitted roots do not apply) and any
// rejected guard. An interior input keeps its normalized conversion; an exit
// writes the crossed channel exactly on its face, as does an input exactly on
// the upper face (L³·target need not round to one).
export function search (rows, l3, target, limit, encode, out) {
	const [u, beyond] = firstExit(rows, target, limit);
	let face = -1, faceValue = 0;
	if (beyond !== undefined) {
		for (let k = 0; k < 3; k++) {
			const v = value(rows[k][0], rows[k][1], rows[k][2], beyond);
			if (v < 0 || v > target) {
				face = k;
				faceValue = v > target ? 1 : 0;
				break;
			}
		}
	}
	for (let k = 0; k < 3; k++) {
		const v = value(rows[k][0], rows[k][1], rows[k][2], u);
		out[k] = k === face ? faceValue : v >= target ? 1 : encode(l3 * v);
	}
	return out;
}
