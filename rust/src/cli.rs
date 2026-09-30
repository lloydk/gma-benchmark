#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Gamut {
    DisplayP3,
    Srgb,
    Rec2020,
    All,
}

#[derive(Debug)]
pub(crate) struct Options {
    pub gamut: Gamut,
    pub check: bool,
    pub validate_only: bool,
}

impl Options {
    pub fn parse() -> Result<Option<Self>, String> {
        Self::parse_args(std::env::args().skip(1))
    }

    fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Option<Self>, String> {
        let mut options = Self {
            gamut: Gamut::DisplayP3,
            check: false,
            validate_only: false,
        };
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--help" | "-h" => return Ok(None),
                "--in-gamut-check" => options.check = true,
                "--validate-only" => options.validate_only = true,
                _ => {
                    let gamut = if arg == "--gamut" {
                        args.next()
                            .filter(|v| !v.is_empty() && !v.starts_with('-'))
                            .ok_or("--gamut requires a value")?
                    } else if let Some(value) = arg.strip_prefix("--gamut=") {
                        if value.is_empty() {
                            return Err("--gamut requires a value".into());
                        }
                        value.to_owned()
                    } else {
                        return Err(format!("unknown argument: {arg}"));
                    };
                    options.gamut = match gamut.as_str() {
                        "display-p3" => Gamut::DisplayP3,
                        "srgb" => Gamut::Srgb,
                        "rec2020" => Gamut::Rec2020,
                        "all" => Gamut::All,
                        _ => return Err(format!("unsupported gamut: {gamut}")),
                    };
                }
            }
        }
        Ok(Some(options))
    }
}

pub(crate) const HELP: &str =
    "Usage: gma-bench [--gamut display-p3|srgb|rec2020|all] [--in-gamut-check] [--validate-only]
Default: display-p3, all 13 methods, native f64 and f32.
sRGB and Rec.2020 support 8 methods; fitted and table-based methods remain P3-only.
--validate-only runs workload checks without timing; cargo test runs the independent oracle tests.";

#[cfg(test)]
mod tests {
    use super::*;
    fn parse(args: &[&str]) -> Result<Option<Options>, String> {
        Options::parse_args(args.iter().map(|v| (*v).to_owned()))
    }
    #[test]
    fn gamut_requires_a_value_in_both_forms() {
        for args in [
            &["--gamut"][..],
            &["--gamut", "--in-gamut-check"],
            &["--gamut="],
        ] {
            assert_eq!(parse(args).unwrap_err(), "--gamut requires a value");
        }
    }
    #[test]
    fn supported_modes_and_strict_unknown_arguments() {
        for args in [
            &["--gamut", "rec2020", "--in-gamut-check", "--validate-only"][..],
            &["--gamut=rec2020", "--validate-only", "--in-gamut-check"],
        ] {
            let options = parse(args).unwrap().unwrap();
            assert_eq!(options.gamut, Gamut::Rec2020);
            assert!(options.check && options.validate_only);
        }
        assert_eq!(parse(&["--typo"]).unwrap_err(), "unknown argument: --typo");
        assert_eq!(
            parse(&["--gamut=xyz"]).unwrap_err(),
            "unsupported gamut: xyz"
        );
        assert!(parse(&["--help"]).unwrap().is_none());
        assert_eq!(parse(&[]).unwrap().unwrap().gamut, Gamut::DisplayP3);
    }
}
