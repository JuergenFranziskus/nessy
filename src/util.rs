pub fn get_flag_u8(byte: u8, flag: u8) -> bool {
    byte & (1 << flag) != 0
}
pub fn set_flag_u8(byte: &mut u8, flag: u8, value: bool) {
    let mask = 1 << flag;
    *byte &= !mask;
    *byte |= if value { mask } else { 0 };
}

pub fn get_flag_u16(short: u16, flag: u16) -> bool {
    short & (1 << flag) != 0
}
pub fn set_flag_u16(short: &mut u16, flag: u16, value: bool) {
    let mask = 1 << flag;
    *short &= !mask;
    *short |= if value { mask } else { 0 };
}
