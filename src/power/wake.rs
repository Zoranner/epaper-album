#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WakeProbe {
    Timer,
    Button,
    External,
    Ulp,
    Unknown,
    Other(u32),
}

impl WakeProbe {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Timer => "timer",
            Self::Button => "button",
            Self::External => "external",
            Self::Ulp => "ulp",
            Self::Unknown => "unknown",
            Self::Other(_) => "other",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wake_probe_has_external_label() {
        assert_eq!(WakeProbe::External.label(), "external");
    }
}
