import { createReference } from "./css-minde-reference.js";

// Independent physical XYZ coefficients and stationary-interval bisection.
// No production matrix, polynomial root finder or iterative solve is used.
export function createBoundaryReference (id) {
	const ref = createReference(id);
	const columns = [[1,0,0], [0,1,0], [0,0,1]].map(ref.lmsToRgb);
	const rows = columns.map((_, i) => columns.map(column => column[i]));
	function boundaries (L, H) {
		const rad = H * Math.PI / 180;
		const q = ref.labToLms([0, Math.cos(rad), Math.sin(rad)]);
		const polys = rows.map(row => [
			row.reduce((s, w, i) => s + w * q[i] ** 3, 0),
			3 * L * row.reduce((s, w, i) => s + w * q[i] ** 2, 0),
			3 * L * L * row.reduce((s, w, i) => s + w * q[i], 0),
			L ** 3 * row.reduce((s, w) => s + w, 0),
		]);
		if (polys.some(p => p[3] < 0 || p[3] > 1)) return { first: 0, outer: 0, exits: [] };
		const evalP = (p, x) => ((p[0] * x + p[1]) * x + p[2]) * x + p[3];
		const exits = [];
		for (let channel = 0; channel < 3; channel++) {
			const [a,b,c,d] = polys[channel];
			const knots = [0, .5], disc = b*b - 3*a*c;
			if (disc >= 0 && a !== 0) {
				for (const x of [(-b - Math.sqrt(disc))/(3*a), (-b + Math.sqrt(disc))/(3*a)]) if (x > 0 && x < .5) knots.push(x);
			}
			else if (a === 0 && b !== 0) {
				const x = -c/(2*b); if (x > 0 && x < .5) knots.push(x);
			}
			knots.sort((a,b) => a-b);
			for (const face of [0,1]) {
				const p = [a,b,c,d-face];
				for (let i = 1; i < knots.length; i++) {
					let lo = knots[i-1], hi = knots[i];
					const fl = evalP(p, lo), fh = evalP(p, hi);
					if (face === 0 ? !(fl >= 0 && fh < 0) : !(fl <= 0 && fh > 0)) continue;
					for (let k = 0; k < 100; k++) {
						const mid = (lo+hi)/2;
						if (mid === lo || mid === hi) break;
						if ((evalP(p, mid) < 0) === (fl < 0)) lo = mid; else hi = mid;
					}
					const chroma = (lo+hi)/2;
					const feasible = polys.every(p => {
						const v = evalP(p, chroma);
						const scale = ((Math.abs(p[0])*chroma+Math.abs(p[1]))*chroma+Math.abs(p[2]))*chroma+Math.abs(p[3]);
						const slack = 64*Number.EPSILON*scale;
						return v >= -slack && v <= 1+slack;
					});
					exits.push({ chroma, channel, face, feasible });
				}
			}
		}
		const first = Math.min(.5, ...exits.map(e => e.chroma));
		const outer = Math.max(0, ...exits.filter(e => e.feasible).map(e => e.chroma));
		return { first, outer, exits };
	}
	function raytrace (input) {
		const [L,C,H] = input;
		if (L <= 0 || L >= 1) return Array(3).fill(L <= 0 ? 0 : 1);
		if (C <= 0) return ref.linearRgb([L,0,H]);
		const gray = L*L*L;
		if (gray === 0) return [0,0,0];
		let target = ref.linearRgb(input), anchor = [gray,gray,gray], last = target;
		const norm = v => Math.max(...v.map(Math.abs));
		for (let i = 0; i < 4; i++) {
			if (i) {
				const lab = ref.linearToLab(target);
				target = ref.linearRgb([L,Math.hypot(lab[1],lab[2]),H]);
			}
			const direction = target.map((v,j) => v-anchor[j]);
			if (i && (norm(direction) <= 32*Number.EPSILON*norm(anchor)
				|| norm(target.map((v,j) => v-last[j])) <= 32*Number.EPSILON*norm(last))) break;
			let distance = Infinity;
			for (let j = 0; j < 3; j++) {
				const d = direction[j];
				if (d === 0) continue;
				distance = Math.min(distance, ((d > 0 ? 1 : 0)-anchor[j])/d);
			}
			if (!Number.isFinite(distance)) break;
			last = anchor.map((v,j) => v+direction[j]*distance);
			if (i && target.every(v => v > 1e-12 && v < 1-1e-12)) anchor = target;
			target = last;
		}
		return last.map(v => Math.max(0,Math.min(1,v)));
	}
	function foldChroma (L, C, H, edges = boundaries(L,H)) {
		if (ref.linearRgb([L,C,H]).every(v => v >= 0 && v <= 1)) return C;
		return Math.max(0, ...edges.exits.filter(e => e.feasible && e.chroma <= C).map(e => e.chroma));
	}
	return { ...ref, boundaries, foldChroma, raytrace };
}
