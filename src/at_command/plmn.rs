#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct Mcc(u16);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub(super) enum MncLen {
    Short,
    Long,
}

impl Default for MncLen {
    fn default() -> Self {
        Self::Long
    }
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(packed)]
pub struct Mnc(u16, MncLen);

impl Mcc {
    #[cfg(test)]
    pub(crate) const fn new(num: u16) -> Self {
        Self(num)
    }

    pub const fn from_str(digits: &str) -> Option<Self> {
        match digits.len() {
            3 => {
                let Ok(value) = u16::from_str_radix(digits, 10) else { return None; };
                Some(Self(value))
            }
            _ => None
        }
    }

    pub const fn to_num(&self) -> u16 {
        self.0
    }
}

impl core::fmt::Display for Mcc {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let value = self.0;
        write!(f, "{value:03}")
    }
}

#[cfg(feature = "defmt")]
impl defmt::Format for Mcc {
    fn format(&self, fmt: defmt::Formatter) {
        let value = self.0;
        defmt::write!(fmt, "{:03}", value)
    }
}

impl Mnc {
    #[cfg(test)]
    pub(crate) const fn new_short(num: u16) -> Self {
        Self(num, MncLen::Short)
    }

    #[cfg(test)]
    pub(crate) const fn new_long(num: u16) -> Self {
        Self(num, MncLen::Long)
    }

    pub const fn from_str(digits: &str) -> Option<Self> {
        match digits.len() {
            3 => {
                let Ok(value) = u16::from_str_radix(digits, 10) else { return None; };
                Some(Self(value, MncLen::Long))
            }
            2 => {
                let Ok(value) = u16::from_str_radix(digits, 10) else { return None; };
                Some(Self(value, MncLen::Short))
            }
            _ => None
        }
    }

    /// Returns the numeric representation of the mnc code.
    ///
    /// IMPORTANT: this loses information about the length of the code!
    pub const fn to_num(&self) -> u16 {
        self.0
    }

    pub const fn is_two_digits(&self) -> bool {
        matches!(self.1, MncLen::Short)
    }
}

impl core::fmt::Display for Mnc {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let value = self.0;
        match self.1 {
            MncLen::Long => write!(f, "{value:03}"),
            MncLen::Short => write!(f, "{value:02}")
        }
    }
}

#[cfg(feature = "defmt")]
impl defmt::Format for Mnc {
    fn format(&self, fmt: defmt::Formatter) {
        let value = self.0;
        match self.1 {
            MncLen::Long => defmt::write!(fmt, "{:03}", value),
            MncLen::Short => defmt::write!(fmt, "{:02}", value)
        }
    }
}