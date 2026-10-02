// Prototype data for Dualray Fast: per-sector hue polynomials of the lower
// face, fitted from exact first roots (independent of Dualray's seeds).
//
// Below the cusp the constant-L/h first exit lies on the lower face, where
// one channel is zero and the other two are L³·g_k(h). The boundary ratio
// u(h) = C/L and g_k(h) depend on hue only, so an out-of-gamut OKLCh color
// below the cusp maps without trig, the 3×3 conversion or a root solve.
//
// Each gamut has three sectors, one per zero channel, split at the
// primaries. The red-zero sector ends at a fold of the red root (P3: just
// past the sector; sRGB/Rec.2020: inside it, at the blue-fold window), so it
// is fitted in w = sqrt(hFold − h). Hues in the blue-fold window are left to
// the exact solver.
//
// Writes rust/src/generated/dualray_fast_*.rs and src/generated/dualray-fast.js.
// Usage: node scripts/generate-dualray-fast.mjs [--check]
import assert from "node:assert/strict";
import { readFileSync, writeFileSync } from "node:fs";
import { SRGB, DISPLAY_P3, REC2020 } from "../src/rgb-spaces.js";
import { LL, LM, LS, AL, AM, AS, BL, BM, BS } from "../src/oklab.js";
import { blueFoldWindows } from "../src/generated/matrix-solver-policy.js";

const check = process.argv.includes("--check");
assert(process.argv.slice(2).every(v => v === "--check"));

// Fit budgets. OUTPUT_BUDGET is the direct output's ΔEOK (runtime target:
// 1e-3 max, 1e-4 p99). BOUNDARY_BUDGET is the boundary ratio's error in
// chroma; with the upward margin it only sets how many colors near the
// boundary fall back to the exact solver, not the output accuracy.
const OUTPUT_BUDGET = Number(process.env.OUTPUT_BUDGET ?? 1e-4);
const BOUNDARY_BUDGET = Number(process.env.BOUNDARY_BUDGET ?? 2e-3);
const MAX_DEGREE = 15;
const DEG = Math.PI / 180;

function model (space) {
	const K = space.oklabToLms.map(r => [r[1], r[2]]);
	const M = space.lmsToRgb;
	// p_k(u) = 1 + A u + B u² + D u³ along a constant-hue ray, u = C/L.
	function cubics (h) {
		const c = Math.cos(h * DEG), s = Math.sin(h * DEG);
		const k = K.map(([ka, kb]) => ka * c + kb * s);
		return M.map(w => [
			3 * (w[0] * k[0] + w[1] * k[1] + w[2] * k[2]),
			3 * (w[0] * k[0] ** 2 + w[1] * k[1] ** 2 + w[2] * k[2] ** 2),
			w[0] * k[0] ** 3 + w[1] * k[1] ** 3 + w[2] * k[2] ** 3,
		]);
	}
	const value = ([A, B, D], u) => 1 + u * (A + u * (B + u * D));
	// First positive root: partition at stationary points, then bisect.
	function firstRoot (q, limit = 16) {
		const [A, B, D] = q;
		const points = [0, limit];
		const disc = B * B - 3 * D * A;
		if (D !== 0 && disc >= 0) {
			for (const s of [(-B - Math.sqrt(disc)) / (3 * D), (-B + Math.sqrt(disc)) / (3 * D)])
				if (s > 0 && s < limit) points.push(s);
		}
		points.sort((x, y) => x - y);
		for (let i = 1; i < points.length; i++) {
			let lo = points[i - 1], hi = points[i];
			if (value(q, hi) > 0) continue;
			for (;;) {
				const mid = lo + (hi - lo) / 2;
				if (mid <= lo || mid >= hi) return mid;
				if (value(q, mid) > 0) lo = mid; else hi = mid;
			}
		}
		return Infinity;
	}
	// Lower-face data with a fixed zero channel.
	function lower (h, zero) {
		const q = cubics(h);
		const u = firstRoot(q[zero]);
		const g = q.map(c => value(c, u));
		g[zero] = 0;
		return { u, g };
	}
	// Fold: the channel's local minimum touches zero.
	function localMin (q) {
		const [A, B, D] = q;
		const disc = B * B - 3 * D * A;
		if (disc < 0) return Infinity;
		let best = Infinity;
		for (const u of [(-B - Math.sqrt(disc)) / (3 * D), (-B + Math.sqrt(disc)) / (3 * D)])
			if (u > 0 && 2 * B + 6 * D * u > 0) best = Math.min(best, value(q, u));
		return best;
	}
	function fold (zero, from) {
		const m = h => localMin(cubics(h)[zero]);
		for (let h = from; h < from + 4; h += 0.005) {
			if (m(h) < 0 && m(h + 0.005) > 0) {
				let a = h, b = h + 0.005;
				for (let i = 0; i < 200; i++) {
					const c = (a + b) / 2;
					if (c <= a || c >= b) break;
					if (m(c) < 0) a = c; else b = c;
				}
				return a;
			}
		}
		return null;
	}
	// Exact zero channel of the first lower exit (free choice).
	function zeroChannel (h) {
		const q = cubics(h);
		let best = Infinity, zero = -1;
		q.forEach((c, k) => { const r = firstRoot(c); if (r < best) { best = r; zero = k; } });
		return zero;
	}
	// Linear RGB to OKLab, for ΔEOK.
	const R2L = space.rgbToLms;
	function lab (rgb) {
		const [l, m, s] = R2L.map(r => Math.cbrt(r[0] * rgb[0] + r[1] * rgb[1] + r[2] * rgb[2]));
		return [LL * l + LM * m + LS * s, AL * l + AM * m + AS * s, BL * l + BM * m + BS * s];
	}
	return { cubics, value, firstRoot, lower, fold, zeroChannel, lab };
}

function chebyshev (f, degree) {
	const n = degree + 1;
	const ys = Array.from({ length: n }, (_, k) => f(Math.cos(Math.PI * (k + 0.5) / n)));
	return Array.from({ length: n }, (_, j) =>
		(j === 0 ? 1 : 2) / n * ys.reduce((s, y, k) => s + y * Math.cos(j * Math.PI * (k + 0.5) / n), 0));
}
// Chebyshev series to monomials in t ∈ [−1, 1].
function monomial (c) {
	const T = [[1], [0, 1]];
	for (let k = 2; k < c.length; k++) {
		const next = [0, ...T[k - 1].map(x => 2 * x)];
		T[k - 2].forEach((x, i) => { next[i] -= x; });
		T.push(next);
	}
	const p = new Array(c.length).fill(0);
	c.forEach((cj, j) => T[j].forEach((x, i) => { p[i] += cj * x; }));
	return p;
}
const horner = (p, t) => p.reduceRight((s, c) => s * t + c, 0);

function generate (space) {
	const m = model(space);
	const window = blueFoldWindows[space.id];
	// Sector edges: refine each change of zero channel by bisection on hue.
	const edges = [];
	let previous = m.zeroChannel(0);
	for (let i = 1; i <= 3600; i++) {
		const h = i / 10;
		const z = m.zeroChannel(h % 360);
		if (z !== previous) {
			let a = h - 0.1, b = h;
			for (let k = 0; k < 200; k++) {
				const c = (a + b) / 2;
				if (c <= a || c >= b) break;
				if (m.zeroChannel(c) === previous) a = c; else b = c;
			}
			edges.push({ hue: b, from: previous, to: z });
			previous = z;
		}
	}
	assert.equal(edges.length, 3, `${space.id}: expected three lower sectors`);
	const sectors = edges.map((edge, i) => {
		const next = edges[(i + 1) % 3];
		let end = next.hue;
		if (end <= edge.hue) end += 360;
		return { zero: edge.to, start: edge.hue, end };
	});
	// Order by zero channel: blue (from red primary), red, green (wraps).
	sectors.sort((a, b) => a.start - b.start);
	assert.deepEqual(sectors.map(s => s.zero), [2, 0, 1], `${space.id}: unexpected sector order`);

	const L_SAMPLES = Array.from({ length: 64 }, (_, i) => (i + 1) / 64);
	const out = sectors.map(sector => {
		const { zero } = sector;
		const channels = [0, 1, 2].filter(k => k !== zero);
		let { start, end } = sector;
		let hFold = null;
		if (zero === 0) {
			hFold = m.fold(0, end - 1);
			assert(hFold !== null, `${space.id}: no red fold near ${end}`);
			if (hFold < end) {
				// Fold inside the sector: stop at the blue-fold window.
				assert(window && window[0] < hFold && window[1] >= end,
					`${space.id}: fold ${hFold} not covered by window ${window}`);
				end = window[0];
			}
		}
		const fit = hFold === null
			? { v: h => h, scale: 2 / (end - start), offset: -(end + start) / (end - start) }
			: (() => {
				const w0 = Math.sqrt(hFold - start), w1 = Math.sqrt(hFold - end);
				return { v: h => Math.sqrt(hFold - h), scale: 2 / (w0 - w1), offset: -(w0 + w1) / (w0 - w1) };
			})();
		const toT = h => fit.scale * fit.v(h) + fit.offset;
		const toH = hFold === null
			? t => (t - fit.offset) / fit.scale
			: t => { const w = (t - fit.offset) / fit.scale; return hFold - w * w; };
		const cache = new Map();
		const at = h => {
			let v = cache.get(h);
			if (!v) { v = m.lower(((h % 360) + 360) % 360, zero); cache.set(h, v); }
			return v;
		};
		// Dense verification grid, including both ends.
		const grid = [];
		const n = Math.ceil((end - start) / 0.01);
		for (let i = 0; i <= n; i++) {
			const h = start + (end - start) * i / n;
			const ex = at(h);
			const Lc = Math.max(...ex.g) ** (-1 / 3);
			grid.push({ h, t: toT(h), ex, Lc });
		}
		// Boundary ratio u(h): smallest degree within budget, then a margin
		// so that û + margin ≥ u on the grid (never claims an inside color).
		let u;
		for (let degree = 4; degree <= MAX_DEGREE; degree++) {
			const p = monomial(chebyshev(t => at(toH(t)).u, degree));
			let worst = 0, under = 0;
			for (const { t, ex, Lc } of grid) {
				const e = horner(p, t) - ex.u;
				worst = Math.max(worst, Math.abs(e) * Lc);
				under = Math.max(under, -e);
			}
			u = { degree, p, error: worst, margin: under * 1.25 + 1e-12 };
			if (worst <= BOUNDARY_BUDGET) break;
		}
		// Channels: smallest common degree whose direct output is within budget.
		let g;
		for (let degree = 4; degree <= MAX_DEGREE; degree++) {
			const ps = channels.map(k => monomial(chebyshev(t => at(toH(t)).g[k], degree)));
			let worst = 0;
			for (const { t, ex, Lc } of grid) {
				const fitted = [0, 0, 0], exact = [0, 0, 0];
				channels.forEach((k, i) => { fitted[k] = horner(ps[i], t); exact[k] = ex.g[k]; });
				for (const L of L_SAMPLES) {
					if (L > Lc) break;
					const L3 = L * L * L;
					const a = m.lab(fitted.map(x => Math.min(1, Math.max(0, L3 * x))));
					const b = m.lab(exact.map(x => L3 * x));
					worst = Math.max(worst, Math.hypot(a[0] - b[0], a[1] - b[1], a[2] - b[2]));
				}
			}
			g = { degree, ps, error: worst };
			if (worst <= OUTPUT_BUDGET) break;
		}
		return { zero, channels, start, end, hFold, scale: fit.scale, offset: fit.offset, u, g };
	});
	return { space, window, sectors: out };
}

const rust = x => {
	const s = String(x);
	return /[.eE]/.test(s) || !Number.isFinite(x) ? s : `${s}.0`;
};
const pad = (p, n) => [...p, ...new Array(n - p.length).fill(0)];
// The shortcut's constants, shared by the Rust and JS emitters. One term count
// per function family across gamuts keeps the Rust trait arrays a fixed size;
// unused high terms are zero.
function constants ({ space, window, sectors }, U_TERMS, G_TERMS) {
	const [blue, red, green] = sectors;
	assert(window === null ? red.end === green.start : red.end === window[0], `${space.id}: red end`);
	// The shortcut never runs inside the blue-fold window, where the in-gamut
	// chroma set can be disconnected; the exact solver handles those hues.
	const greenStart = window === null ? green.start : Math.max(green.start, window[1]);
	return {
		BLUE_START: blue.start, RED_START: red.start, RED_END: red.end, GREEN_START: greenStart, RED_FOLD: red.hFold,
		SCALE: sectors.map(s => s.scale), OFFSET: sectors.map(s => s.offset), MARGIN: sectors.map(s => s.u.margin),
		U: sectors.map(s => pad(s.u.p, U_TERMS)),
		G0: sectors.map(s => pad(s.g.ps[0], G_TERMS)), G1: sectors.map(s => pad(s.g.ps[1], G_TERMS)),
	};
}
function emit (data, U_TERMS, G_TERMS) {
	const { space, sectors } = data;
	const c = constants(data, U_TERMS, G_TERMS);
	const row = p => `[${p.map(rust).join(", ")}]`;
	const lines = [
		`// Generated by scripts/generate-dualray-fast.mjs; do not edit.`,
		`// ${space.id}: lower-face hue fits. Sector order: blue-zero, red-zero, green-zero.`,
		...sectors.map(s => `// zero=${"RGB"[s.zero]} ${s.start.toFixed(6)}..${s.end.toFixed(6)} u deg ${s.u.degree} (${s.u.error.toExponential(2)}), g deg ${s.g.degree} (ΔEOK ${s.g.error.toExponential(2)})${s.hFold === null ? "" : `, fold ${s.hFold}`}`),
		`pub const U_TERMS: usize = ${U_TERMS};`,
		`pub const G_TERMS: usize = ${G_TERMS};`,
		`// Shortcut hues: [BLUE_START, RED_END) and (GREEN_START, BLUE_START + 360).`,
		...["BLUE_START", "RED_START", "RED_END", "GREEN_START", "RED_FOLD"].map(k => `pub const ${k}: f64 = ${rust(c[k])};`),
		`// t = SCALE * v + OFFSET, v = hue (+360 below BLUE_START) or sqrt(RED_FOLD - hue).`,
		`pub const SCALE: [f64; 3] = ${row(c.SCALE)};`,
		`pub const OFFSET: [f64; 3] = ${row(c.OFFSET)};`,
		`// Added to the fitted boundary ratio so that it bounds the sampled ratio from above.`,
		`pub const MARGIN: [f64; 3] = ${row(c.MARGIN)};`,
		`pub const U: [[f64; ${U_TERMS}]; 3] = [${c.U.map(row).join(", ")}];`,
		`// The two nonzero linear channels, in increasing channel order, divided by L³.`,
		`pub const G0: [[f64; ${G_TERMS}]; 3] = [${c.G0.map(row).join(", ")}];`,
		`pub const G1: [[f64; ${G_TERMS}]; 3] = [${c.G1.map(row).join(", ")}];`,
		``,
	];
	return lines.join("\n");
}

// The same grouped-power (Estrin) tree as Rust's estrin without FMA:
// adjacent pairs combine with t, then t², t⁴, …; an odd tail passes through.
function estrinSource (name, n) {
	let level = Array.from({ length: Math.ceil(n / 2) }, (_, i) =>
		2 * i + 1 < n ? `p[o + ${2 * i + 1}] * t + p[o${i ? ` + ${2 * i}` : ""}]` : `p[o + ${2 * i}]`);
	const lines = [`export function ${name} (p, o, t) {`];
	let power = "t", depth = 0;
	while (level.length > 1) {
		const names = level.map((_, i) => `${"abcdefgh"[depth]}${i}`);
		lines.push(`\tconst ${level.map((x, i) => `${names[i]} = ${x}`).join(", ")};`);
		const next = `t${2 ** (depth + 1)}`;
		lines.push(`\tconst ${next} = ${power} * ${power};`);
		power = next;
		level = Array.from({ length: Math.ceil(names.length / 2) }, (_, i) =>
			2 * i + 1 < names.length ? `${names[2 * i + 1]} * ${power} + ${names[2 * i]}` : names[2 * i]);
		depth++;
	}
	lines.push(`\treturn ${level[0]};`, `}`);
	return lines.join("\n");
}
function emitJs (all, U_TERMS, G_TERMS) {
	const flat = rows => `Object.freeze([${rows.flat().map(String).join(", ")}])`;
	const entries = all.map(data => {
		const c = constants(data, U_TERMS, G_TERMS);
		return `\t${JSON.stringify(data.space.id)}: {\n` + [
			`blueStart: ${c.BLUE_START}, redStart: ${c.RED_START}, redEnd: ${c.RED_END}, greenStart: ${c.GREEN_START}, redFold: ${c.RED_FOLD}`,
			`scale: ${flat(c.SCALE)}, offset: ${flat(c.OFFSET)}, margin: ${flat(c.MARGIN)}`,
			`u: ${flat(c.U)}`, `g0: ${flat(c.G0)}`, `g1: ${flat(c.G1)}`,
		].map(line => `\t\t${line},`).join("\n") + `\n\t},`;
	});
	return [
		`// Generated by scripts/generate-dualray-fast.mjs; do not edit.`,
		`// Lower-face hue fits per target (fit errors: rust/src/generated/dualray_fast_*.rs).`,
		`// Sector order: blue-zero, red-zero, green-zero. u, g0 and g1 hold each`,
		`// sector's monomial coefficients in t, U_TERMS or G_TERMS per sector.`,
		`export const U_TERMS = ${U_TERMS}, G_TERMS = ${G_TERMS};`,
		estrinSource("estrinU", U_TERMS),
		estrinSource("estrinG", G_TERMS),
		`export const dualrayFastData = Object.freeze({`,
		...entries,
		`});`,
		`for (const entry of Object.values(dualrayFastData)) Object.freeze(entry);`,
		``,
	].join("\n");
}

const all = [SRGB, DISPLAY_P3, REC2020].map(generate);
const U_TERMS = Math.max(...all.flatMap(d => d.sectors.map(s => s.u.degree))) + 1;
const G_TERMS = Math.max(...all.flatMap(d => d.sectors.map(s => s.g.degree))) + 1;
let stale = false;
function write (file, text) {
	if (check) {
		let current = null;
		try { current = readFileSync(file, "utf8"); } catch {}
		if (current !== text) { console.error(`stale: ${file.pathname}`); stale = true; }
	} else {
		writeFileSync(file, text);
	}
}
for (const data of all) {
	const { space } = data;
	for (const s of data.sectors)
		console.log(`${space.id} zero=${"RGB"[s.zero]} ${s.start.toFixed(4)}..${s.end.toFixed(4)}: u deg ${s.u.degree} ${s.u.error.toExponential(2)} margin ${s.u.margin.toExponential(2)}; g deg ${s.g.degree} ΔEOK ${s.g.error.toExponential(2)}`);
	write(new URL(`../rust/src/generated/dualray_fast_${space.id.replace("-", "_")}.rs`, import.meta.url), emit(data, U_TERMS, G_TERMS));
}
write(new URL("../src/generated/dualray-fast.js", import.meta.url), emitJs(all, U_TERMS, G_TERMS));
if (stale) process.exit(1);
