// Intersection equations shared by the two generated mapper bodies.
if ((arg_l1 - arg_l0) * arg_cuspC - (arg_cuspL - arg_l0) * arg_c1 <= 0) {
	const denom = arg_c1 * arg_cuspL + arg_cuspC * (arg_l0 - arg_l1);
	t = denom === 0 ? 0 : (arg_cuspC * arg_l0) / denom;
}
else {
	const denom = arg_c1 * (arg_cuspL - 1) + arg_cuspC * (arg_l0 - arg_l1);
	t = denom === 0 ? 0 : (arg_cuspC * (arg_l0 - 1)) / denom;

	const dl = arg_l1 - arg_l0;
	const ldtBase = dl + arg_c1 * arg_q0;
	const mdtBase = dl + arg_c1 * arg_q1;
	const sdtBase = dl + arg_c1 * arg_q2;
	const L = arg_l0 + t * (arg_l1 - arg_l0);
	const C = t * arg_c1;
	const l = L + C * arg_q0;
	const m = L + C * arg_q1;
	const s = L + C * arg_q2;
	const l2 = l * l;
	const m2 = m * m;
	const s2 = s * s;
	const l3 = l2 * l;
	const m3 = m2 * m;
	const s3 = s2 * s;
	const ldt = 3 * ldtBase * l2;
	const mdt = 3 * mdtBase * m2;
	const sdt = 3 * sdtBase * s2;
	const ldt2 = 6 * ldtBase * ldtBase * l;
	const mdt2 = 6 * mdtBase * mdtBase * m;
	const sdt2 = 6 * sdtBase * sdtBase * s;

	const r = RL * l3 + RM * m3 + RS * s3 - 1;
	const r1 = RL * ldt + RM * mdt + RS * sdt;
	const r2 = RL * ldt2 + RM * mdt2 + RS * sdt2;
	const ur = r1 / (r1 * r1 - 0.5 * r * r2);
	const tr = ur >= 0 ? -r * ur : Number.MAX_VALUE;

	const g = GL * l3 + GM * m3 + GS * s3 - 1;
	const g1 = GL * ldt + GM * mdt + GS * sdt;
	const g2 = GL * ldt2 + GM * mdt2 + GS * sdt2;
	const ug = g1 / (g1 * g1 - 0.5 * g * g2);
	const tg = ug >= 0 ? -g * ug : Number.MAX_VALUE;

	const blue = BL * l3 + BM * m3 + BS * s3 - 1;
	const blue1 = BL * ldt + BM * mdt + BS * sdt;
	const blue2 = BL * ldt2 + BM * mdt2 + BS * sdt2;
	const ub = blue1 / (blue1 * blue1 - 0.5 * blue * blue2);
	const tb = ub >= 0 ? -blue * ub : Number.MAX_VALUE;

	t += Math.min(tr, Math.min(tg, tb));
}
