use crate::creature::Creature;
use crate::otb_io::{
    OTB_BLOCK_START, is_otb_block_end, read_u8_otb, read_u16_le_otb, skip_otb_block,
    item_loader::Item
};
use std::collections::HashMap;

/*
* OTBM format
* uses little endian
* each node i started with 0xFE and ended with 0xFF
* header
*   version 4 bytes
*   map width 2 bytes
*   map height 2 bytes
*   items major version 4 bytes
*   items minor version 4 bytes
* map data 0x02
*   aditional properties N bytes
*       desc
*       house filename
*       spawn filename
*   waypoints 0x0F
*       waypoint 0x10
*           name N bytes
*           x 2 bytes
*           y 2 bytes
*           z 1 byte
*   towns 0x0C
*       town 0x0D
*           id 2 bytes
*           name N bytes
*           temple position x 2 bytes
*           temple position y 2 bytes
*           temple position y 1 bytes
*   tile area 0x04
*       x 2 bytes
*       y 2 bytes
*       z 1 byte
*       tile 0x05
*           x 1 byte elative to tile area
*           y 1 byte relative to tile area
*           additional properties N bytes
*           item 0x06
*               id 2 bytes
*               additional properties N bytes
*               nested item 0x06
*       housetile 0x0E
*           x 2 bytes
*           y 2 bytes
*           id 4 bytes
*           additional properties N bytes
*
* additional properties
*    0x01 DESCRIPTION
*    0x02 EXT_FILE
*    0x03 TILE_FLAGS
*    0x04 ACTION_ID
*    0x05 UNIQUE_ID
*    0x06 TEXT
*    0x08 TELE_DEST
*    0x09 ITEM
*    0x0A DEPOT_ID
*    0x0B EXT_SPAWN_FILE
*    0x0D EXT_HOUSE_FILE
*    0x0E HOUSEDOORID
*    0x0F COUNT
*    0x16 RUNE OTBM_ATTR_RUNE_CHARGES
*/

const MAP_DATA_BLOCK_START: u8 = 0x02;
const WAYPOINTS_BLOCK_START: u8 = 0x0F;
const TOWNS_BLOCK_START: u8 = 0x0C;
const TILE_AREA_BLOCK_START: u8 = 0x04;
const TILE_BLOCK_START: u8 = 0x05;
const ITEM_BLOCK_START: u8 = 0x06;
const HOUSE_TILE_BLOCK_START: u8 = 0x06;
const TILE_SPRITE_PROPERTY: u8 = 0x09;

pub struct MapData {
    pub attrs: Vec<u8>,
    pub waypoints: Vec<u8>,
    pub towns: Vec<u8>,
    pub tile_areas: Vec<TileArea>,
}

pub struct TileArea {
    pub x: u16,
    pub y: u16,
    pub z: u8,
    pub tiles: Vec<Tile>,
    pub house_tiles: Vec<Tile>,
}

#[derive(Clone)]
pub struct Tile {
    pub x: u8,
    pub y: u8,
    pub floor_item_server_id: u16,
    pub floor_item_client_id: u16,
    pub items: Vec<MapItem>,
    pub creatures: Vec<Creature>,
}

#[derive(Clone)]
pub struct MapItem {
    pub server_id: u16,
    pub client_id: u16,
    pub items: Vec<MapItem>,
}

impl MapData {
    fn new() -> Self {
        return MapData {
            attrs: vec![],
            waypoints: vec![],
            towns: vec![],
            tile_areas: vec![],
        };
    }
}

pub fn read_file(filepath: &str) -> MapData {
    println!("parsing {}", filepath);
    let bytes: Vec<u8> = std::fs::read(filepath).unwrap();
    let mut idx: usize = 0;
    loop {
        let next = read_u8_otb(&mut idx, &bytes);
        if next == OTB_BLOCK_START {
            let block_id = read_u8_otb(&mut idx, &bytes);
            if block_id == MAP_DATA_BLOCK_START {
                break;
            }
        }
    }
    let mut tile_areas: Vec<TileArea> = Vec::new();
    let towns: Vec<u8> = Vec::new();
    let waypoints: Vec<u8> = Vec::new();
    loop {
        if is_otb_block_end(idx, &bytes) {
            break;
        }
        let next = read_u8_otb(&mut idx, &bytes);
        if next == OTB_BLOCK_START {
            let block_id = read_u8_otb(&mut idx, &bytes);
            match block_id {
                WAYPOINTS_BLOCK_START => {
                    skip_otb_block(&mut idx, &bytes);
                }
                TOWNS_BLOCK_START => {
                    skip_otb_block(&mut idx, &bytes);
                }
                TILE_AREA_BLOCK_START => {
                    let tile_area = parse_tile_area(&bytes, &mut idx);
                    tile_areas.push(tile_area);
                }
                _ => {
                    skip_otb_block(&mut idx, &bytes);
                }
            }
        }
    }
    println!("done parsing {}", filepath);
    return MapData {
        attrs: vec![],
        waypoints,
        towns,
        tile_areas,
    };
}

pub fn create_tile_map(
    map_data: &MapData,
    item_data: &HashMap<u16, Item>,
) -> HashMap<(u16, u16, u8), Tile> {
    let mut map: HashMap<(u16, u16, u8), Tile> = HashMap::new();
    for area in map_data.tile_areas.iter() {
        for tile in area.tiles.iter() {
            let mut t = tile.clone();
            t.floor_item_client_id = item_data.get(&t.floor_item_server_id).unwrap().client_id;
            // for item in t.items.iter_mut() {
            //     item.client_id = item_data.get(&item.server_id).unwrap().client_id;
            // }
            map.insert(
                (area.x + tile.x as u16, area.y + tile.y as u16, area.z),
                t
            );
        }
    }

    return map;
}

pub fn parse_tile_area(bytes: &Vec<u8>, idx: &mut usize) -> TileArea {
    let mut tiles: Vec<Tile> = Vec::new();
    let house_tiles: Vec<Tile> = Vec::new();
    let x = read_u16_le_otb(idx, bytes);
    let y = read_u16_le_otb(idx, bytes);
    let z = read_u8_otb(idx, bytes);
    loop {
        if is_otb_block_end(*idx, bytes) {
            *idx += 1;
            break;
        }
        let next = read_u8_otb(idx, bytes);
        if next == OTB_BLOCK_START {
            let block_id = read_u8_otb(idx, bytes);
            match block_id {
                TILE_BLOCK_START => {
                    let tile = parse_tile(&bytes, idx);
                    tiles.push(tile);
                }
                HOUSE_TILE_BLOCK_START => {
                    skip_otb_block(idx, bytes);
                }
                _ => {
                    skip_otb_block(idx, bytes);
                }
            }
        }
    }
    return TileArea {
        x,
        y,
        z,
        tiles,
        house_tiles,
    };
}

pub fn parse_tile(bytes: &[u8], idx: &mut usize) -> Tile {
    let mut items: Vec<MapItem> = Vec::new();
    let x = read_u8_otb(idx, bytes);
    let y = read_u8_otb(idx, bytes);
    let mut tile_sprite_id: u16 = 0;
    if bytes[*idx] == TILE_SPRITE_PROPERTY {
        *idx += 1;
        tile_sprite_id = read_u16_le_otb(idx, bytes);
    }
    loop {
        if is_otb_block_end(*idx, bytes) {
            *idx += 1;
            break;
        }
        let next = read_u8_otb(idx, bytes);
        if next == OTB_BLOCK_START {
            let block_id = read_u8_otb(idx, bytes);
            match block_id {
                ITEM_BLOCK_START => {
                    let item = parse_items(bytes, idx);
                    items.push(item);
                }
                _ => {
                    skip_otb_block(idx, bytes);
                }
            }
        }
    }
    return Tile {
        x,
        y,
        items,
        floor_item_server_id: tile_sprite_id,
        floor_item_client_id: tile_sprite_id,
        creatures: vec![],
    };
}

pub fn parse_items(bytes: &[u8], idx: &mut usize) -> MapItem {
    let mut items: Vec<MapItem> = Vec::new();
    let id = read_u16_le_otb(idx, bytes);
    loop {
        if is_otb_block_end(*idx, bytes) {
            *idx += 1;
            break;
        }
        let next = read_u8_otb(idx, bytes);
        if next == OTB_BLOCK_START {
            let block_start = read_u8_otb(idx, bytes);
            match block_start {
                ITEM_BLOCK_START => {
                    let nested_item = parse_items(bytes, idx);
                    items.push(nested_item);
                }
                _ => {
                    skip_otb_block(idx, bytes);
                }
            }
        }
    }
    return MapItem {
        server_id: id,
        client_id: id,
        items,
    };
}
