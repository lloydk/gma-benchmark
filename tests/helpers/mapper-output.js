import assert from "node:assert/strict";

// Match the timing loop's contract: the caller owns and reads this buffer.
export function mapperOutput(map, input, ...args) {
 const out = [NaN, NaN, NaN];
 assert.equal(map(input, out, ...args), out, "mapper must return the supplied output buffer");
 assert.ok(out.every(Number.isFinite), "mapper must write all three output channels");
 return out;
}
