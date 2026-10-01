import { makeLut } from "./makeLut.js";

// Number of slices in the LUT
const SLICES = 400;
const HUE_INDEX_SCALE = 10;
const HUE_INDEX_BUCKETS = 360 * HUE_INDEX_SCALE;

// getRgbConversions supplies a stable converter per descriptor. Share the
// sampled table across both lookup variants and repeated factory calls.
// Distinct descriptors (even with the same name) keep independent tables.
const tablesByConverter = new WeakMap();
function tableForConverter (rgbToOklch) {
	let rows = tablesByConverter.get(rgbToOklch);
	if (!rows) {
		rows = makeLut(rgbToOklch, SLICES).map(({l,c,h,curvature}) => [l,c,h,curvature]);
		tablesByConverter.set(rgbToOklch, rows);
	}
	return rows;
}

/**
 * Creates a function that returns the maximum chroma for a given lightness and hue
 * @param rgbToOklch converter from RGB to OKLCH
 * @returns function that returns the maximum chroma for a given lightness and hue
 */
export function makeEdgeSeeker (rgbToOklch) {
	return makeEdgeSeekerFromTable(tableForConverter(rgbToOklch));
}

// Table construction and column/index allocation happen only at setup time.
// Private runtime rows are shared by every factory for a target.
export function makeEdgeSeekerFromTable (rows) {
	const { lutLength, lutL, lutC, lutH, lutCurvature } = tableColumns(rows);

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
	return makeEdgeSeekerIndexedFromTable(tableForConverter(rgbToOklch));
}

export function makeEdgeSeekerIndexedFromTable (rows) {
	const columns = tableColumns(rows);
	const { lutLength, lutL, lutC, lutH, lutCurvature } = columns;
	let intervalByBucket = indexes.get(columns);
	if (!intervalByBucket) {
		intervalByBucket = makeIntervalIndex(lutH, lutLength);
		indexes.set(columns, intervalByBucket);
	}

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

const columnsByTable = new WeakMap();
const indexes = new WeakMap();
function tableColumns (lut) {
	const existing = columnsByTable.get(lut);
	if (existing) return existing;
	validateEdgeSeekerTable(lut);
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
		lutL[i] = item[0];
		lutC[i] = item[1];
		lutH[i] = item[2];
		lutCurvature[i] = item[3];
	}
	const columns = { lutLength, lutL, lutC, lutH, lutCurvature };
	columnsByTable.set(lut, columns);
	return columns;
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
export function intersectionWithArc (x, curvature) {
	if (curvature === 0) {
		return x;
	} // straight line

	// Solve k*y² + (t-k)*y - x*(t+k*(1-x)) = 0, t = sqrt(2-k²).
	// Rationalization avoids cancellation near k=0 and selects the intended
	// arc without switching roots when endpoint rounding leaves [0,1].
	const t = Math.sqrt(2 - curvature * curvature);
	const b = t - curvature;
	const d = x * (t + curvature * (1 - x));
	const disc = Math.max(0, b * b + 4 * curvature * d);
	return Math.max(0, Math.min(1, 2 * d / (b + Math.sqrt(disc))));
}

// Invalid sampled data must fail at setup, never become NaN in interpolation.
export function validateEdgeSeekerTable(rows) {
	const invalid = () => { throw new RangeError("Invalid Edge Seeker table: expected finite, strictly increasing hue knots spanning 0..360"); };
	if (!Array.isArray(rows) || rows.length < 2 || rows.length > 65535) invalid();
	if (rows[0]?.[2] !== 0 || rows.at(-1)?.[2] !== 360) invalid();
	for (let i = 0; i < rows.length; i++) {
		const row = rows[i];
		if (!Array.isArray(row) || row.length !== 4 || !row.every(Number.isFinite)
			|| !(row[0] > 0 && row[0] < 1) || !(row[1] > 0) || !(Math.abs(row[3]) < 1)
			|| (i && !(row[2] > rows[i - 1][2]))) invalid();
	}
	for (const column of [0,1,3]) if (rows[0][column] !== rows.at(-1)[column]) invalid();
}
