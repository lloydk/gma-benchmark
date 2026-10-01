// Native-order authored-coordinate reference. Shares descriptor coefficients,
// but no production conversion, transfer function or membership predicate.
// XYZ reference checks below independently constrain the shared physical data.
import assert from "node:assert/strict";
import { createReference } from "./css-minde-reference.js";
export const bindMapperMode = (maps, checked) => Object.fromEntries(
 Object.entries(maps).map(([name,map])=>[name,(input,out)=>map(input,out,checked)]));
export function createCanonicalReference(space) {
 const xyz=createReference(space.id);
 return input=>{
  const [l,c,h]=input,angle=h*Math.PI/180,a=c*Math.cos(angle),b=c*Math.sin(angle);
  const lms=space.oklabToLms.map(([,ka,kb])=>{const v=l+ka*a+kb*b;return v*v*v;});
  const linear=space.lmsToRgb.map(([x,y,z])=>x*lms[0]+y*lms[1]+z*lms[2]);
  const independent=xyz.linearRgb(input);
  assert.ok(linear.every((v,i)=>Math.abs(v-independent[i])<=3e-14*Math.max(1,Math.abs(v))),`canonical XYZ agreement: ${space.id} ${input}`);
  return {linear,inside:linear.every(v=>v>=0&&v<=1),encoded:linear.map(v=>xyz.encode(Math.max(0,Math.min(1,v))))};
 };
}
