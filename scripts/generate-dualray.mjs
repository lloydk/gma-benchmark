// Fit first-lower-root seeds from the production Rust target definitions.
// Chebyshev sampling, converted to balanced monomials; red tips use a square-root coordinate.
// The Display-P3 data is pinned to preserve the existing numerical path.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { loadProfiles } from "./rgb-gamut-profiles.mjs";
const check = process.argv.includes("--check");
assert(process.argv.slice(2).every(v => v === "--check"));
const pinned = JSON.parse(readFileSync(new URL("./data/dualray-display-p3.json", import.meta.url)));
const rust = n => Number.isInteger(n) ? `${n}.0` : String(n);
const array = a => `[${a.map(v => Array.isArray(v) ? array(v) : rust(v)).join(", ")}]`;
const DEG = Math.PI / 180,
	TERMS = 18;
const poly = (d, b, a, x) => ((d * x + b) * x + a) * x + 1;
function cubic(profile, face, h) {
	const A = Math.cos(h * DEG),
		B = Math.sin(h * DEG);
	const q = profile.oklabToLms.map(r => r[1] * A + r[2] * B);
	const w = profile.lmsToRgb[face];
	return [
		w.reduce((s, v, i) => s + v * q[i] ** 3, 0),
		3 * w.reduce((s, v, i) => s + v * q[i] ** 2, 0),
		3 * w.reduce((s, v, i) => s + v * q[i], 0),
	];
}
function root(profile, face, h) {
	const [d, b, a] = cubic(profile, face, h);
	const { rootLimit } = profile.dualray;
	const points = [0, rootLimit];
	const disc = b * b - 3 * d * a;
	if (d !== 0 && disc >= 0) {
		for (const s of [(-b - Math.sqrt(disc)) / (3 * d), (-b + Math.sqrt(disc)) / (3 * d)])
			if (s > 0 && s < rootLimit) points.push(s);
	} else if (d === 0 && b !== 0) {
		const s = -a / (2 * b);
		if (s > 0 && s < rootLimit) points.push(s);
	}
	points.sort((a, b) => a - b);
	for (let i = 1; i < points.length; i++) {
		let lo = points[i - 1],
			hi = points[i];
		if (poly(d, b, a, hi) > 0) continue;
		for (let k = 0; k < 60; k++) {
			const mid = (lo + hi) / 2;
			if (poly(d, b, a, mid) > 0) lo = mid;
			else hi = mid;
		}
		return (lo + hi) / 2;
	}
	throw Error(`no root ${profile.name} face=${face} h=${h}`);
}
function minimum(profile, face, h) {
	const [d, b, a] = cubic(profile, face, h),
		disc = b * b - 3 * d * a;
	if (disc < 0) return 1;
	let result = Infinity;
	for (const u of [(-b - Math.sqrt(disc)) / (3 * d), (-b + Math.sqrt(disc)) / (3 * d)])
		if (u > 0 && 6 * d * u + 2 * b > 0) result = Math.min(result, poly(d, b, a, u));
	return result;
}
function arc(x, y) {
	const c = Math.atan2(y, x) / DEG,
		r = Math.acos(1 / Math.hypot(x, y)) / DEG;
	return [c - r, c + r].map(h => (h + 720) % 360);
}
function fit(n, lo, hi, fn) {
	const centre = (lo + hi) / 2,
		half = (hi - lo) / 2;
	const values = Array.from({ length: n }, (_, k) => fn(centre + half * Math.cos((Math.PI * (k + 0.5)) / n)));
	const cheb = Array.from(
		{ length: n },
		(_, j) => (2 / n) * values.reduce((s, v, k) => s + v * Math.cos((j * Math.PI * (k + 0.5)) / n), 0),
	);
	cheb[0] /= 2;
	const mono = Array(n).fill(0);
	let prev = Array(n).fill(0),
		cur = Array(n).fill(0);
	prev[0] = 1;
	cur[1] = 1;
	for (let j = 0; j < n; j++) {
		const t = j === 0 ? prev : cur;
		for (let k = 0; k < n; k++) mono[k] += cheb[j] * t[k];
		if (j === 0) continue;
		const next = Array(n).fill(0);
		for (let k = 1; k < n; k++) next[k] += 2 * cur[k - 1];
		for (let k = 0; k < n; k++) next[k] -= prev[k];
		prev = cur;
		cur = next;
	}
	return { centre, invHalf: 1 / half, coef: mono };
}
function fitAt(f, v) {
	const t = (v - f.centre) * f.invHalf,
		t2 = t * t,
		t4 = t2 * t2,
		t8 = t4 * t4,
		c = f.coef;
	// Match fitCode's balanced groups, including Rec.2020's 22-term pieces.
	const pair = i => c[i] + c[i + 1] * t;
	const low = pair(0) + pair(2) * t2 + (pair(4) + pair(6) * t2) * t4;
	const high = pair(8) + pair(10) * t2 + (pair(12) + pair(14) * t2) * t4;
	const tail = c.length === 18 ? pair(16) : pair(16) + pair(18) * t2 + pair(20) * t4;
	return low + (high + tail * t8) * t8;
}
function makeFit(profile, face, lo, hi, fold) {
	// Ordinary pieces use the cross product with a fixed midpoint direction.
	// The red tip uses sqrt(sin(fold-h)), preserving the square-root branch point.
	const ref = fold ?? (lo + hi) / 2;
	const variable = h => (fold === undefined ? Math.sin((h - ref) * DEG) : Math.sqrt(Math.sin((fold - h) * DEG)));
	const inverse = v => (fold === undefined ? ref + Math.asin(v) / DEG : fold - Math.asin(v * v) / DEG);
	const bounds = [variable(lo), variable(hi)].sort((a, b) => a - b);
	// Rec.2020 has wider green/blue sectors, requiring four extra terms.
	return {
		...fit(profile.name === "rec2020" && face !== 0 ? 22 : TERMS, ...bounds, v => root(profile, face, inverse(v))),
		ref,
		fold,
		lo,
		hi,
		face,
	};
}

function channelBasis(profile, row) {
    const result = Array(9).fill(0);
    for (let k = 0; k < 3; k++) {
        const w = profile.lmsToRgb[row][k], a = profile.oklabToLms[k][1], b = profile.oklabToLms[k][2];
        result[0] += w * (a*a*a - 3*a*b*b);
        result[1] += w * (3*a*a*b - b*b*b);
        result[2] += 3*w*a*b*b;
        result[3] += w*b*b*b;
        result[4] += 3*w*b*b;
        result[5] += 3*w*(a*a-b*b);
        result[6] += 6*w*a*b;
        result[7] += 3*w*a;
        result[8] += 3*w*b;
    }
    return result;
}
function sectors(profile) {
    const h = [0,1,2].map(j => {
        const [l,m,s] = profile.rgbToLms.map(r => Math.cbrt(r[j]));
        return Math.atan2(0.0259040424655478*l+0.7827717124575296*m-0.8086757549230774*s,
            1.9779985324311684*l-2.4285922420485799*m+0.4505937096174110*s);
    });
    return [[1,2],[2,0]].map(([i,j]) => {
        const a=Math.cos(h[i]),b=Math.sin(h[i]),c=Math.cos(h[j]),d=Math.sin(h[j]);
        return [(d-b)/(a*d-b*c),(a-c)/(a*d-b*c)];
    });
}
function fitCode(f, language) {
    const declaration = language === "js" ? "const" : "let";
    const sin=rust(f.sin ?? Math.sin(f.ref*DEG)), cos=rust(f.cos ?? Math.cos(f.ref*DEG));
    const v=f.fold===undefined ? `(b * ${cos} - a * ${sin})`
        : language === "js" ? `Math.sqrt(Math.max(0, a * ${sin} - b * ${cos}))`
        : `(a * ${sin} - b * ${cos}).max(0.0).sqrt()`;
    const p=i => `(${rust(f.coef[i])} + ${rust(f.coef[i+1])} * t)`;
    const group=i => `(${p(i)} + ${p(i+2)} * t2) + (${p(i+4)} + ${p(i+6)} * t2) * t4`;
    const tail=f.coef.length===18 ? p(16) : `(${p(16)} + ${p(18)} * t2 + ${p(20)} * t4)`;
    const expression = `low + (high + ${tail} * t8) * t8`;
    const result = language === "js" ? `lowerSeed = ${expression};\nbreak seedEval;` : `return ${expression};`;
    return `${declaration} t = (${v} - ${rust(f.centre)}) * ${rust(f.invHalf)};
${declaration} t2=t*t;
${declaration} t4=t2*t2;
${declaration} t8=t4*t4;
${declaration} low=${group(0)};
${declaration} high=${group(8)};
${result}
`;
}
function seedCode(fits, split, language) {
    const conditions = [language === "js" ? "face === 1" : "face == 1",
        language === "js" ? "face === 2" : "face == 2",
        `a * ${rust(split[0])} - b * ${rust(split[1])} > 0.0`];
    const start = language === "js" ? "seedEval: {\nconst a=A,b=B;\n" : "pub fn seed(a: Float, b: Float, face: u8) -> Float {\n";
    return start + conditions.map((condition,i) =>
        `if ${language === "js" ? `(${condition})` : condition} {\n${fitCode(fits[i],language)}}\n`).join("")
        + `${fitCode(fits[3],language)}}\n`;
}
const jsImports = [], jsData = [];
const template = readFileSync(new URL("./templates/dualray-mapper.js", import.meta.url), "utf8");
const seedSlot = "\t\t\t\t/* INLINE_SEED */";
assert.equal(template.split(seedSlot).length, 2, "mapper template must have exactly one seed slot");
function emit(file, content) {
    const previous = existsSync(file) ? readFileSync(file,"utf8") : null;
    if (check) assert.equal(previous,content,`${file.pathname} stale`);
    else if (previous !== content) writeFileSync(file,content);
}
for (const profile of await loadProfiles()) {
    const id=profile.name, lines=id==='display-p3' ? pinned.sectors : sectors(profile);
    const basis=id==='display-p3' ? pinned.basis : [0,1,2].map(i => channelBasis(profile,i));
    let fits, seedSplit;
    if(id==='display-p3') {
        fits=pinned.fits.map((f,i)=>({sin:f[0],cos:f[1],centre:f[2],invHalf:f[3],coef:f.slice(4),fold:i===3?true:undefined}));
        seedSplit=pinned.split;
    }
    else {
        const [redStart,redEnd]=arc(...lines[0]),[,blueStart]=arc(...lines[1]);
        let lo=redEnd-0.1, hi=redEnd+3;
        assert(minimum(profile,0,lo)<0 && minimum(profile,0,hi)>0, `fold bracket ${id}`);
        for(let i=0;i<60;i++){const mid=(lo+hi)/2; if(minimum(profile,0,mid)<0)lo=mid; else hi=mid;}
        const fold=(lo+hi)/2;
        assert(Array.isArray(profile.dualray.foldWindow) && profile.dualray.foldWindow.length === 2, `${id} requires a fold window`);
        const [redEndFit,greenStart]=profile.dualray.foldWindow, split=redEndFit-14;
        seedSplit=[Math.sin(split*DEG),Math.cos(split*DEG)];
        // Empirical safety margin, paired with Rust's exhaustive f32 edge-hue
        // regression. This guards configuration drift, not arbitrary gamuts.
        const margin=0.02;
        for(const [name,h] of [['sector switch',redEnd],['fold',fold]]) {
            assert(redEndFit + margin < h && h + margin < greenStart, `${id} ${name} ${h} requires ${margin} degrees inside fold window`);
        }
        console.log(`${id}: sector ${redEnd}, fold ${fold}, minimum window margin ${Math.min(redEnd-redEndFit,greenStart-redEnd,fold-redEndFit,greenStart-fold)}`);
        fits=[makeFit(profile,1,greenStart,blueStart+360),makeFit(profile,2,blueStart,redStart),
            makeFit(profile,0,redStart,split),makeFit(profile,0,split,redEndFit,fold)];
        let worstRoot=0,worstResidual=0;
        for(const f of fits) for(let i=0;i<=65536;i++) {
            const h=f.lo+(f.hi-f.lo)*i/65536, A=Math.cos(h*DEG),B=Math.sin(h*DEG);
            const v=f.fold===undefined ? B*Math.cos(f.ref*DEG)-A*Math.sin(f.ref*DEG) : Math.sqrt(Math.max(0,A*Math.sin(f.ref*DEG)-B*Math.cos(f.ref*DEG)));
            const x=fitAt(f,v),[d,b,a]=cubic(profile,f.face,h),fx=poly(d,b,a,x),f1=(3*d*x+2*b)*x+a;
            const polished=x-fx*f1/(f1*f1-fx*(3*d*x+b));
            const err=Math.abs(polished-root(profile,f.face,h)), residual=Math.abs(poly(d,b,a,polished));
            assert(Number.isFinite(err)&&polished>0);
            worstRoot=Math.max(worstRoot,err);worstResidual=Math.max(worstResidual,residual);
        }
        assert(worstRoot<1e-13&&worstResidual<1e-13,`${id} convergence ${worstRoot}, ${worstResidual}`);
        console.log(`${check?'Checked':'Generated'} ${id}: 262148 held-out directions, root error ${worstRoot}, residual ${worstResidual}`);

    }
    // Verify normalized basis independently of its algebraic expansion.
    let maxBasis=0, maxRoot=0;
    for(let i=0;i<36000;i++) {
        const h=(i+.37)/100,a=Math.cos(h*DEG),b=Math.sin(h*DEG);
        let first=Infinity;
        for(let face=0;face<3;face++) {
            const k=basis[face],computed=[k[0]*a*a*a+k[1]*a*a*b+k[2]*a+k[3]*b,k[4]+k[5]*a*a+k[6]*a*b,k[7]*a+k[8]*b];
            const wanted=cubic(profile,face,h);
            for(let j=0;j<3;j++)maxBasis=Math.max(maxBasis,Math.abs(computed[j]-wanted[j]));
            try{first=Math.min(first,root(profile,face,h));}catch(e){if(!String(e).includes('no root'))throw e;}
        }
        assert(first>0&&Number.isFinite(first)&&first<profile.dualray.rootLimit);
        maxRoot=Math.max(maxRoot,first);
    }
    assert(maxBasis<2e-14,`${id} basis ${maxBasis}`);
    console.log(`${id}: max basis difference ${maxBasis}, sampled lower-root maximum ${maxRoot}`);
    const seed=seedCode(fits,seedSplit,"rust");
    const content=`// Generated by scripts/generate-dualray.mjs; do not edit.
// ${id}: ${id==='display-p3'?'pinned incumbent data':'target-fitted lower-root seeds'}.
pub const BASIS: [[f64; 9]; 3] = ${array(basis)};
pub const SECTORS: [[Float; 2]; 2] = ${array(lines)};
#[inline(always)]
${seed}`;
    const factoryName = `create_${id.replaceAll('-', '_')}`;
    // Same expression tree as Rust, emitted inside the shared mapper template.
    // No runtime coefficient loop, dynamic evaluator or hot seed-helper call.
    const jsSeed = seedCode(fits,seedSplit,"js").trimEnd().split("\n").map(line=>"\t\t\t\t"+line).join("\n");
    const mapper = "// Generated by scripts/generate-dualray.mjs; do not edit.\n"
        + template.replace(seedSlot,()=>jsSeed);
    emit(new URL(`../src/generated/dualray-${id}.js`,import.meta.url),mapper);
    jsImports.push(`import { createDualrayKernel as ${factoryName} } from "./dualray-${id}.js";`);
    jsData.push(`${JSON.stringify(id)}: { basis: ${JSON.stringify(basis)}, sectors: ${JSON.stringify(lines)}, rootLimit: ${profile.dualray.rootLimit}, create: ${factoryName} }`);
    const file=new URL(`../rust/src/generated/dualray_${id.replaceAll('-','_')}.rs`,import.meta.url);
    emit(file,content);
}

const jsFile = new URL('../src/generated/dualray.js', import.meta.url);
const jsContent = '// Generated by scripts/generate-dualray.mjs; do not edit.\n'
    + jsImports.join('\n') + '\nconst data = {\n' + jsData.join(',\n') + '\n};\n'
    + 'for (const entry of Object.values(data)) { for (const rows of [entry.basis,entry.sectors]) { rows.forEach(Object.freeze); Object.freeze(rows); } Object.freeze(entry); }\n'
    + 'export const dualrayData = Object.freeze(data);\n';
emit(jsFile,jsContent);

// Exercise the actual emitted f32 kernel, including both sides of the window
// cutovers. A JavaScript approximation would miss Rust/libm rounding changes.
const edgeCheck = execFileSync("cargo", [
    "test", "--release", "--manifest-path", fileURLToPath(new URL("../rust/Cargo.toml", import.meta.url)),
    "float32::tests::dualray_fold_window_edges_cover_every_neighbouring_f32_hue", "--", "--exact", "--nocapture",
], { encoding: "utf8", stdio: ["ignore", "pipe", "inherit"] });
process.stdout.write(edgeCheck);
assert.match(edgeCheck, /test result: ok\. 1 passed; 0 failed/, "Rust f32 edge regression must run, not merely compile");
