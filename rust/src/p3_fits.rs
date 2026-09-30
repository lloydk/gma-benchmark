// Incumbent Bottosson approximation data; not part of the RGB definition.
use super::Float;

// ── Bottosson Display-P3 cusp approximation constants ──
pub(super) const BOTTOSSON_EPSILON: Float = 1e-12;
pub(super) const P3_RED1: Float = -1.772343927512981;
pub(super) const P3_RED2: Float = -0.8207587433674072;
pub(super) const P3_GREEN1: Float = 1.8031987175305495;
pub(super) const P3_GREEN2: Float = -1.1932813966558915;

pub(super) const P3_RED_K0: Float = 1.1941401817282744;
pub(super) const P3_RED_K1: Float = 1.7629811997119493;
pub(super) const P3_RED_K2: Float = 0.5958599382477117;
pub(super) const P3_RED_K3: Float = 0.7575999740542505;
pub(super) const P3_RED_K4: Float = 0.5681684967813678;
pub(super) const P3_GREEN_K0: Float = 0.7395668192259771;
pub(super) const P3_GREEN_K1: Float = -0.45954279991477065;
pub(super) const P3_GREEN_K2: Float = 0.08285308768965816;
pub(super) const P3_GREEN_K3: Float = 0.1254116495192955;
pub(super) const P3_GREEN_K4: Float = -0.14503290744357106;
pub(super) const P3_BLUE_K0: Float = 1.3650944117698118;
pub(super) const P3_BLUE_K1: Float = -0.013962295571040945;
pub(super) const P3_BLUE_K2: Float = -1.1452305089885595;
pub(super) const P3_BLUE_K3: Float = -0.5025987876721942;
pub(super) const P3_BLUE_K4: Float = 0.003174713114731378;
