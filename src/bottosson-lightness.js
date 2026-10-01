// Backwards-compatible P3 instances. Import bottosson-factory.js for setup
// limited to a selected target (no default P3 cache allocation).
import { DISPLAY_P3 } from "./rgb-spaces.js";
import { createBottossonLightness, createBottossonLightnessCached } from "./bottosson-factory.js";
export * from "./bottosson-factory.js";
export const bottossonLightness = createBottossonLightness(DISPLAY_P3);
export const bottossonLightnessCached = createBottossonLightnessCached(DISPLAY_P3);
