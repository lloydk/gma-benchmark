// Shared, deterministic validation manifest; not the timing workload.
export function matrixSamples () {
	const samples = [];
	for (let l = 1; l < 100; l++) for (let h = 0; h < 360; h++) samples.push([l/100, .4, h]);
	for (let i = 0; i < 8192; i++) samples.push([.001 + .998*((i*.7548776662466927)%1), .5*((i*.5698402909980532)%1), (i*137.50776405003785)%1080 - 360]);
	for (let k = 14; k <= 52; k++) for (const h of [18.5,30,104,117.75,150,245.1,264.05,301.75]) samples.push([1-2**-k,.4,h], [2**-k,.4,h]);
	for (let i = 0; i <= 1000; i++) for (const l of [.1,.414,.49,.7,.98]) for (const center of [245.17,264.13]) samples.push([l,.4,center + (i-500)*.0005]);
	for (const l of [-.1,0,1e-12,.1,.5,.99,1,1.1]) for (const c of [-.1,0,.02,.4]) for (const h of [-720,-114.9,-95.9,0,245.1,264.05,720]) samples.push([l,c,h]);
	return samples.concat(mappingProbes());
}

// Small harness probes exercise input-chroma gaps and canonical preservation;
// unlike the C=.4 timing workloads, these reach both entry-mode decisions.
export function mappingProbes () {
 const samples = [[.3,.177897,264.053],[.2,.15757,245.067],[.3,.20787,264.053],[.2,.1808,245.067]];
 for (const [L,H,lo,hi] of [[.3,264.053,.17,.215],[.2,245.067,.15,.185]]) {
  for (let i=0;i<=90;i++) for (const shift of [-360,0,360]) samples.push([L,lo+(hi-lo)*i/90,H+shift]);
 }
 for (const L of [.001,.3,.7,.99999]) for (const C of [0,.0001,.01,.4]) for (const H of [-95.947,30.123,245.067,624.053]) samples.push([L,C,H]);
 return samples;
}
