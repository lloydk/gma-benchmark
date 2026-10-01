import { profiles } from "./generated/rgb-spaces.js";

const clamp = x => x < 0 ? 0 : x > 1 ? 1 : x;
const srgb = Object.freeze({
	encode: x => Math.abs(x) <= 0.0031308 ? 12.92 * x : Math.sign(x) * (1.055 * Math.abs(x) ** (1 / 2.4) - 0.055),
	decode: x => Math.abs(x) <= 0.04045 ? x / 12.92 : Math.sign(x) * ((Math.abs(x) + 0.055) / 1.055) ** 2.4,
	encodeClamped: x => {
		x = clamp(x);
		return x <= 0.0031308 ? x * 12.92 : 1.055 * x ** (1 / 2.4) - 0.055;
	},
});
// CSS Rec.2020 encoding, distinct from the piecewise BT.2020 OETF.
const gamma24 = Object.freeze({
	encode: x => Math.sign(x) * Math.abs(x) ** (1 / 2.4),
	decode: x => Math.sign(x) * Math.abs(x) ** 2.4,
	encodeClamped: x => clamp(x) ** (1 / 2.4),
});
function freeze (value) {
	if (value && typeof value === "object") {
		Object.values(value).forEach(freeze);
		Object.freeze(value);
	}
	return value;
}
const transfers = { srgb, "gamma-2.4": gamma24 };
const spaces = profiles.map(profile => {
	if (!Object.hasOwn(transfers, profile.encoding)) throw new RangeError(`Unsupported encoding: ${profile.encoding}`);
	return freeze({ ...profile, whitePoint: "D65", transfer: transfers[profile.encoding] });
});
export const [SRGB, DISPLAY_P3, REC2020] = spaces;
export const RGB_SPACES = Object.freeze(Object.fromEntries(spaces.map(space => [space.id, space])));
export function getRgbSpace (id) {
	if (!Object.hasOwn(RGB_SPACES, id)) throw new RangeError(`Unsupported gamut: ${id}`);
	return RGB_SPACES[id];
}
