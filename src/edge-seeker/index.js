// Backwards-compatible P3 instances; factory.js samples only requested targets.
import { DISPLAY_P3 } from "../rgb-spaces.js";
import { createEdgeSeeker, createEdgeSeekerIndexed } from "./factory.js";
export * from "./factory.js";
export const edgeSeeker = createEdgeSeeker(DISPLAY_P3);
export const edgeSeekerIndexed = createEdgeSeekerIndexed(DISPLAY_P3);
