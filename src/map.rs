use crate::{
    connection::State,
    creature::Creature,
    otb_io::{
        item_loader::ItemData,
        map_loader::{OtbMapData, OtbMapItem, OtbTile},
    },
    payload::ServerPacketType,
};
use std::vec;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

pub const VIEWPORT_X: u16 = 8;
pub const VIEWPORT_Y: u16 = 6;

#[derive(Clone)]
pub struct Item {
    pub client_id: u16,
    pub items: Vec<Item>,
}

impl Item {
    pub fn from_otb_map_item(
        otb_item: &OtbMapItem,
        server_id_to_client_id: &HashMap<u16, u16>,
    ) -> Item {
        let mut item = Item {
            client_id: *server_id_to_client_id.get(&otb_item.server_id).unwrap(),
            items: vec![],
        };
        for inner_item in otb_item.items.iter() {
            item.items
                .push(Item::from_otb_map_item(inner_item, server_id_to_client_id));
        }
        return item;
    }

    pub fn add_item(&mut self, item: Item) {
        let mut updated_items = vec![item];
        updated_items.append(&mut self.items);
        self.items = updated_items;
    }
}

#[derive(Clone)]
pub struct Tile {
    pub floor_item_client_id: u16,
    pub top_items: Vec<Item>,
    pub bot_items: Vec<Item>,
    pub creatures: Vec<Creature>,
}

impl Tile {
    pub fn form_otb_tile(
        otb_tile: &OtbTile,
        item_data: &HashMap<u16, ItemData>,
        server_id_to_client_id: &HashMap<u16, u16>,
    ) -> Tile {
        let mut tile = Tile {
            floor_item_client_id: *server_id_to_client_id
                .get(&otb_tile.floor_item_server_id)
                .unwrap(),
            top_items: vec![],
            bot_items: vec![],
            creatures: vec![],
        };
        for tile_item in otb_tile.items.iter() {
            let item = Item::from_otb_map_item(tile_item, &server_id_to_client_id);
            let item_info = item_data.get(&item.client_id).unwrap();
            if item_info.top_order < 255 {
                tile.top_items.push(item.clone());
            } else {
                tile.bot_items.push(item.clone());
            }
        }
        return tile;
    }
    pub fn get_item_at_stack_pos(&self, stack_pos: u8) -> Option<Item> {
        let mut counter = 1;
        for item in self.top_items.iter() {
            if counter == stack_pos {
                return Some(item.clone());
            }
            counter += 1;
        }
        counter += self.creatures.len() as u8;
        for item in self.bot_items.iter() {
            if counter == stack_pos {
                return Some(item.clone());
            }
            counter += 1;
        }
        return None;
    }

    pub fn change_at_stack_pos(&mut self, stack_pos: u8, to_item_id: u16) {
        let mut counter = 1;
        for item in self.top_items.iter_mut() {
            if counter == stack_pos {
                item.client_id = to_item_id;
                return;
            }
            counter += 1;
        }
        counter += self.creatures.len() as u8;
        for item in self.bot_items.iter_mut() {
            if counter == stack_pos {
                item.client_id = to_item_id;
                return;
            }
            counter += 1;
        }
    }
}

pub fn create_tile_map(
    map_data: &OtbMapData,
    item_data: &HashMap<u16, ItemData>,
    server_id_to_client_id: &HashMap<u16, u16>,
) -> HashMap<(u16, u16, u8), Tile> {
    let mut map: HashMap<(u16, u16, u8), Tile> = HashMap::new();
    for area in map_data.tile_areas.iter() {
        for tile in area.tiles.iter() {
            let map_tile = Tile::form_otb_tile(tile, item_data, server_id_to_client_id);
            map.insert(
                (area.x + tile.x as u16, area.y + tile.y as u16, area.z),
                map_tile,
            );
        }
    }

    return map;
}

pub fn get_tile_description(tile: &Tile) -> Vec<u8> {
    let mut bytes: Vec<u8> = Vec::new();
    bytes.extend_from_slice(&tile.floor_item_client_id.to_le_bytes());
    for item in &tile.top_items {
        bytes.extend_from_slice(&item.client_id.to_le_bytes());
    }
    for creautre in &tile.creatures {
        bytes.extend_from_slice(&creautre.as_bytes());
    }
    for item in &tile.bot_items {
        bytes.extend_from_slice(&item.client_id.to_le_bytes());
    }
    return bytes;
}

pub fn get_map_description(
    state: Arc<Mutex<State>>,
    from_x: u16,
    to_x: u16,
    from_y: u16,
    to_y: u16,
    from_z: u8,
    to_z: u8,
) -> Vec<u8> {
    let state_handle = state.lock().unwrap();
    let mut map_description: Vec<u8> = Vec::new();
    let mut skip: i32 = -1;
    for z in (from_z..=to_z).rev() {
        for x in from_x..=to_x {
            for y in from_y..=to_y {
                match state_handle.map.get(&(x, y, z as u8)) {
                    Some(tile) => {
                        if skip >= 0 {
                            map_description.push(skip as u8);
                            map_description.push(0xFF);
                        }
                        skip = 0;
                        map_description.extend_from_slice(&get_tile_description(tile));
                    }
                    None => {
                        skip += 1;
                        if skip == 0xFF {
                            map_description.push(0xFF);
                            map_description.push(0xFF);
                            skip = -1;
                        }
                    }
                }
            }
        }
    }
    if skip >= 0 {
        map_description.push(skip as u8);
        map_description.push(0xFF);
    }
    return map_description;
}

pub fn full_map_bound(pos: (u16, u16, u8)) -> (u16, u16, u16, u16) {
    return (
        pos.0 - VIEWPORT_X,
        pos.0 + VIEWPORT_X + 1,
        pos.1 - VIEWPORT_Y,
        pos.1 + VIEWPORT_Y + 1,
    );
}

#[derive(Clone)]
pub enum Direction {
    North,
    South,
    East,
    West,
}

impl Direction {
    pub fn move_in_dir(&self, from: (u16, u16, u8)) -> (u16, u16, u8) {
        return match self {
            Direction::North => (from.0, from.1 - 1, from.2),
            Direction::South => (from.0, from.1 + 1, from.2),
            Direction::East => (from.0 + 1, from.1, from.2),
            Direction::West => (from.0 - 1, from.1, from.2),
        };
    }
    pub fn map_description_bounds(&self, pos: (u16, u16, u8)) -> (u16, u16, u16, u16) {
        return match self {
            Direction::North => (
                pos.0 - VIEWPORT_X,
                pos.0 + VIEWPORT_X + 1,
                pos.1 - VIEWPORT_Y,
                pos.1 - VIEWPORT_Y,
            ),
            Direction::South => (
                pos.0 - VIEWPORT_X,
                pos.0 + VIEWPORT_X + 1,
                pos.1 + VIEWPORT_Y + 1,
                pos.1 + VIEWPORT_Y + 1,
            ),
            Direction::East => (
                pos.0 + VIEWPORT_X + 1,
                pos.0 + VIEWPORT_X + 1,
                pos.1 - VIEWPORT_Y,
                pos.1 + VIEWPORT_Y + 1,
            ),
            Direction::West => (
                pos.0 - VIEWPORT_X,
                pos.0 - VIEWPORT_X,
                pos.1 - VIEWPORT_Y,
                pos.1 + VIEWPORT_Y + 1,
            ),
        };
    }
    pub fn packet_id(&self) -> u8 {
        return match self {
            Direction::North => ServerPacketType::MapNorth as u8,
            Direction::South => ServerPacketType::MapSouth as u8,
            Direction::East => ServerPacketType::MapEast as u8,
            Direction::West => ServerPacketType::MapWest as u8,
        };
    }
}
