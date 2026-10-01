import { matrixSamples } from "./matrix-samples.js";
import { edgeSeekerTables } from "../fixtures/edge-seeker.js";
import { neighbour } from "./probe-utils.js";
export { neighbour } from "./probe-utils.js";
export function edgeSeekerProbes(id) {
 const rows=edgeSeekerTables[id],samples=[];
 for(const [l,c,h] of rows)for(const offset of [-720,-360,0,360,720])
  for(const angle of [neighbour(h+offset,-1),h+offset,neighbour(h+offset,1)])for(const lightness of [neighbour(l,-1),l,neighbour(l,1)])
   for(const chroma of [0,neighbour(c,-1),.5])samples.push([lightness,chroma,angle]);
 // The folded-table repair has a steep ramp over ~0.0001 degree. Sample it
 // directly instead of relying on integer grid hues or random chance.
 for(let i=1;i<rows.length;i++)if(rows[i][2]-rows[i-1][2]<.001&&Math.abs(rows[i][1]-rows[i-1][1])>.001) {
  const lo=rows[i-1][2],hi=rows[i][2];
  for(let n=-10;n<=110;n++)for(const offset of [-360,0,360])for(const l of [.01,.17938176,.414,rows[i-1][0],rows[i][0],.8,.9999])
   samples.push([l,.5,lo+(hi-lo)*n/100+offset]);
 }
 for(const l of [-1,0,Number.MIN_VALUE,1e-12,.001,.5,neighbour(1,-1),1,2])
  for(const c of [-.1,0,1e-14,.02,.4])for(const h of [-720,-360,-0,0,30.0123,245.067,264.04913,360,720])samples.push([l,c,h]);
 return samples;
}
export const edgeSeekerSamples=id=>[...matrixSamples(),...edgeSeekerProbes(id)];
