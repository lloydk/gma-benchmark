// Dualray Fast: an approximate, cache-free constant-L/h gamut mapper for
// OKLCh input; a port of rust/src/dualray_fast.rs, which describes the design.
// Per-sector hue polynomials (scripts/generate-dualray-fast.mjs) give the
// lower-face boundary ratio and the two nonzero linear channels, so most
// out-of-gamut colors below the cusp need no trig, conversion or root solve.
// In-gamut colors return the canonical conversion. Above the cusp Dualray's
// upper solve refines a chord seed; everything else takes Dualray's exact
// search. The accuracy target is a deltaEOK of 1e-3 max and 1e-4 at p99.
// Rust's polynomial transfer row is not ported: bit access through typed
// arrays costs more than Math.pow in both V8 and JavaScriptCore.
// `tables` (the `dualray fast (tables)` row) corrects the upper seed from a
// generated table per target (360 B), so one Householder step usually
// converges. Unlike Rust it keeps Math.cos/Math.sin for the hue direction: a
// 22.5° table made V8's grid workload slower.
import { RGB_SPACES } from "./rgb-spaces.js";
import { getRgbConversions } from "./rgb-convert.js";
import { search } from "./dualray-kernel.js";
import { dualrayData } from "./generated/dualray.js";
import { dualrayFastData, estrinU, estrinG, U_TERMS, G_TERMS } from "./generated/dualray-fast.js";
const DEG_TO_RAD = Math.PI / 180;
// Dualray's residual and face tolerance, and the tables row's (Rust's f64
// SEEDED_TOLERANCE), far below the deltaEOK budget.
const TOLERANCE = 1e-12;
const SEEDED_TOLERANCE = 1e-7;

export function createDualrayFast (space, { tables = false } = {}) {
	if (RGB_SPACES[space.id] !== space || !Object.hasOwn(dualrayFastData, space.id)) throw new RangeError(`No Dualray Fast fits for ${space.id}`);
	const data = dualrayFastData[space.id];
	const { blueStart, redStart, redEnd, greenStart, redFold } = data;
	const [scale, offset, margin, U, G0, G1] = [data.scale, data.offset, data.margin, data.u, data.g0, data.g1].map(p => Float64Array.from(p));
	const seeds = Int8Array.from(data.seed), seedScale = data.seedScale;
	const tolerance = tables ? SEEDED_TOLERANCE : TOLERANCE;
	const basis = dualrayData[space.id].basis;
	const [[rD3,rD2,rD1,rD0,rB0,rB2,rB1,rA1,rA0],[gD3,gD2,gD1,gD0,gB0,gB2,gB1,gA1,gA0],[bD3,bD2,bD1,bD0,bB0,bB2,bB1,bA1,bA0]] = basis;
	const { oklchToRgbIfInGamut } = getRgbConversions(space);
	const encode = space.transfer.encodeClamped;
	// Dualray's exact search (cold): rejected upper solves, blue-fold hues and
	// out-of-range hues in the margin band.
	const exact = (L, C, hue, out) => {
		const radians = hue * DEG_TO_RAD, A = Math.cos(radians), B = Math.sin(radians);
		const A2 = A * A, AB = A * B, A3 = A2 * A, A2B = A2 * B, invL = 1 / L;
		const row = k => [k[0] * A3 + k[1] * A2B + k[2] * A + k[3] * B, k[4] + k[5] * A2 + k[6] * AB, k[7] * A + k[8] * B];
		return search([row(basis[0]), row(basis[1]), row(basis[2])], L * L * L, invL * invL * invL, C * invL, encode, out);
	};
	// The lower face: the sector's zero channel and two fitted channels.
	const lower = (sector, x, y, out) => {
		out[0] = out[1] = out[2] = 0;
		out[sector === 1 ? 1 : 0] = encode(x);
		out[sector === 0 ? 1 : 2] = encode(y);
		return out;
	};
	return function dualrayFast (oklch, out) {
		const L = oklch[0], C = oklch[1] > 0 ? oklch[1] : 0, H = oklch[2];
		if (!(L > 0 && L < 1)) { out[0] = out[1] = out[2] = L >= 1 ? 1 : 0; return out; }
		// The fits need the reduced hue, but the canonical conversion uses the
		// authored one, and for a large hue the two directions differ. Check
		// membership first, so the shortcut cannot claim an in-gamut color.
		const inRange = H >= 0 && H < 360;
		let hue = H;
		if (!inRange) {
			if (oklchToRgbIfInGamut(L, C, H, out)) return out;
			hue = H % 360;
			if (hue < 0) hue += 360;
		}
		// Sectors by zero channel: blue, red (in the fold's square root), green.
		let sector, v;
		if (hue >= blueStart && hue < redStart) { sector = 0; v = hue; }
		else if (hue >= redStart && hue < redEnd) { sector = 1; v = Math.sqrt(redFold - hue); }
		// Strict: the blue-fold window includes its upper endpoint.
		else if (hue > greenStart && hue < 360) { sector = 2; v = hue; }
		else if (hue >= 0 && hue < blueStart) { sector = 2; v = hue + 360; }
		// A hue the fits do not cover (a blue-fold window).
		else return inRange && oklchToRgbIfInGamut(L, C, H, out) ? out : exact(L, C, hue, out);
		const t = scale[sector] * v + offset[sector];
		const root = estrinU(U, sector * U_TERMS, t);
		const L3 = L * L * L;
		let g0, g1;
		if (C >= (root + margin[sector]) * L) {
			// Beyond the lower boundary: below the cusp the lower face, above it
			// always out of gamut.
			g0 = estrinG(G0, sector * G_TERMS, t);
			g1 = estrinG(G1, sector * G_TERMS, t);
			if (L3 * g0 <= 1 && L3 * g1 <= 1) return lower(sector, L3 * g0, L3 * g1, out);
		} else {
			if (inRange && oklchToRgbIfInGamut(L, C, H, out)) return out;
			g0 = estrinG(G0, sector * G_TERMS, t);
			g1 = estrinG(G1, sector * G_TERMS, t);
			// Out of gamut below the cusp (the margin band): the lower face. For an
			// out-of-range hue, "out of gamut" was decided at the authored
			// direction; search the reduced one instead.
			if (L3 * g0 <= 1 && L3 * g1 <= 1) return inRange ? lower(sector, L3 * g0, L3 * g1, out) : exact(L, C, hue, out);
		}

		// Above the cusp: the chord from the fitted lower root to the brighter
		// lower channel's crossing, refined by Dualray's upper solve: two
		// Householder steps, then one retry on a channel that exceeds the
		// target, never moving away from neutral. With tables, the bin's
		// correction lets the first face stop after one converged step.
		const brighter = g1 > g0;
		let face = brighter ? (sector === 0 ? 1 : 2) : sector === 1 ? 1 : 0;
		const radians = hue * DEG_TO_RAD, A = Math.cos(radians), B = Math.sin(radians);
		const A2 = A * A, AB = A * B, A3 = A2 * A, A2B = A2 * B;
		const rd = rD3 * A3 + rD2 * A2B + rD1 * A + rD0 * B, rb = rB0 + rB2 * A2 + rB1 * AB, ra = rA1 * A + rA0 * B;
		const gd = gD3 * A3 + gD2 * A2B + gD1 * A + gD0 * B, gb = gB0 + gB2 * A2 + gB1 * AB, ga = gA1 * A + gA0 * B;
		const bd = bD3 * A3 + bD2 * A2B + bD1 * A + bD0 * B, bb = bB0 + bB2 * A2 + bB1 * AB, ba = bA1 * A + bA0 * B;
		const invL = 1 / L, target = invL * invL * invL, limit = C * invL, guard = target * (1 + tolerance);
		let u, early = false, r = 0, g = 0, blue = 0;
		if (tables) {
			// The hue bin's correction; its low bit keeps both steps.
			const bin = 2 * Math.min(179, (hue * 0.5) | 0), k0 = seeds[bin];
			const tau = (target - 1) / ((brighter ? g1 : g0) - 1);
			const k = ((k0 >> 1) + seeds[bin + 1] * tau) * seedScale;
			u = root * tau * (1 + (1 - tau) * k);
			early = (k0 & 1) === 0;
		} else u = (root * (target - 1)) / ((brighter ? g1 : g0) - 1);
		for (let retry = 0; ; retry++) {
			const previous = u;
			const wd = face === 0 ? rd : face === 1 ? gd : bd;
			const wb = face === 0 ? rb : face === 1 ? gb : bb;
			const wa = face === 0 ? ra : face === 1 ? ga : ba;
			for (let step = 0; step < 2; step++) {
				const du = wd * u, q = du + wb, p = q * u + wa;
				const f = (p * u + 1) - target;
				if (early && retry === 0 && step === 1 && Math.abs(f) <= target * tolerance) break;
				const f1 = (du + q) * u + p;
				const halfSecond = (du + du) + q;
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
			if (!(u >= 0 && u <= limit && (retry === 0 || u <= previous) &&
				r >= -tolerance && g >= -tolerance && blue >= -tolerance &&
				Math.abs((face === 0 ? r : face === 1 ? g : blue) - target) <= target * tolerance)) return exact(L, C, hue, out);
			const brightest = r > g ? (r > blue ? 0 : 2) : g > blue ? 1 : 2;
			if ((brightest === 0 ? r : brightest === 1 ? g : blue) <= guard) break;
			if (retry) return exact(L, C, hue, out);
			face = brightest;
		}
		out[0] = face === 0 ? 1 : encode(L3 * r);
		out[1] = face === 1 ? 1 : encode(L3 * g);
		out[2] = face === 2 ? 1 : encode(L3 * blue);
		return out;
	};
}
// The `dualray fast (tables)` row.
export const createDualrayFastTables = space => createDualrayFast(space, { tables: true });
