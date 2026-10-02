// Compiled twice with a concrete Float alias: all arithmetic, caches and LUT
// entries use that precision. SINGLE branches are resolved at compile time.
include!("conditioning.rs");

pub(crate) mod compensated {
    include!("compensated.rs");
}

pub(crate) mod dualray {
    include!("dualray.rs");
}

pub(crate) mod dualray_fast {
    include!("dualray_fast.rs");
}

pub(crate) mod css_minde {
    include!("css_minde.rs");
}
pub(crate) mod gamut {
    include!("gamut.rs");
}
mod transfer {
    include!("transfer.rs");
}
pub(crate) mod color {
    include!("color.rs");
}
pub(crate) mod clip {
    include!("clip.rs");
}
mod p3_compat {
    include!("p3_compat.rs");
}
use color::{KA0, KA1, KA2, KB0, KB1, KB2};
#[cfg(test)]
use p3_compat::*;
include!("polynomial.rs");
pub(crate) mod rgb_solvers {
    include!("rgb_solvers.rs");
}

#[cfg(test)]
pub(crate) mod rgb_tests {
    include!("rgb_tests.rs");
}

const PI: Float = std::f64::consts::PI as Float;

pub(crate) mod bottosson {
    include!("bottosson.rs");
}

pub(crate) mod edge_seeker {
    include!("edge_seeker.rs");
}
