use crate::otb_io::{
    OTB_BLOCK_START, is_otb_block_end, read_str_otb, read_u8_otb, read_u16_le_otb,
};
use std::collections::HashMap;

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

const ITEM_SERVER_ID_ATTR: u8 = 0x10;
const ITEM_CLIENT_ID_ATTR: u8 = 0x11;
const ITEM_NAME_ATTR: u8 = 0x12;
const ITEM_TOP_ORDER: u8 = 0x2B;

pub enum ItemType {
    Nothing = 0,
    Ground = 1,
    Container = 2,
    Weapon = 3,
    Ammunition = 4,
    Armor = 5,
    Rune = 6,
    Teleport = 7,
    Magicfield = 8,
    Writeable = 9,
    Key = 10,
    Splash = 11,
    Fluid = 12,
    Last = 13, // Not sure what is that
}

impl ItemType {
    pub fn from_u8(byte: u8) -> ItemType {
        match byte {
            0 => ItemType::Nothing,
            1 => ItemType::Ground,
            2 => ItemType::Container,
            3 => ItemType::Weapon,
            4 => ItemType::Ammunition,
            5 => ItemType::Armor,
            6 => ItemType::Rune,
            7 => ItemType::Teleport,
            8 => ItemType::Magicfield,
            9 => ItemType::Writeable,
            10 => ItemType::Key,
            11 => ItemType::Splash,
            12 => ItemType::Fluid,
            13 => ItemType::Last,
            _ => ItemType::Nothing,
        }
    }
}

pub struct ItemData {
    pub server_id: u16,
    pub client_id: u16,
    pub item_type: ItemType,
    pub flags: u32,
    pub attributes: Vec<u8>,
    pub item_name: String,
    pub top_order: u8,
}

pub fn read_otb_items(filepath: &str) -> (HashMap<u16, ItemData>, HashMap<u16, u16>) {
    println!("reading {}", filepath);
    let mut items_map: HashMap<u16, ItemData> = HashMap::new();
    let mut server_id_to_client_id: HashMap<u16, u16> = HashMap::new();
    let bytes: Vec<u8> = std::fs::read(filepath).unwrap();
    let mut idx: usize = 0;
    // skip to where the data actually starts
    for _ in 0..2 {
        loop {
            let next = read_u8_otb(&mut idx, &bytes);
            if next == OTB_BLOCK_START {
                // align to be before start of first item block
                idx -= 1;
                break;
            }
        }
    }
    while idx < bytes.len() {
        if is_otb_block_end(idx, &bytes) {
            break;
        }
        let next = read_u8_otb(&mut idx, &bytes);
        if next == OTB_BLOCK_START {
            let item = parse_item_block(&mut idx, &bytes);
            let server_id = item.server_id;
            let client_id = item.client_id;
            items_map.insert(item.client_id, item);
            server_id_to_client_id.insert(server_id, client_id);
        }
    }
    println!("done parsing {}", filepath);
    return (items_map, server_id_to_client_id);
}

fn parse_item_block(idx: &mut usize, bytes: &[u8]) -> ItemData {
    let item_type = ItemType::from_u8(read_u8_otb(idx, bytes));
    // skip flags for now
    *idx += 4;
    let mut server_id: u16 = 0;
    let mut client_id: u16 = 0;
    let mut item_name: String = "".to_string();
    let mut top_order: u8 = 255;
    let mut item_byte: Vec<u8> = Vec::new();
    loop {
        if is_otb_block_end(*idx, bytes) {
            *idx += 1;
            break;
        }
        let next = read_u8_otb(idx, bytes);
        item_byte.push(next);
        match next {
            ITEM_SERVER_ID_ATTR => {
                if server_id != 0 {
                    continue;
                }
                *idx += 2;
                server_id = read_u16_le_otb(idx, bytes);
                item_byte.extend_from_slice(&server_id.to_le_bytes());
            }
            ITEM_CLIENT_ID_ATTR => {
                if client_id != 0 {
                    continue;
                }
                *idx += 2;
                client_id = read_u16_le_otb(idx, bytes);
                item_byte.extend_from_slice(&client_id.to_le_bytes());
            }
            ITEM_NAME_ATTR => {
                if item_name.len() > 0 {
                    continue;
                }
                item_name = read_str_otb(idx, bytes);
                item_byte.extend_from_slice(&item_name.as_bytes());
            }
            ITEM_TOP_ORDER => {
                top_order = read_u8_otb(idx, bytes);
                item_byte.push(top_order);
            }
            _ => (),
        }
    }
    return ItemData {
        server_id,
        client_id,
        item_type,
        top_order,
        item_name,
        flags: 0,
        attributes: vec![],
    };
}
