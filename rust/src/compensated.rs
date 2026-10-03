// Native compensated arithmetic for bounded solver operands. This module is
// compiled separately in each lane; no runtime conversion changes precision.
// AArch64 exposes hardware floating-point FMA through `neon`, not x86's `fma`.
use super::Float;
#[cfg(any(
    not(any(
        target_feature = "fma",
        all(target_arch = "aarch64", target_feature = "neon")
    )),
    test
))]
use super::SINGLE;

#[inline(always)]
pub(crate) fn product_error(a: Float, b: Float, product: Float) -> Float {
    #[cfg(any(
        target_feature = "fma",
        all(target_arch = "aarch64", target_feature = "neon")
    ))]
    {
        a.mul_add(b, -product)
    }
    #[cfg(not(any(
        target_feature = "fma",
        all(target_arch = "aarch64", target_feature = "neon")
    )))]
    {
        split_product_error(a, b, product)
    }
}

// Dekker two-product. Callers use bounded coefficients and factors: splitting
// does not overflow for finite products in their compensation paths.
#[cfg(any(
    not(any(
        target_feature = "fma",
        all(target_arch = "aarch64", target_feature = "neon")
    )),
    test
))]
#[inline(always)]
fn split_product_error(a: Float, b: Float, product: Float) -> Float {
    let splitter = if SINGLE { 4097.0 } else { 134217729.0 };
    let split = |x: Float| {
        let t = splitter * x;
        let hi = t - (t - x);
        (hi, x - hi)
    };
    let (ah, al) = split(a);
    let (bh, bl) = split(b);
    ((ah * bh - product) + ah * bl + al * bh) + al * bl
}

// Compensated a*b+c for the bounded Bottosson conditioning path. The fallback
// is an accurate sum, not a general correctly-rounded IEEE FMA emulation.
#[inline(always)]
pub(crate) fn mul_add(a: Float, b: Float, c: Float) -> Float {
    #[cfg(any(
        target_feature = "fma",
        all(target_arch = "aarch64", target_feature = "neon")
    ))]
    {
        a.mul_add(b, c)
    }
    #[cfg(not(any(
        target_feature = "fma",
        all(target_arch = "aarch64", target_feature = "neon")
    )))]
    {
        let product = a * b;
        let residual = product_error(a, b, product);
        let sum = product + c;
        let recovered = sum - product;
        let error = (product - (sum - recovered)) + (c - recovered);
        sum + (error + residual)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_and_fused_product_residuals_agree_on_solver_operands() {
        let mut state = 0x12345678u32;
        for _ in 0..100_000 {
            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
            let a = (state as i32 as Float) / (1u32 << 27) as Float;
            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
            let b = (state as i32 as Float) / (1u32 << 29) as Float;
            let product = a * b;
            let split = split_product_error(a, b, product);
            let fused = a.mul_add(b, -product);
            assert_eq!(split, fused, "{a} * {b}");
            assert_eq!(product_error(a, b, product), fused);
        }
        let epsilon = Float::EPSILON;
        assert_eq!(
            mul_add(1.0 + epsilon, 1.0 - epsilon, -1.0),
            -epsilon * epsilon
        );
    }
}
