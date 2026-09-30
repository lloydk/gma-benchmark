//! D65 RGB definitions from CSS Color 4, 26 September 2026, sections 10 and 19.
//! https://www.w3.org/TR/2026/CRD-css-color-4-20260926/#color-conversion-code
//!
//! These f64 definitions are used to compose matrices at compile time. Each
//! arithmetic lane rounds the resulting coefficients to its own scalar type.
pub(crate) type Matrix = [[f64; 3]; 3];

pub(crate) struct Definition {
    pub name: &'static str,
    pub rgb_to_xyz: Matrix,
    pub xyz_to_rgb: Matrix,
}

// One marker per physical gamut, shared by the f32 and f64 implementations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SpaceId {
    Srgb,
    DisplayP3,
    Rec2020,
}
pub(crate) trait RgbSpace: Copy {
    const ID: SpaceId;
    const DEFINITION: &'static Definition;
}
#[derive(Clone, Copy, Debug)]
pub(crate) struct Srgb;
#[derive(Clone, Copy, Debug)]
pub(crate) struct DisplayP3;
#[derive(Clone, Copy, Debug)]
pub(crate) struct Rec2020;
impl RgbSpace for Srgb {
    const ID: SpaceId = SpaceId::Srgb;
    const DEFINITION: &'static Definition = &SRGB;
}
impl RgbSpace for DisplayP3 {
    const ID: SpaceId = SpaceId::DisplayP3;
    const DEFINITION: &'static Definition = &DISPLAY_P3;
}
impl RgbSpace for Rec2020 {
    const ID: SpaceId = SpaceId::Rec2020;
    const DEFINITION: &'static Definition = &REC2020;
}

pub(crate) const SRGB: Definition = Definition {
    name: "srgb",
    rgb_to_xyz: [
        [506752.0 / 1228815.0, 87881.0 / 245763.0, 12673.0 / 70218.0],
        [87098.0 / 409605.0, 175762.0 / 245763.0, 12673.0 / 175545.0],
        [7918.0 / 409605.0, 87881.0 / 737289.0, 1001167.0 / 1053270.0],
    ],
    xyz_to_rgb: [
        [12831.0 / 3959.0, -329.0 / 214.0, -1974.0 / 3959.0],
        [
            -851781.0 / 878810.0,
            1648619.0 / 878810.0,
            36519.0 / 878810.0,
        ],
        [705.0 / 12673.0, -2585.0 / 12673.0, 705.0 / 667.0],
    ],
};

pub(crate) const DISPLAY_P3: Definition = Definition {
    name: "display-p3",
    rgb_to_xyz: [
        [
            608311.0 / 1250200.0,
            189793.0 / 714400.0,
            198249.0 / 1000160.0,
        ],
        [
            35783.0 / 156275.0,
            247089.0 / 357200.0,
            198249.0 / 2500400.0,
        ],
        [0.0, 32229.0 / 714400.0, 5220557.0 / 5000800.0],
    ],
    xyz_to_rgb: [
        [
            446124.0 / 178915.0,
            -333277.0 / 357830.0,
            -72051.0 / 178915.0,
        ],
        [-14852.0 / 17905.0, 63121.0 / 35810.0, 423.0 / 17905.0],
        [11844.0 / 330415.0, -50337.0 / 660830.0, 316169.0 / 330415.0],
    ],
};

pub(crate) const REC2020: Definition = Definition {
    name: "rec2020",
    rgb_to_xyz: [
        [
            63426534.0 / 99577255.0,
            20160776.0 / 139408157.0,
            47086771.0 / 278816314.0,
        ],
        [
            26158966.0 / 99577255.0,
            472592308.0 / 697040785.0,
            8267143.0 / 139408157.0,
        ],
        [0.0, 19567812.0 / 697040785.0, 295819943.0 / 278816314.0],
    ],
    xyz_to_rgb: [
        [
            30757411.0 / 17917100.0,
            -6372589.0 / 17917100.0,
            -4539589.0 / 17917100.0,
        ],
        [
            -19765991.0 / 29648200.0,
            47925759.0 / 29648200.0,
            467509.0 / 29648200.0,
        ],
        [
            792561.0 / 44930125.0,
            -1921689.0 / 44930125.0,
            42328811.0 / 44930125.0,
        ],
    ],
};

pub(crate) const LMS_TO_XYZ: Matrix = [
    [1.2268798758459243, -0.5578149944602171, 0.2813910456659647],
    [-0.0405757452148008, 1.1122868032803170, -0.0717110580655164],
    [-0.0763729366746601, -0.4214933324022432, 1.5869240198367816],
];
pub(crate) const XYZ_TO_LMS: Matrix = [
    [0.8190224379967030, 0.3619062600528904, -0.1288737815209879],
    [0.0329836539323885, 0.9292868615863434, 0.0361446663506424],
    [0.0481771893596242, 0.2642395317527308, 0.6335478284694309],
];
