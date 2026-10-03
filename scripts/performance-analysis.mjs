// Hand-written narrative lives in the Markdown template. Numbers come from
// the artifacts; assertions make changed conclusions require editorial review.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { median } from './performance-stats.mjs';

export function renderPerformanceAnalysis(report, profile) {
 const review = (condition,reason) => assert.ok(condition,`Performance analysis needs editorial review: ${reason}`);
 review(profile?.schema==='gma-performance-math-v1','regenerate the Math-call profile');
 review(profile.sourceCommit===report.sourceCommit && profile.runtime===report.environment.node,'profile implementation/runtime changed');
 review(report.environment.node==='v26.10.0' && report.environment.bun==='1.4.2','recheck the engine observations for the new runtime versions');
 const row=(w,r,m,g='display-p3')=>report.summary.find(x=>x.workload===`${g}-${w}` && x.runtime===r && x.method===m);
 const ns=(w,r,m,g='display-p3')=>{const x=row(w,r,m,g)?.ns; review(Number.isFinite(x),`missing ${g}/${w}/${r}/${m}`);return x;};
 const ops=(w,m,op)=>{
  const x=profile.rows.find(x=>x.workload===`display-p3-${w}` && x.method===m);
  const validation=report.validation.find(v=>v.workload===`display-p3-${w}` && v.runtime==='node' && v.method===m);
  review(x && validation && x.checksum===validation.checksum && x.count===validation.count,`profile inputs/output changed for ${w}/${m}`);
  review(x.perColor[op]===x.counts[op]/x.count,`invalid ${op} count`);
  return x.perColor[op];
 };
 const runtimes=['node','bun','rust-f64','rust-f32'];
 const title={node:'Node',bun:'Bun','rust-f64':'Rust f64','rust-f32':'Rust f32'};
 const n=x=>x.toFixed(1), ratio=(a,b)=>(a/b).toFixed(2), pct=x=>`${(100*x).toFixed(1)}%`;
 const table=(headers,rows)=>[
  '| '+headers.join(' | ')+' |', '| '+headers.map((_,i)=>i?'---:':'---').join(' | ')+' |',
  ...rows.map(row=>'| '+row.join(' | ')+' |'),
 ].join('\n');
 for (const r of runtimes) {
  review(ns('random',r,'dualray-fast')<ns('random',r,'dualray'),`Fast no longer beats Dualray in ${r}`);
  review(ns('below-cusp',r,'dualray-fast')/ns('below-cusp',r,'dualray')<ns('above-cusp',r,'dualray-fast')/ns('above-cusp',r,'dualray'),`Fast's lower-face advantage changed in ${r}`);
  for (const [a,b] of [['oklch-cubic-no-cache','oklch-cubic'],['bottosson-lightness','bottosson-lightness-cached']]) review(ns('random',r,a)>ns('random',r,b),`cache no longer wins in ${r}`);
 }
 review(ns('below-cusp','rust-f64','dualray-fast')<ns('below-cusp','rust-f64','clip'),'Fast no longer beats clip below cusp');
 review(ns('random','node','css-minde')<Math.min(ns('random','bun','css-minde'),ns('random','rust-f64','css-minde')),'CSS MINDE engine ordering changed');
 for (const r of ['rust-f64','rust-f32']) review(ns('random',r,'dualray-fast-poly')<ns('random',r,'dualray-fast'),`poly encoder no longer wins in ${r}`);
 review(ns('random','rust-f64','dualray-fast')-ns('random','rust-f64','dualray-fast-poly')>ns('random','rust-f32','dualray-fast')-ns('random','rust-f32','dualray-fast-poly'),'encoder precision comparison reversed');
 review(ns('random','rust-f32','edge-seeker')/ns('grid','rust-f32','edge-seeker')>ns('random','rust-f32','edge-seeker-indexed')/ns('grid','rust-f32','edge-seeker-indexed'),'index no longer reduces random/grid penalty');
 for (const r of ['node','rust-f64']) review(Math.abs(ns('random',r,'oklch-halley')/ns('random',r,'oklch-ostrowski')-1)<.15,`iterative solvers no longer close in ${r}`);
 review(ns('random','rust-f32','oklch-cubic-no-cache')>ns('random','rust-f64','oklch-cubic-no-cache'),'uncached cubic f32 exception disappeared');
 review(ns('inside-checked','node','css-minde')<ns('random-checked','node','css-minde'),'CSS MINDE interior result changed');
 review(ns('inside-checked','node','dualray-fast')>ns('random-checked','node','dualray-fast'),'Fast interior result changed');
 review(ops('below-cusp','dualray-fast','sin')===0 && ops('below-cusp','dualray-fast','cos')===0,'Fast lower path now uses trig');
 review(ops('above-cusp','edge-seeker','sqrt')===2 && ops('below-cusp','edge-seeker','sqrt')===0,'Edge Seeker arc work changed');
 review(Math.abs(ops('random','css-minde','cbrt')-3*ops('random','css-minde','sqrt'))<1e-12,'CSS MINDE error-comparison counts changed');
 const fastAbove=profile.rows.find(x=>x.workload==='display-p3-above-cusp' && x.method==='dualray-fast');
 review(fastAbove?.paths && fastAbove.pathsPerColor,'regenerate the Fast path counters');
 for (const [path,count] of Object.entries(fastAbove.paths)) {
  review(Number.isSafeInteger(count) && count>=0 && fastAbove.pathsPerColor[path]===count/fastAbove.count,`invalid Fast ${path} count`);
 }
 review(fastAbove.paths.upper===fastAbove.count && fastAbove.paths.lower===0 && fastAbove.paths.exact===0,'Fast above-cusp branch attribution changed');
 review(fastAbove.paths.precheck>0 && fastAbove.paths.precheck<fastAbove.count,'Fast precheck fraction changed');
 review(fastAbove.counts.sin===fastAbove.paths.upper+fastAbove.paths.precheck && fastAbove.counts.cos===fastAbove.counts.sin,'Fast trig attribution changed');
 review(ns('random','node','raytrace')>ns('random','rust-f64','raytrace'),'Raytrace Node/Rust ordering changed');
 review(ns('random','bun','raytrace')>ns('random','node','raytrace'),'Raytrace Node/Bun ordering changed');
 for (const m of ['oklch-cubic','dualray-fast']) review(ns('random','bun',m)<ns('random','node',m),`Bun advantage changed for ${m}`);

 const text={};
 const times={
  fast_node_below:['below-cusp','node','dualray-fast'],fast_node_above:['above-cusp','node','dualray-fast'],
  fast_f64_below:['below-cusp','rust-f64','dualray-fast'],fast_f64_above:['above-cusp','rust-f64','dualray-fast'],
  clip_f64_below:['below-cusp','rust-f64','clip'],
  minde_node:['random','node','css-minde'],minde_bun:['random','bun','css-minde'],minde_f64:['random','rust-f64','css-minde'],
  fast_f64:['random','rust-f64','dualray-fast'],fast_f32:['random','rust-f32','dualray-fast'],
  poly_f64:['random','rust-f64','dualray-fast-poly'],poly_f32:['random','rust-f32','dualray-fast-poly'],
  edge_grid:['grid','rust-f32','edge-seeker'],edge_random:['random','rust-f32','edge-seeker'],
  index_grid:['grid','rust-f32','edge-seeker-indexed'],index_random:['random','rust-f32','edge-seeker-indexed'],
  halley_node:['random','node','oklch-halley'],ostrowski_node:['random','node','oklch-ostrowski'],
  halley_f64:['random','rust-f64','oklch-halley'],ostrowski_f64:['random','rust-f64','oklch-ostrowski'],
  uncached_f32:['random','rust-f32','oklch-cubic-no-cache'],uncached_f64:['random','rust-f64','oklch-cubic-no-cache'],
  minde_inside_node:['inside-checked','node','css-minde'],minde_checked_node:['random-checked','node','css-minde'],
  fast_inside_node:['inside-checked','node','dualray-fast'],fast_checked_node:['random-checked','node','dualray-fast'],
  halley_inside_f64:['inside-checked','rust-f64','oklch-halley'],halley_inside_f32:['inside-checked','rust-f32','oklch-halley'],
  dualray_inside_f64:['inside-checked','rust-f64','dualray'],
  cubic_node:['random','node','oklch-cubic'],cubic_bun:['random','bun','oklch-cubic'],
  fast_node:['random','node','dualray-fast'],fast_bun:['random','bun','dualray-fast'],
 };
 for (const [key,args] of Object.entries(times)) text[key]=n(ns(...args));
 const savings=runtimes.map(r=>1-ns('random',r,'dualray-fast')/ns('random',r,'dualray'));
 text.fast_savings=`**${pct(Math.min(...savings))}–${pct(Math.max(...savings))} across these runtimes**`;
 text.fast_comparison_table=table(['Runtime','Random: time saved by Fast','Above cusp: Fast vs Dualray'],runtimes.map((r,i)=>{
  const change=ns('above-cusp',r,'dualray-fast')/ns('above-cusp',r,'dualray')-1;
  review(Math.abs(change)<.15,`above-cusp Dualray/Fast comparison is no longer close in ${r}`);
  return [title[r],pct(savings[i]),`${pct(Math.abs(change))} ${change<0?'less':'more'} time`];
 }));
 text.below_share=pct(report.p3RandomBelowCusp);
 text.fast_above_trig=ops('above-cusp','dualray-fast','sin').toFixed(2);
 text.fast_precheck_share=pct(fastAbove.pathsPerColor.precheck);
 const cacheRange=(a,b)=>{const xs=runtimes.map(r=>ns('random',r,a)/ns('random',r,b));return `${Math.min(...xs).toFixed(2)}–${Math.max(...xs).toFixed(2)}× across runtimes`;};
 text.cubic_cache_ratios=cacheRange('oklch-cubic-no-cache','oklch-cubic');
 text.bottosson_cache_ratios=cacheRange('bottosson-lightness','bottosson-lightness-cached');
 text.cubic_uncached_cbrt=ops('random','oklch-cubic-no-cache','cbrt').toFixed(2);
 text.cubic_cached_cbrt=ops('random','oklch-cubic','cbrt').toFixed(2);
 text.grid_winners=['node','bun'].map(r=>{
  const winner=report.summary.filter(x=>x.workload==='display-p3-grid' && x.runtime===r && x.method!=='clip').sort((a,b)=>a.ns-b.ns)[0];
  return `${report.methods.find(([id])=>id===winner.method)[1]} in ${title[r]} (${n(winner.ns)} ns)`;
 }).join(' and ');
 text.minde_cbrt=n(ops('random','css-minde','cbrt'));
 text.minde_errors=n(ops('random','css-minde','cbrt')/3);
 text.minde_inside_cbrt=n(ops('inside-checked','css-minde','cbrt'));
 text.raytrace_cbrt=n(ops('random','raytrace','cbrt'));
 text.raytrace_times=['node','bun','rust-f64'].map(r=>`${n(ns('random',r,'raytrace'))} ns in ${title[r]}`).join(', ');
 // Review-supplied independent-operation throughput difference: 8.7−4.3 ns.
 // Retain it only as a consistency check, never as an additive time profile.
 const operationGap=4.4;
 text.minde_cbrt_gap=n(ops('random','css-minde','cbrt')*operationGap);
 text.raytrace_cbrt_gap=n(ops('random','raytrace','cbrt')*operationGap);
 const mindeGap=ns('random','bun','css-minde')-ns('random','node','css-minde');
 const raytraceGap=ns('random','bun','raytrace')-ns('random','node','raytrace');
 text.minde_engine_gap=n(mindeGap); text.raytrace_engine_gap=n(raytraceGap);
 review(Math.abs(ops('random','css-minde','cbrt')*operationGap/mindeGap-1)<.15,'CSS MINDE cube-root consistency check changed');
 review(ops('random','raytrace','cbrt')*operationGap>raytraceGap*2,'Raytrace no longer contrasts with CSS MINDE');
 for (const precision of ['f64','f32']) text[`poly_saving_${precision}`]=n(ns('random',`rust-${precision}`,'dualray-fast')-ns('random',`rust-${precision}`,'dualray-fast-poly'));
 text.edge_ratio=ratio(ns('random','rust-f32','edge-seeker'),ns('grid','rust-f32','edge-seeker'));
 text.index_ratio=ratio(ns('random','rust-f32','edge-seeker-indexed'),ns('grid','rust-f32','edge-seeker-indexed'));
 const shared=report.runtimeMethods['rust-f32'].filter(m=>report.runtimeMethods['rust-f64'].includes(m));
 text.rust_methods=shared.length;
 text.f32_wins=shared.filter(m=>ns('random','rust-f32',m)<ns('random','rust-f64',m)).length;
 review(text.f32_wins>shared.length/2,'f32 no longer wins for most methods');
 const canonical=['css-minde','oklch-cubic','oklch-cubic-no-cache','oklch-cubic-direct',
  'oklch-halley','oklch-ostrowski','bottosson-lightness','bottosson-lightness-cached',
  'edge-seeker','edge-seeker-indexed','raytrace'];
 review(canonical.every(m=>shared.includes(m)),'canonical-comparison methods missing');
 for (const r of ['f64','f32']) text[`interior_median_${r}`]=n(median(canonical.map(m=>ns('inside-checked',`rust-${r}`,m))));
 review(Number(text.interior_median_f32)<Number(text.interior_median_f64),'canonical interior f32 advantage reversed');
 text.runtime_comparison_table=table(['P3 random ratio','Median across shared methods','Range across methods'],[
  ['node','bun'],['node','rust-f64'],['bun','rust-f64'],
 ].map(([a,b])=>{
  const methods=report.runtimeMethods[a].filter(m=>report.runtimeMethods[b].includes(m));
  const xs=methods.map(m=>ns('random',a,m)/ns('random',b,m)), middle=median(xs);
  review(b==='bun'?Math.abs(middle-1)<.1:middle>1.1 && middle<1.3,`runtime summary changed for ${a}/${b}`);
  return [`${title[a]} / ${title[b]}`,`${middle.toFixed(2)}×`,`${Math.min(...xs).toFixed(2)}–${Math.max(...xs).toFixed(2)}×`];
 }));
 const gamuts=['srgb','display-p3','rec2020'];
 const gamutRatio=(g,w,r,m)=>ns(w,r,m,g)/ns(w,r,m);
 for (const w of ['random','grid']) {
  const inputs=gamuts.map(g=>report.workloads.find(x=>x.id===`${g}-${w}`));
  review(inputs.every(x=>x && x.count>0 && !x.checked && x.sha256),'missing standard gamut input evidence');
  review(inputs.every(x=>x.count===inputs[0].count && x.sha256===inputs[0].sha256),`gamut ${w} inputs are no longer identical`);
  review(inputs[0].inside===0 && inputs[1].inside===0 && inputs[2].inside>0 && inputs[2].inside/inputs[2].count<.005,`gamut ${w} membership mix changed`);
  text[`rec2020_${w}_inside`]=inputs[2].inside;
  text[`rec2020_${w}_inside_share`]=pct(inputs[2].inside/inputs[2].count);
 }
 text.gamut_comparison_table=table(['Runtime','sRGB / P3, random','Rec.2020 / P3, random','sRGB / P3, grid','Rec.2020 / P3, grid'],runtimes.map(r=>{
  const middle=(g,w)=>median(report.runtimeMethods[r].map(m=>gamutRatio(g,w,r,m)));
  for (const w of ['random','grid']) {
   review(Math.abs(middle('srgb',w)-1)<.03,`sRGB/P3 typical cost changed in ${r}/${w}`);
   review(Math.abs(middle('rec2020',w)-1)<.1,`Rec.2020/P3 typical cost changed in ${r}/${w}`);
   review(Math.abs(middle('rec2020',w)-1)>Math.abs(middle('srgb',w)-1),`relative gamut sensitivity changed in ${r}/${w}`);
  }
  review(middle('rec2020','grid')>middle('rec2020','random'),'Rec.2020 grid/random cost pattern changed');
  return [title[r],...['random','grid'].flatMap(w=>['srgb','rec2020'].map(g=>`${middle(g,w).toFixed(2)}×`))];
 }));
 const dualrayChanges=[];
 for (const r of runtimes) for (const g of gamuts) {
  const winner=r.startsWith('rust-')?'dualray-fast-poly':'dualray-fast';
  review(report.runtimeMethods[r].filter(m=>m!=='clip' && m!==winner).every(m=>ns('random',r,winner,g)<ns('random',r,m,g)),`random gamut winner changed in ${g}/${r}`);
  for (const m of ['dualray','dualray-fast']) dualrayChanges.push(Math.abs(gamutRatio(g,'random',r,m)-1));
 }
 review(Math.max(...dualrayChanges)<.1,'Dualray/Fast gamut sensitivity changed');
 text.dualray_gamut_change=pct(Math.max(...dualrayChanges));
 const penaltyRange=xs=>`${pct(Math.min(...xs))}–${pct(Math.max(...xs))}`;
 for (const w of ['random','grid']) {
  const iterative=['oklch-halley','oklch-ostrowski'].map(m=>gamutRatio('rec2020',w,'bun',m)-1);
  review(iterative.every(x=>x>.15),'Bun iterative Rec.2020 penalty changed');
  text[`iterative_rec2020_${w}_penalty`]=penaltyRange(iterative);
  const cubic=runtimes.map(r=>gamutRatio('rec2020',w,r,'oklch-cubic')-1);
  review(cubic.every(x=>x>0),'cached cubic Rec.2020 penalty changed');
  text[`cubic_rec2020_${w}_penalty`]=penaltyRange(cubic);
 }
 for (const g of gamuts) text[`halley_bun_${g}`]=n(ns('random','bun','oklch-halley',g));
 review(ns('random','bun','oklch-halley','srgb')>ns('random','bun','oklch-halley'),'Bun Halley sRGB penalty changed');
 const clipPenalty=['node','bun','rust-f64'].map(r=>gamutRatio('rec2020','random',r,'clip')-1);
 review(clipPenalty.every(x=>x>0),'binary64 Rec.2020 clip penalty changed');
 text.clip_rec2020_penalty=penaltyRange(clipPenalty);
 review(ns('random','rust-f32','clip','rec2020')<ns('random','rust-f32','clip'),'f32 Rec.2020 clip ordering changed');
 text.clip_rec2020_f32=n(ns('random','rust-f32','clip','rec2020'));
 text.clip_p3_f32=n(ns('random','rust-f32','clip'));
 const template=readFileSync(new URL('./templates/performance-analysis.md',import.meta.url),'utf8');
 return template.replace(/\{\{([^}]+)\}\}/g,(_,key)=>{assert.ok(Object.hasOwn(text,key),`Unknown analysis token: ${key}`);return text[key];});
}
