pub mod map_loader; 
pub mod item_loader;

pub const OTB_BLOCK_START: u8 = 0xFE;
pub const OTB_BLOCK_END: u8 = 0xFF;
pub const OTB_ESCAPE_CHARACTER: u8 = 0xFD;

pub fn read_u8_otb(idx: &mut usize, bytes: &[u8]) -> u8 {
    let mut byte = bytes[*idx];
    if byte == OTB_ESCAPE_CHARACTER
        && (bytes[*idx + 1] == OTB_BLOCK_END
            || bytes[*idx + 1] == OTB_BLOCK_START
            || bytes[*idx + 1] == OTB_ESCAPE_CHARACTER)
    {
        *idx += 1;
        byte = bytes[*idx];
    }
    *idx += 1;
    return byte;
}

pub fn read_u16_le_otb(idx: &mut usize, bytes: &[u8]) -> u16 {
    let first = read_u8_otb(idx, bytes);
    let second = read_u8_otb(idx, bytes);
    let word = first as u16 | ((second as u16) << 8);
    return word;
}


pub fn is_otb_block_end(idx: usize, bytes: &[u8]) -> bool {
    return bytes[idx] == OTB_BLOCK_END;
}

pub fn skip_otb_block(idx: &mut usize, bytes: &[u8]) {
    while bytes[*idx] != OTB_BLOCK_END {
        *idx += 1;
    }
    *idx += 1;
}

pub fn read_str_otb(idx: &mut usize, bytes: &[u8]) -> String {
    let str_len = read_u16_le_otb(idx, bytes);
    let mut item_name = String::from("");
    for _ in 0..str_len {
        let byte = read_u8_otb(idx, bytes);
        item_name.push(byte as char);
    }
    return item_name;
}
