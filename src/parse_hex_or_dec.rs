use core::num::ParseIntError;

pub(crate) trait ParseHexOrDec: Sized {
    fn parse_hex_or_dec(s: &str) -> Result<Self, ParseIntError>;
}

impl ParseHexOrDec for i64 {
    fn parse_hex_or_dec(s: &str) -> Result<i64, ParseIntError> {
        if let Some(s) = s.strip_prefix("0x") {
            i64::from_str_radix(s, 16)
        } else {
            i64::from_str_radix(s, 10)
        }
    }
}

impl ParseHexOrDec for u64 {
    fn parse_hex_or_dec(s: &str) -> Result<u64, ParseIntError> {
        if let Some(s) = s.strip_prefix("0x") {
            u64::from_str_radix(s, 16)
        } else {
            u64::from_str_radix(s, 10)
        }
    }
}

impl ParseHexOrDec for i32 {
    fn parse_hex_or_dec(s: &str) -> Result<i32, ParseIntError> {
        if let Some(s) = s.strip_prefix("0x") {
            i32::from_str_radix(s, 16)
        } else {
            i32::from_str_radix(s, 10)
        }
    }
}

impl ParseHexOrDec for u32 {
    fn parse_hex_or_dec(s: &str) -> Result<u32, ParseIntError> {
        if let Some(s) = s.strip_prefix("0x") {
            u32::from_str_radix(s, 16)
        } else {
            u32::from_str_radix(s, 10)
        }
    }
}

impl ParseHexOrDec for i16 {
    fn parse_hex_or_dec(s: &str) -> Result<i16, ParseIntError> {
        if let Some(s) = s.strip_prefix("0x") {
            i16::from_str_radix(s, 16)
        } else {
            i16::from_str_radix(s, 10)
        }
    }
}

impl ParseHexOrDec for u16 {
    fn parse_hex_or_dec(s: &str) -> Result<u16, ParseIntError> {
        if let Some(s) = s.strip_prefix("0x") {
            u16::from_str_radix(s, 16)
        } else {
            u16::from_str_radix(s, 10)
        }
    }
}

impl ParseHexOrDec for i8 {
    fn parse_hex_or_dec(s: &str) -> Result<i8, ParseIntError> {
        if let Some(s) = s.strip_prefix("0x") {
            i8::from_str_radix(s, 16)
        } else {
            i8::from_str_radix(s, 10)
        }
    }
}

impl ParseHexOrDec for u8 {
    fn parse_hex_or_dec(s: &str) -> Result<u8, ParseIntError> {
        if let Some(s) = s.strip_prefix("0x") {
            u8::from_str_radix(s, 16)
        } else {
            u8::from_str_radix(s, 10)
        }
    }
}