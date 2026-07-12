use super::{AtParseErr, AtParseLine, AtResponse, ResponseCode};

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct GenericOk;

#[derive(Clone, Copy, Debug)]
pub enum SimError {
    /// Generic error
    Generic,

    /// Error relating to mobile equipment or to the network.
    CmeErr { code: u32 },

    /// Error relating to message service or to the network.
    CmsErr { code: u32 },
}

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct WritePrompt;

#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct CloseOk {
    pub connection: usize,
}

impl AtParseLine for GenericOk {
    fn from_line(line: &str, _instant: &embassy_time::Instant) -> Result<Self, AtParseErr> {
        // TODO: SHUT OK should be seperate type
        (line == "OK" || line == "SHUT OK")
            .then(|| GenericOk)
            .ok_or(AtParseErr::Mismatch)
    }
}

impl AtResponse for GenericOk {
    fn from_generic(code: &mut ResponseCode) -> Option<&mut Self> {
        match code {
            ResponseCode::Ok(ok) => Some(ok),
            _ => None,
        }
    }
}


impl AtParseLine for SimError {
    fn from_line(line: &str, _instant: &embassy_time::Instant) -> Result<Self, AtParseErr> {
        if let Some(code) = line.strip_prefix("+CME ERROR:") {
            Ok(SimError::CmeErr {
                code: code.trim_start().parse()?,
            })
        } else if let Some(code) = line.strip_prefix("+CMS ERROR:") {
            Ok(SimError::CmsErr {
                code: code.trim_start().parse()?,
            })
        } else if line == "ERROR" {
            Ok(SimError::Generic)
        } else {
            Err(AtParseErr::Mismatch)
        }
    }
}

impl AtParseLine for WritePrompt {
    fn from_line(line: &str, _instant: &embassy_time::Instant) -> Result<Self, AtParseErr> {
        line.starts_with(">")
            .then(|| WritePrompt)
            .ok_or(AtParseErr::Mismatch)
    }
}

impl AtResponse for WritePrompt {
    fn from_generic(code: &mut ResponseCode) -> Option<&mut Self> {
        match code {
            ResponseCode::WritePrompt(prompt) => Some(prompt),
            _ => None,
        }
    }
}

impl AtParseLine for CloseOk {
    fn from_line(line: &str, _instant: &embassy_time::Instant) -> Result<Self, AtParseErr> {
        let connection = line
            .strip_suffix(", CLOSE OK")
            .ok_or(AtParseErr::Mismatch)?
            .parse()?;

        Ok(CloseOk { connection })
    }
}

impl AtResponse for CloseOk {
    fn from_generic(code: &mut ResponseCode) -> Option<&mut Self> {
        match code {
            ResponseCode::CloseOk(close_ok) => Some(close_ok),
            _ => None,
        }
    }
}

#[cfg(feature = "defmt")]
impl defmt::Format for SimError {
    fn format(&self, fmt: defmt::Formatter) {
        use defmt::write;
        match self {
            Self::Generic => write!(fmt, "'ERROR'"),
            Self::CmeErr { code } => match code {
                0 => write!(fmt, "0: phone failure"),
                1 => write!(fmt, "1: no connection to phone"),
                2 => write!(fmt, "2: phone-adaptor link reserved"),
                3 => write!(fmt, "3: operation not allowed"),
                4 => write!(fmt, "4: operation not supported"),
                5 => write!(fmt, "5: PH-SIM PIN required"),
                6 => write!(fmt, "6: PH-FSIM PIN required"),
                7 => write!(fmt, "7: PH-FSIM PUK required"),
                10 => write!(fmt, "10: SIM not inserted"),
                11 => write!(fmt, "11: SIM PIN required"),
                12 => write!(fmt, "12: SIM PUK required"),
                13 => write!(fmt, "13: SIM failure"),
                14 => write!(fmt, "14: SIM busy"),
                15 => write!(fmt, "15: SIM wrong"),
                16 => write!(fmt, "16: incorrect password"),
                17 => write!(fmt, "17: SIM PIN2 required"),
                18 => write!(fmt, "18: SIM PUK2 required"),
                20 => write!(fmt, "20: memory full"),
                21 => write!(fmt, "21: invalid index"),
                22 => write!(fmt, "22: not found"),
                23 => write!(fmt, "23: memory failure"),
                24 => write!(fmt, "24: text string too long"),
                25 => write!(fmt, "25: invalid characters in text string"),
                26 => write!(fmt, "26: dial string too long"),
                27 => write!(fmt, "27: invalid characters in dial string"),
                30 => write!(fmt, "30: no network service"),
                31 => write!(fmt, "31: network timeout"),
                32 => write!(fmt, "32: network not allowed - emergency call only"),
                40 => write!(fmt, "40: network personalisation PIN required"),
                41 => write!(fmt, "41: network personalisation PUK required"),
                42 => write!(fmt, "42: network subset personalisation PIN required"),
                43 => write!(fmt, "43: network subset personalisation PUK required"),
                44 => write!(fmt, "44: service provider personalisation PIN required"),
                45 => write!(fmt, "45: service provider personalisation PUK required"),
                46 => write!(fmt, "46: corporate personalisation PIN required"),
                47 => write!(fmt, "47: corporate personalisation PUK required"),
                99 => write!(fmt, "99: resource limitation"),
                100 => write!(fmt, "100: unknown"),
                103 => write!(fmt, "103: Illegal MS"),
                106 => write!(fmt, "106: Illegal ME"),
                107 => write!(fmt, "107: GPRS services not allowed"),
                111 => write!(fmt, "111: PLMN not allowed"),
                112 => write!(fmt, "112: Location area not allowed"),
                113 => write!(fmt, "113: Roaming not allowed in this location area"),
                132 => write!(fmt, "132: service option not supported"),
                133 => write!(fmt, "133: requested service option not subscribed"),
                134 => write!(fmt, "134: service option temporarily out of order"),
                148 => write!(fmt, "148: unspecified GPRS error"),
                149 => write!(fmt, "149: PDP authentication failure"),
                150 => write!(fmt, "150: invalid mobile class"),
                160 => write!(fmt, "160: DNS resolve failed"),
                161 => write!(fmt, "161: Socket open failed"),
                171 => write!(fmt, "171: MMS task is busy now"),
                172 => write!(fmt, "172: The MMS data is oversize"),
                173 => write!(fmt, "173: The operation is overtime"),
                174 => write!(fmt, "174: There is no MMS receiver"),
                175 => write!(fmt, "175: The storage for address is full"),
                176 => write!(fmt, "176: Not find the address"),
                177 => write!(fmt, "177: The connection to network is failed"),
                178 => write!(fmt, "178: Failed to read push message"),
                179 => write!(fmt, "179: This is not a push message"),
                180 => write!(fmt, "180: gprs is not attached"),
                181 => write!(fmt, "181: tcpip stack is busy"),
                182 => write!(fmt, "182: The MMS storage is full"),
                183 => write!(fmt, "183: The box is empty"),
                184 => write!(fmt, "184: failed to save MMS"),
                185 => write!(fmt, "185: It is in edit mode"),
                186 => write!(fmt, "186: It is not in edit mode"),
                187 => write!(fmt, "187: No content in the buffer"),
                188 => write!(fmt, "188: Not find the file"),
                189 => write!(fmt, "189: Failed to receive MMS"),
                190 => write!(fmt, "190: Failed to read MMS"),
                191 => write!(fmt, "191: Not M-Notification.ind"),
                192 => write!(fmt, "192: The MMS inclosure is full"),
                193 => write!(fmt, "193: Unknown"),
                600 => write!(fmt, "600: No Error"),
                601 => write!(fmt, "601: Unrecognized Command"),
                602 => write!(fmt, "602: Return Value Error"),
                603 => write!(fmt, "603: Syntax Error"),
                604 => write!(fmt, "604: Unspecified Error"),
                605 => write!(fmt, "605: Data Transfer Already"),
                606 => write!(fmt, "606: Action Already"),
                607 => write!(fmt, "607: Not At Cmd"),
                608 => write!(fmt, "608: Multi Cmd too long"),
                609 => write!(fmt, "609: Abort Cops"),
                610 => write!(fmt, "610: No Call Disc"),
                611 => write!(fmt, "611: BT SAP Undefined"),
                612 => write!(fmt, "612: BT SAP Not Accessible"),
                613 => write!(fmt, "613: BT SAP Card Removed"),
                614 => write!(fmt, "614: AT Not Allowed By Customer"),
                753 => write!(fmt, "753: missing required cmd parameter"),
                754 => write!(fmt, "754: invalid SIM command"),
                755 => write!(fmt, "755: invalid File Id"),
                756 => write!(fmt, "756: missing required P1/2/3 parameter"),
                757 => write!(fmt, "757: invalid P1/2/3 parameter"),
                758 => write!(fmt, "758: missing required command data"),
                759 => write!(fmt, "759: invalid characters in command data"),
                765 => write!(fmt, "765: Invalid input value"),
                766 => write!(fmt, "766: Unsupported mode"),
                767 => write!(fmt, "767: Operation failed"),
                768 => write!(fmt, "768: Mux already running"),
                769 => write!(fmt, "769: Unable to get control"),
                770 => write!(fmt, "770: SIM network reject"),
                771 => write!(fmt, "771: Call setup in progress"),
                772 => write!(fmt, "772: SIM powered down"),
                773 => write!(fmt, "773: SIM file not present"),
                791 => write!(fmt, "791: Param count not enough"),
                792 => write!(fmt, "792: Param count beyond"),
                793 => write!(fmt, "793: Param value range beyond"),
                794 => write!(fmt, "794: Param type not match"),
                795 => write!(fmt, "795: Param format invalid"),
                796 => write!(fmt, "796: Get a null param"),
                797 => write!(fmt, "797: CFUN state is 0 or 4"),
                code @ _ => write!(fmt, "CME({=u32}) <unknown error>", *code)
            },
            Self::CmsErr { code } => match code {
                1 => write!(fmt, "1: Unassigned(unallocated) number"),
                3 => write!(fmt, "3: No route to destination"),
                6 => write!(fmt, "6: Channel unacceptable"),
                8 => write!(fmt, "8: Operator determined barring"),
                10 => write!(fmt, "10: Call barred"),
                11 => write!(fmt, "11: Reserved"),
                16 => write!(fmt, "16: Normal call clearing"),
                17 => write!(fmt, "17: User busy"),
                18 => write!(fmt, "18: No user responding"),
                19 => write!(fmt, "19: User alerting, no answer"),
                21 => write!(fmt, "21: Short message transfer rejected"),
                22 => write!(fmt, "22: Number changed"),
                25 => write!(fmt, "25: Pre-emption"),
                26 => write!(fmt, "26: Non-selected user clearing"),
                27 => write!(fmt, "27: Destination out of service"),
                28 => write!(fmt, "28: Invalid number format (incomplete number)"),
                29 => write!(fmt, "29: Facility rejected"),
                30 => write!(fmt, "30: Response to STATUS ENQUIRY"),
                32 => write!(fmt, "32: Normal, unspecified"),
                34 => write!(fmt, "34: No circuit/channel available"),
                38 => write!(fmt, "38: Network out of order"),
                41 => write!(fmt, "41: Temporary failure"),
                42 => write!(fmt, "42: Switching equipment Congestion"),
                43 => write!(fmt, "43: Access information discarded"),
                44 => write!(fmt, "44: Requested circuit/channel not available"),
                47 => write!(fmt, "47: Resources unavailable, unspecified"),
                49 => write!(fmt, "49: Quality of service unavailable"),
                50 => write!(fmt, "50: Requested facility not subscribed"),
                55 => write!(fmt, "55: Requested facility not subscribed"),
                57 => write!(fmt, "57: Bearer capability not authorized"),
                58 => write!(fmt, "58: Bearer capability not presently available"),
                63 => write!(fmt, "63: Service or option not available, unspecified"),
                65 => write!(fmt, "65: Bearer service not implemented"),
                68 => write!(fmt, "68: ACM equal or greater than ACM maximum"),
                69 => write!(fmt, "69: Requested facility not implemented"),
                70 => write!(fmt, "70: Only restricted digital information bearer capability is available"),
                79 => write!(fmt, "79: Service or option not implemented, unspecified"),
                81 => write!(fmt, "81: Invalid transaction identifier value"),
                87 => write!(fmt, "87: User not member of CUG"),
                88 => write!(fmt, "88: Incompatible destination"),
                91 => write!(fmt, "91: Invalid transit network selection"),
                95 => write!(fmt, "95: Semantically incorrect message"),
                96 => write!(fmt, "96: Invalid mandatory information"),
                97 => write!(fmt, "97: Message type non-existent or not implemented"),
                98 => write!(fmt, "98: Message type not compatible with protocol state"),
                99 => write!(fmt, "99: Information element non-existent or not implemented"),
                100 => write!(fmt, "100: Conditional information element error"),
                101 => write!(fmt, "101: Message not compatible with protocol"),
                102 => write!(fmt, "102: Recovery on timer expiry"),
                111 => write!(fmt, "111: Protocol error, unspecified"),
                127 => write!(fmt, "127: Interworking, unspecified"),
                128 => write!(fmt, "128: Telematic interworking not supported"),
                129 => write!(fmt, "129: Short message Type 0 not supported"),
                130 => write!(fmt, "130: Cannot replace short message"),
                143 => write!(fmt, "143: Unspecified TP-PID error"),
                144 => write!(fmt, "144: Data coding scheme (alphabet) not supported"),
                145 => write!(fmt, "145: Message class not supported"),
                159 => write!(fmt, "159: Unspecified TP-DCS error"),
                160 => write!(fmt, "160: Command cannot be acted"),
                161 => write!(fmt, "161: Command unsupported"),
                175 => write!(fmt, "175: Unspecified TP-Command error"),
                176 => write!(fmt, "176: TPDU not supported"),
                192 => write!(fmt, "192: SC busy"),
                193 => write!(fmt, "193: No SC subscription"),
                194 => write!(fmt, "194: SC system failure"),
                195 => write!(fmt, "195: Invalid SME address"),
                196 => write!(fmt, "196: Destination SME barred"),
                197 => write!(fmt, "197: SM Rejected-Duplicate SM"),
                198 => write!(fmt, "198: TP-VPF not supported"),
                199 => write!(fmt, "199: TP-VP not supported"),
                208 => write!(fmt, "208: SIM SMS storage full"),
                209 => write!(fmt, "209: No SMS storage capability in SIM"),
                210 => write!(fmt, "210: Error in MS"),
                211 => write!(fmt, "211: Memory Capacity Exceeded"),
                212 => write!(fmt, "212: SIM Application Toolkit Busy"),
                213 => write!(fmt, "213: SIM data download error"),
                224 => write!(fmt, "224: CP retry exceed"),
                225 => write!(fmt, "225: RP trim timeout"),
                226 => write!(fmt, "226: SMS connection broken"),
                255 => write!(fmt, "255: Unspecified error cause"),
                300 => write!(fmt, "300: ME failure"),
                301 => write!(fmt, "301: SMS reserved"),
                302 => write!(fmt, "302: operation not allowed"),
                303 => write!(fmt, "303: operation not supported"),
                304 => write!(fmt, "304: invalid PDU mode"),
                305 => write!(fmt, "305: invalid text mode"),
                310 => write!(fmt, "310: SIM not inserted"),
                311 => write!(fmt, "311: SIM pin necessary"),
                312 => write!(fmt, "312: PH SIM pin necessary"),
                313 => write!(fmt, "313: SIM failure"),
                314 => write!(fmt, "314: SIM busy"),
                315 => write!(fmt, "315: SIM wrong"),
                316 => write!(fmt, "316: SIM PUK required"),
                317 => write!(fmt, "317: SIM PIN2 required"),
                318 => write!(fmt, "318: SIM PUK2 required"),
                320 => write!(fmt, "320: memory failure"),
                321 => write!(fmt, "321: invalid memory index"),
                322 => write!(fmt, "322: memory full"),
                323 => write!(fmt, "323: invalid input parameter"),
                324 => write!(fmt, "324: invalid input format"),
                325 => write!(fmt, "325: invalid input value"),
                330 => write!(fmt, "330: SMSC address unknown"),
                331 => write!(fmt, "331: no network"),
                332 => write!(fmt, "332: network timeout"),
                340 => write!(fmt, "340: no cnma ack"),
                500 => write!(fmt, "500: Unknown"),
                512 => write!(fmt, "512: SMS no error"),
                513 => write!(fmt, "513: Message length exceeds maximum length"),
                514 => write!(fmt, "514: Invalid request parameters"),
                515 => write!(fmt, "515: ME storage failure"),
                516 => write!(fmt, "516: Invalid bearer service"),
                517 => write!(fmt, "517: Invalid service mode"),
                518 => write!(fmt, "518: Invalid storage type"),
                519 => write!(fmt, "519: Invalid message format"),
                520 => write!(fmt, "520: Too many MO concatenated messages"),
                521 => write!(fmt, "521: SMSAL not ready"),
                522 => write!(fmt, "522: SMSAL no more service"),
                523 => write!(fmt, "523: Not support TP-Status-Report & TP-Command in storage"),
                524 => write!(fmt, "524: Reserved MTI"),
                525 => write!(fmt, "525: No free entity in RL layer"),
                526 => write!(fmt, "526: The port number is already registered"),
                527 => write!(fmt, "527: There is no free entity for port number"),
                528 => write!(fmt, "528: More Message to Send state error"),
                529 => write!(fmt, "529: MO SMS is not allow"),
                530 => write!(fmt, "530: GPRS is suspended"),
                531 => write!(fmt, "531: ME storage full"),
                532 => write!(fmt, "532: Doing SIM refresh"),
                code @ _ => write!(fmt, "CMS({=u32}) <unknown error>", *code)
            }
        }
    }
}