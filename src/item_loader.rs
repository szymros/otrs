use std::collections::HashMap;
use crate::otb_loader::{is_block_end, read_u8, read_u16_le};


/*
*   file start with a block of headers that contain version and so on it a block like any other so
*   it starts with 0xFE and then nested inside this block are item blocks
* 
*   each item block starts with 0xFE and then end with 0xFF
*   0xFE
*   u8 - item type
*   u32 - item flags
*   N attributes
*   0xFF
*
*   each attribute has format
*   u8 - attribute id
*   u16 - attribute contents len
*   attribute contents
*   ...
*/


const BLOCK_START: u8 = 0xFE;
const BLOCK_END: u8 = 0xFF;

const ITEM_SERVER_ID_ATTR: u8 = 0x10;
const ITEM_CLIENT_ID_ATTR: u8 = 0x11;
const ITEM_NAME_ATTR: u8 = 0x12;

pub struct Item {
   pub server_id: u16,
   pub client_id: u16,
   pub item_type: u8,
   pub flags: u32,
   pub attributes: Vec<u8>,
   pub item_name: String,
}

pub fn read_otb_items(filepath: &str) -> HashMap<u16,Item>{
    println!("reading {}", filepath);
    let mut items_map: HashMap<u16, Item> = HashMap::new();
    let bytes: Vec<u8> = std::fs::read(filepath).unwrap();
    let mut idx: usize = 0;
    // skip to where the data actually starts
    for _ in 0..2 {
        loop {
            let next = read_u8(&mut idx, &bytes);
            if next == BLOCK_START {
                // align to be before start of first item block
                idx -= 1;
                break;
            }
        }
    }
    while idx < bytes.len() {
        if is_block_end(idx, &bytes) {
            break;
        }
        let next = read_u8(&mut idx, &bytes);
        if next == BLOCK_START {
            let item = parse_item_block(&mut idx, &bytes);
            items_map.insert(item.server_id, item);
        }
    }
    println!("done parsing {}", filepath);
    return items_map;
}

fn read_str(idx: &mut usize, bytes: &[u8]) -> String {
    let str_len = read_u16_le(idx, bytes);
    let mut item_name = String::from("");
    for _ in 0..str_len {
        let byte = read_u8(idx, bytes);
        item_name.push(byte as char);
    }
    return item_name;
}

fn parse_item_block(idx: &mut usize, bytes: &[u8]) -> Item {
    let item_type = read_u8(idx, bytes);
    // skip flags for now
    *idx += 4;
    let mut server_id: u16 = 0;
    let mut client_id: u16 = 0;
    let mut item_name: String = "".to_string();
    loop {
        if is_block_end(*idx, bytes) {
            *idx += 1;
            break;
        }
        let next = read_u8(idx, bytes);
        match next {
            ITEM_SERVER_ID_ATTR => {
                if server_id != 0 {
                    continue;
                }
                *idx += 2;
                server_id = read_u16_le(idx, bytes)
            }
            ITEM_CLIENT_ID_ATTR => {
                if client_id != 0 {
                    continue;
                }
                *idx += 2;
                client_id = read_u16_le(idx, bytes)
            }
            ITEM_NAME_ATTR => {
                if item_name.len() > 0 {
                    continue;
                }
                item_name = read_str(idx, bytes);
            }
            _ => (),
        }
    }
    return Item {
        server_id,
        client_id,
        item_type,
        item_name,
        flags: 0,
        attributes: vec![],
    };
}
