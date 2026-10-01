import { createBoundaryReference } from "./matrix-reference.js";
import { bottossonFits } from "../../src/generated/bottosson.js";

// Seeds are algorithm parameters, shared deliberately. Geometry, sector choice,
// derivatives and conversion are independent of the production mapper.
export function createBottossonReference(id) {
 const ref = createBoundaryReference(id);
 const hues = [0,1,2].map(i => {
  const rgb = [0,0,0]; rgb[i] = 1;
  const [,a,b] = ref.linearToLab(rgb);
  return (Math.atan2(b,a)*180/Math.PI + 360)%360;
 });
 function face(h) {
  h = (h%360+360)%360;
  return h >= hues[1] && h < hues[2] ? 0 : h < hues[0] || h >= hues[2] ? 1 : 2;
 }
 // Recover coefficients from four XYZ-converted values, then differentiate
 // the polynomial rather than the LMS cubes used in production.
 function polynomial(l,h,channel,face) {
  const v = [0,1,-1,2].map(c => ref.linearRgb([l,c,h])[channel]);
  const b = (v[1]+v[2])/2-v[0], odd = (v[1]-v[2])/2;
  const a = (v[3]-v[0]-4*b-2*odd)/6;
  return [a,b,odd-a,v[0]-face];
 }
 function correction([a,b,c,d],x) {
  const f = ((a*x+b)*x+c)*x+d, f1 = (3*a*x+2*b)*x+c, f2 = 6*a*x+2*b;
  const u = f1/(f1*f1-.5*f*f2);
  return [u,-f*u];
 }
 function cusp(h,channel=face(h)) {
  const angle = h*Math.PI/180, a=Math.cos(angle), b=Math.sin(angle);
  const [k0,k1,k2,k3,k4] = bottossonFits[id].fits[channel];
  const seed = k0+k1*a+k2*b+k3*a*a+k4*a*b;
  const saturation = seed + correction(polynomial(1,h,channel,0),seed)[1];
  const l = 1/Math.cbrt(Math.max(...ref.linearRgb([1,saturation,h])));
  return { l, c:l*saturation, saturation };
 }
 function policy(l,h,channel=face(h)) {
  const cp = cusp(h,channel);
  if(l<=cp.l) return l*cp.saturation;
  const c = cp.c*(1-l)/(1-cp.l);
  const step = Math.min(...[0,1,2].map(i=>correction(polynomial(l,h,i,1),c)).filter(([u])=>u>=0).map(([,step])=>step));
  return c+step;
 }
 function adjacentFaces(h) {
  const channels = [face(h)], angle = (h%360+360)%360;
  for(let i=0;i<3;i++) if(Math.abs(angle-hues[i])<=1e-10) channels.push(...[[1,2],[2,0],[0,1]][i]);
  return [...new Set(channels)];
 }
 function exactSaturation(h,channel=face(h)) {
  const [a,b,c] = polynomial(1,h,channel,0);
  const disc=b*b-3*a*c;
  const knots=[0,4];
  if(disc>=0) for(const x of [(-b-Math.sqrt(disc))/(3*a),(-b+Math.sqrt(disc))/(3*a)]) if(x>0&&x<4) knots.push(x);
  knots.sort((a,b)=>a-b);
  const value=x=>ref.linearRgb([1,x,h])[channel];
  for(let i=1;i<knots.length;i++) {
   let lo=knots[i-1],hi=knots[i];
   if(!(value(lo)>=0&&value(hi)<0)) continue;
   for(let n=0;n<64;n++) {const mid=(lo+hi)/2;if(mid===lo||mid===hi)break;if(value(mid)>=0)lo=mid;else hi=mid;}
   return (lo+hi)/2;
  }
  throw new Error(`No face crossing: ${id} ${h} ${channel}`);
 }
 return {...ref,hues,face,policy,cusp,adjacentFaces,exactSaturation};
}
