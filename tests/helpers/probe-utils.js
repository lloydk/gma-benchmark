const bits = new DataView(new ArrayBuffer(8));
export function neighbour(x,direction) {
 if(x===0)return direction*Number.MIN_VALUE;
 bits.setFloat64(0,x);
 bits.setBigUint64(0,bits.getBigUint64(0)+BigInt(x<0?-direction:direction));
 return bits.getFloat64(0);
}
// Decimal round-trips include signed zero; Array.join/JSON.stringify lose it.
export const serializeProbes = samples => samples.map(v=>v.map(x=>Object.is(x,-0)?"-0":String(x)).join(",")).join("\n")+"\n";
