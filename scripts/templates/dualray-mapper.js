// Maintained in scripts/templates/dualray-mapper.js; regenerate after edits.
// Imports resolve from src/generated/. Only the seed expression is specialized.
import { value, polish, firstRoot, lowerExit, mapFold } from "../dualray-kernel.js";
import { blueFoldWindow, inBlueFold } from "../matrix-solver-policy.js";
import { interiorWithin } from "../dualray-interval.js";
const DEG_TO_RAD=Math.PI/180;
const HUE_FAST_LIMIT=1e9;

export function createDualrayKernel(space, data) {
	const {basis,sectors,rootLimit}=data;
	const clampedGamma=space.transfer.encodeClamped,foldWindow=blueFoldWindow(space);
	const [[red1,red2],[green1,green2]]=sectors;
	const [rD3,rD2,rD1,rD0,rB0,rB2,rB1,rA1,rA0]=basis[0];
	const [gD3,gD2,gD1,gD0,gB0,gB2,gB1,gA1,gA0]=basis[1];
	const [bD3,bD2,bD1,bD0,bB0,bB2,bB1,bA1,bA0]=basis[2];
	return function dualray (oklch, out) {
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
					const du = wd * u, q = du + wb, v = q * u + wa;
					const f = (v * u + 1) - target;
					if (step === 1 && Math.abs(f) <= target * 1e-12) break;
					const f1 = (du + q) * u + v;
					const halfSecond = (du + du) + q;
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
					if (interiorWithin(ra,rb,u,guard) && interiorWithin(ga,gb,u,guard) && interiorWithin(ba,bb,u,guard)) {
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

			const fold = inBlueFold(hue, foldWindow);
			if (fold) {
				const endR=value(rd,rb,ra,inputU),endG=value(gd,gb,ga,inputU),endB=value(bd,bb,ba,inputU);
				if (endR>=0 && endR<=target && endG>=0 && endG<=target && endB>=0 && endB<=target) {
					// Certify the entire segment; inside endpoints can lie beyond re-entry.
					// Keep rounding-scale contacts on the isolation/snapping path.
					const margin=32*Number.EPSILON*target;
					if (endR>margin && endR<target-margin && endG>margin && endG<target-margin && endB>margin && endB<target-margin &&
						interiorWithin(ra,rb,inputU,target) && interiorWithin(ga,gb,inputU,target) && interiorWithin(ba,bb,inputU,target)) break mapped;
					return mapFold([[rd,rb,ra],[gd,gb,ga],[bd,bb,ba]],L3,target,inputU,rootLimit,clampedGamma,out);
				}
			}

			face = A * red1 + B * red2 > 1 ? 0 : A * green1 + B * green2 > 1 ? 1 : 2;
			const d = face === 0 ? rd : face === 1 ? gd : bd;
			const b = face === 0 ? rb : face === 1 ? gb : bb;
			const a = face === 0 ? ra : face === 1 ? ga : ba;
			let saturation;
			if (fold) {
				const result = lowerExit(rd,rb,ra,gd,gb,ga,bd,bb,ba,rootLimit);
				saturation = result[0]; face = result[1];
			} else {
				let lowerSeed;
				/* INLINE_SEED */
				saturation = polish(lowerSeed,d,b,a);
			}
			let lowerR = face === 0 ? 0 : ((rd * saturation + rb) * saturation + ra) * saturation + 1;
			let lowerG = face === 1 ? 0 : ((gd * saturation + gb) * saturation + ga) * saturation + 1;
			let lowerB = face === 2 ? 0 : ((bd * saturation + bb) * saturation + ba) * saturation + 1;
			if (
				!(saturation > 0 && saturation < rootLimit) ||
				lowerR < -1e-12 ||
				lowerG < -1e-12 ||
				lowerB < -1e-12
			) {
				const result = lowerExit(rd,rb,ra,gd,gb,ga,bd,bb,ba,rootLimit);
				saturation = result[0]; face = result[1];
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
				const du = wd * u, q = du + wb, v = q * u + wa;
				const f = (v * u + 1) - target;
				const f1 = (du + q) * u + v;
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

}
