import { createBoundaryReference } from "./matrix-reference.js";
import { neighbour } from "./probe-utils.js";
const cache = new Map();
export function boundarySamples(id) {
 if(cache.has(id))return cache.get(id);
 const ref=createBoundaryReference(id),samples=[];
 const hues=Array.from({length:72},(_,i)=>i*5+.037);
 for(const primary of [[1,0,0],[0,1,0],[0,0,1]]) {
  const [,a,b]=ref.linearToLab(primary),h=Math.atan2(b,a)*180/Math.PI;
  for(const delta of [-.01,-1e-6,0,1e-6,.01])hues.push(h+delta);
 }
 for(const h of hues)for(const offset of [-720,-360,0,360,720])for(const l of [.01,.1,.3,.49,.7,.9,.98]) {
  const angle=h+offset,c=ref.boundaries(l,angle).first;
  for(const chroma of [c*(1-1e-7),neighbour(c,-1),c,neighbour(c,1),c*(1+1e-7)])samples.push([l,chroma,angle]);
 }
 // Concrete V8/glibc membership disagreement, from the original review corpus.
 samples.push([.9560843957994063,.20618683222959897,828.0328245917666]);
 for(const h of [-0,0,-360,360])samples.push([.5,.01,h]);
 cache.set(id,samples);return samples;
}
