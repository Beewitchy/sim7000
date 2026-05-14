use heapless::String;

use super::{AtParseErr, AtParseLine, AtRequest, AtResponse, GenericOk, ResponseCode};

/// AT+GSN
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct GetImei;

impl AtRequest for GetImei {
    type Response = (Imei, GenericOk);
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "AT+GSN\r")
    }
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Imei {
    pub imei: String<16>,
}

impl AtParseLine for Imei {
    fn from_line(line: &str) -> Result<Self, AtParseErr> {
        if ![15, 16].contains(&line.len()) {
            return Err("Invalid length".into());
        }

        if line.chars().any(|c| !c.is_ascii_digit()) {
            return Err("Contains non-digit character".into());
        }

        let provided_check_digit = (line.chars().next_back())
            .expect("string is not empty")
            .to_digit(10)
            .expect("all chars are ascii digits");

        let expected_check_digit = calculate_check_digit(&line[..line.len() - 1]);
        if (provided_check_digit as u8) != expected_check_digit {
            return Err("Imei number has invalid check digit".into());
        }

        Ok(Imei { imei: line.try_into().unwrap_or_default() })
    }
}

/// Calculate the IMEI check digit from an IMEI string
///
/// NOTE: the provided string must not already contain the check digit.
///
/// NOTE: the provided string must contain only ascii digits.
fn calculate_check_digit(imei: &str) -> u8 {
    // the check digit is calculated by iterating over each digit and
    // 1. doubling every other digit
    // 2. summing all digits
    // (if doubling a digit generated two new digits, sum those as well)
    // check digit is (10 - (sum % 10)) % 10

    fn is_even(n: usize) -> bool {
        (n & 1) == 0
    }

    let sum: u32 = imei
        .chars()
        .flat_map(|d| d.to_digit(10))
        .enumerate()
        .map(|(i, d)| {
            if is_even(i) {
                d
            } else {
                let mut doubled = d * 2;
                if doubled >= 10 {
                    doubled = (doubled % 10) + 1
                }
                doubled
            }
        })
        .sum();

    ((10 - (sum % 10)) % 10) as u8
}

impl AtResponse for Imei {
    fn from_generic(code: &mut ResponseCode) -> Option<&mut Self> {
        match code {
            ResponseCode::Imei(v) => Some(v),
            _ => None,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn parse_imei() {
        let valid_imeis = [
            "490154203237518",
            "869951035460918",
            "869931033480910",
            "869951035458235",
        ];

        for valid in valid_imeis {
            let _ = Imei::from_line(valid).expect("failed to parse imei");
        }
    }
}
