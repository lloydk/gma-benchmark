import { createBoundaryReference } from "./matrix-reference.js";
import { makeColorList } from "../../src/edge-seeker/makeColorList.js";
import { getRgbConversions } from "../../src/rgb-convert.js";
import { RGB_SPACES } from "../../src/rgb-spaces.js";
const tables = new Map();
export function runtimeReferenceRows(id) {
 if(!tables.has(id)) {
  // Sampling/filtering remains shared algorithm data. Curvature is recovered
  // independently from the circle equation, without makeLut/calculateCurvature.
  const convert=getRgbConversions(RGB_SPACES[id]).rgbToOklch;
  const peaks=makeColorList(convert,.5,400),curves=makeColorList(convert,.75,400);
  const rows=peaks.map(({l,c,h})=>{
   const hi=curves.findIndex(v=>v.h>=h),a=curves[Math.max(0,hi-1)],b=curves[hi];
   const t=a.h===b.h?0:(h-a.h)/(b.h-a.h);
   const x=(1-(a.l+t*(b.l-a.l)))/(1-l),y=(a.c+t*(b.c-a.c))/c;
   const A=x*x+y*y-x-y,B=y-x;
   const k=B===0?0:-Math.SQRT2*B*Math.sign(A)/Math.hypot(A,B);
   return [l,c,h,k];
  });
  tables.set(id,rows);
 }
 return tables.get(id);
}

// Rebuild the LUT independently of the production cache in this runtime.
// Shared sampling is separately checked against recorded generator fixtures.
// The LUT is an algorithm parameter. Lookup is a linear scan, interpolation
// uses differences, and the arc is bisected from its circle residual. None of
// the production lookup, index, interpolation or quadratic helpers are used.
export function createEdgeSeekerReference(id) {
 const ref = createBoundaryReference(id), rows = runtimeReferenceRows(id);
 function itemAt(h) {
  h %= 360; if(h < 0)h += 360;
  const hi = rows.findIndex(row => row[2] >= h);
  if(rows[hi][2] === h)return rows[hi];
  const a=rows[hi-1],b=rows[hi],t=(h-a[2])/(b[2]-a[2]);
  return a.map((v,i)=>v+t*(b[i]-v));
 }
 function arc(x,k) {
  const t=Math.sqrt(2-k*k);let lo=0,hi=1;
  for(let i=0;i<60;i++) {
   const y=(lo+hi)/2;if(y===lo||y===hi)break;
   const residual=k*(x*x+y*y-x-y)+t*(y-x);
   if(residual<0)lo=y;else hi=y;
  }
  return (lo+hi)/2;
 }
 function chroma(l,h) {
  if(l<=0||l>=1)return 0;
  const [il,ic,,k]=itemAt(h);
  return l<=il?l/il*ic:ic*arc((1-l)/(1-il),k);
 }
 return {...ref,rows,itemAt,arc,chroma};
}
