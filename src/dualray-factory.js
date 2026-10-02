// Select generated inline-seed code once; physical descriptors remain separate.
import { RGB_SPACES } from "./rgb-spaces.js";
import { dualrayData } from "./generated/dualray.js";
// Internal numerical entry points retained for direct regression tests.
export { firstExit, search } from "./dualray-kernel.js";

export function createDualray(space) {
 if(RGB_SPACES[space.id]!==space || !Object.hasOwn(dualrayData,space.id))throw new RangeError(`No Dualray fits for ${space.id}`);
 const data=dualrayData[space.id];
 return data.create(space,data);
}
