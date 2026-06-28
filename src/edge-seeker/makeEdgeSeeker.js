import { makeLut } from "./makeLut.js";

// Number of slices in the LUT
const SLICES = 400;
const HUE_INDEX_SCALE = 10;
const HUE_INDEX_BUCKETS = 360 * HUE_INDEX_SCALE;

/**
 * Creates a function that returns the maximum chroma for a given lightness and hue
 * @param rgbToOklch converter from RGB to OKLCH
 * @returns function that returns the maximum chroma for a given lightness and hue
 */
export function makeEdgeSeeker (rgbToOklch) {
	const { lutLength, lutL, lutC, lutH, lutCurvature } = makeLutColumns(rgbToOklch);

	return function getMaxChroma (l, h = 0) {
		if (l <= 0 || l >= 1) {
			return 0;
		}
		h = h < 0 ? (h % 360) + 360 : h % 360;
		let start = 0;
		let end = lutLength - 1;
		let mid = Math.floor((start + end) / 2);

		while (start <= end) {
			const midHue = lutH[mid];
			if (midHue === h) {
				return maxChromaFromLutItem(l, lutL[mid], lutC[mid], lutCurvature[mid]);
			}
			else if (midHue < h) {
				start = mid + 1;
			}
			else {
				end = mid - 1;
			}
			mid = Math.floor((start + end) / 2);
		}

		const lowHue = lutH[mid];
		const highHue = lutH[mid + 1];
		const t = (h - lowHue) / (highHue - lowHue);
		const itemL = lerp(lutL[mid], lutL[mid + 1], t);
		const itemC = lerp(lutC[mid], lutC[mid + 1], t);
		const itemCurvature = lerp(lutCurvature[mid], lutCurvature[mid + 1], t);

		return maxChromaFromLutItem(l, itemL, itemC, itemCurvature);
	};
}

/**
 * Creates a function like makeEdgeSeeker, but uses a dense hue interval index as
 * a starting point and then corrects to the exact LUT interval.
 * @param rgbToOklch converter from RGB to OKLCH
 * @returns function that returns the maximum chroma for a given lightness and hue
 */
export function makeEdgeSeekerIndexed (rgbToOklch) {
	const { lutLength, lutL, lutC, lutH, lutCurvature } = makeLutColumns(rgbToOklch);
	const intervalByBucket = makeIntervalIndex(lutH, lutLength);

	return function getMaxChroma (l, h = 0) {
		if (l <= 0 || l >= 1) {
			return 0;
		}
		h = h < 0 ? (h % 360) + 360 : h % 360;

		let bucket = Math.floor(h * HUE_INDEX_SCALE);
		if (bucket >= HUE_INDEX_BUCKETS) {
			bucket = HUE_INDEX_BUCKETS - 1;
		}

		let interval = intervalByBucket[bucket];
		while (interval > 0 && h < lutH[interval]) {
			interval--;
		}
		while (interval + 1 < lutLength - 1 && h > lutH[interval + 1]) {
			interval++;
		}

		const lowHue = lutH[interval];
		const highHue = lutH[interval + 1];
		const t = (h - lowHue) / (highHue - lowHue);
		const itemL = lerp(lutL[interval], lutL[interval + 1], t);
		const itemC = lerp(lutC[interval], lutC[interval + 1], t);
		const itemCurvature = lerp(lutCurvature[interval], lutCurvature[interval + 1], t);

		return maxChromaFromLutItem(l, itemL, itemC, itemCurvature);
	};
}

function makeLutColumns (rgbToOklch) {
	const lut = makeLut(rgbToOklch, SLICES);
	const lutLength = lut.length;
	// Parallel numeric columns. Keeping the hue column contiguous makes the
	// binary search cache-friendly for arbitrary (non-repeating) hues, where an
	// array-of-objects layout would pointer-chase scattered heap objects.
	const lutL = new Array(lutLength).fill(0);
	const lutC = new Array(lutLength).fill(0);
	const lutH = new Array(lutLength).fill(0);
	const lutCurvature = new Array(lutLength).fill(0);
	for (let i = 0; i < lutLength; i++) {
		const item = lut[i];
		lutL[i] = item.l;
		lutC[i] = item.c;
		lutH[i] = item.h;
		lutCurvature[i] = item.curvature;
	}
	return { lutLength, lutL, lutC, lutH, lutCurvature };
}

function makeIntervalIndex (lutH, lutLength) {
	const intervalByBucket = new Uint16Array(HUE_INDEX_BUCKETS);
	let interval = 0;
	for (let bucket = 0; bucket < HUE_INDEX_BUCKETS; bucket++) {
		const h = bucket / HUE_INDEX_SCALE;
		while (interval + 1 < lutLength - 1 && lutH[interval + 1] <= h) {
			interval++;
		}
		intervalByBucket[bucket] = interval;
	}
	return intervalByBucket;
}

/** Standard linear interpolation */
function lerp (start, end, t) {
	if (t <= 0) {
		return start;
	}
	if (t >= 1) {
		return end;
	}
	return start * (1 - t) + end * t;
}

function maxChromaFromLutItem (l, itemL, itemC, itemCurvature) {
	// The bottom (dark) part is always a straight line
	if (l <= itemL) {
		return (l / itemL) * itemC;
	}

	// The top (bright) part is approximated by an arc
	const x = (1 - l) / (1 - itemL); // Normalize l to 0-1 in arc space
	return itemC * intersectionWithArc(x, itemCurvature);
}

/** Finds the intersection of a line and an arc */
function intersectionWithArc (x, curvature) {
	if (curvature === 0) {
		return x;
	} // straight line

	const radius = Math.abs(1 / curvature);
	// Midpoint of the line segment from (0,0) to (1,1)
	const midpointX = 0.5;
	const midpointY = 0.5;

	// Distance from midpoint to any of the points (0,0) or (1,1)
	const halfDiagonal = Math.sqrt(midpointX ** 2 + midpointY ** 2);

	// Distance from midpoint to the center (using Pythagorean theorem)
	const distanceToCenter = Math.sqrt(radius ** 2 - halfDiagonal ** 2);

	// Since the bisector's slope is -1, the line is at 45 degrees, so the offsets for h and k are equal
	const offset = distanceToCenter / Math.sqrt(2);

	// Position of the center of the circle. Sign helps to determine the correct center
	const centerX = (curvature > 0 ? offset : -offset) + midpointX;
	const centerY = (curvature > 0 ? -offset : offset) + midpointY;

	// Calculate y for given x
	const underRoot = radius ** 2 - (x - centerX) ** 2;

	// If the value under the square root is negative, no solution exists for this center
	if (underRoot < 0) {
		return 0;
	}
	const sqrtVal = Math.sqrt(underRoot);
	const res1 = centerY + sqrtVal;
	if (res1 >= 0 && res1 <= 1) {
		return res1;
	}
	else {
		return centerY - sqrtVal;
	}
}
