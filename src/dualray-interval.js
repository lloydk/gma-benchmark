// For p(x)=1+a*x+b*x²+d*x³ on [0,u], these are the two
// interior Bernstein controls. Callers must check both endpoints separately.
export function interiorWithin(a, b, u, guard) {
 const limit = 3 * guard, linear = a * u;
 const c1 = 3 + linear, c2 = 3 + 2 * linear + b * u * u;
 return c1 >= 0 && c1 <= limit && c2 >= 0 && c2 <= limit;
}
