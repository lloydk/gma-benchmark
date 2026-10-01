import { edgeSeekerTables } from "../fixtures/edge-seeker.js";
import { runtimeReferenceRows } from "./edge-seeker-reference.js";
const cache=new Map();
export function steepIntervals(id) {
 if(!cache.has(id))cache.set(id,[edgeSeekerTables[id],runtimeReferenceRows(id)].flatMap(rows=>rows.flatMap((row,i)=>
  i&&row[2]-rows[i-1][2]<.001&&Math.abs(row[1]-rows[i-1][1])>.001?[[rows[i-1][2],row[2]]]:[])));
 return cache.get(id);
}
export function inSteepInterval(id,h) {
 h%=360;if(h<0)h+=360;
 // Only rounding-scale endpoint slack, not the matrix solvers' fold window.
 return steepIntervals(id).some(([lo,hi])=>h>=lo-1e-12&&h<=hi+1e-12);
}
