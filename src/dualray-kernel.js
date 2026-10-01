// Shared numeric helpers for the generated Dualray mappers.
export function value (d, b, a, x) {
	return ((d * x + b) * x + a) * x + 1;
}

export function polish (x, d, b, a) {
	const dx=d*x,q=dx+b,r=q*x+a;
	const f=r*x+1,halfSecond=(dx+dx)+q,f1=(dx+q)*x+r;
	const denominator=f1*f1-f*halfSecond;
	return denominator !== 0 ? x-(f*f1)/denominator : x;
}

// Cold path. Partition at derivative roots so a shallow negative interval
// cannot be skipped by endpoint bracketing. Used for both lower-root
// guard failures and upper-face/convergence failures.
export function firstRoot (d, b, a, constant, limit) {
	let s0 = limit;
	let s1 = limit;
	if (d === 0) {
		const s = -a / (2 * b);
		if (s > 0 && s < limit) s0 = s;
	} else {
		const discriminant = b * b - 3 * d * a;
		if (discriminant >= 0) {
			const q = -b - (b < 0 ? -1 : 1) * Math.sqrt(discriminant);
			const t0 = q / (3 * d);
			const t1 = q === 0 ? 0 : a / q;
			const low = Math.min(t0, t1);
			const high = Math.max(t0, t1);
			if (low > 0 && low < limit) s0 = low;
			if (high > 0 && high < limit) {
				if (s0 === limit) s0 = high;
				else s1 = high;
			}
		}
	}
	let lo = 0;
	const negative = constant < 0;
	for (let interval = 0; interval < 3; interval++) {
		let hi = interval === 0 ? s0 : interval === 1 ? s1 : limit;
		const fhi = ((d * hi + b) * hi + a) * hi + constant;
		if (fhi === 0 && hi === limit) {
			const slope=(3*d*hi+2*b)*hi+a;
			if (negative ? slope>0 : slope<0) return hi;
		}
		if (negative ? fhi>0 : fhi<0) {
			for (let step = 0; step < 64; step++) {
				const mid = lo + (hi - lo) * 0.5;
				if (mid === lo || mid === hi) break;
				const f = ((d * mid + b) * mid + a) * mid + constant;
				if (negative ? f<=0 : f>=0) {
					lo = mid;
				} else hi = mid;
			}
			return lo + (hi - lo) * 0.5;
		}
		lo = hi;
		if (hi === limit) break;
	}
	return Infinity;
}

export function lowerExit(rd,rb,ra,gd,gb,ga,bd,bb,ba,limit) {
	let root=firstRoot(rd,rb,ra,1,limit),face=0;
	const g=firstRoot(gd,gb,ga,1,Math.min(root,limit));
	if(g<root){root=g;face=1;}
	const b=firstRoot(bd,bb,ba,1,Math.min(root,limit));
	if(b<root){root=b;face=2;}
	return [root,face];
}
export function mapFold(rows,l3,target,inputU,limit,encode,out) {
	let u=Math.min(inputU,limit),face=-1,faceValue=0;
	for(let i=0;i<3;i++) {
		const [d,b,a]=rows[i],root=firstRoot(d,b,a,1,u);
		if(root<=u){u=root;face=i;}
	}
	if(Number.isFinite(target))for(let i=0;i<3;i++) {
		const [d,b,a]=rows[i],root=firstRoot(-d,-b,-a,target-1,u);
		if(root<=u){u=root;face=i;faceValue=1;}
	}
	for(let i=0;i<3;i++)out[i]=i===face?faceValue:encode(l3*value(...rows[i],u));
	return out;
}
