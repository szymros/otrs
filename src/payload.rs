use std::sync::{Arc, Mutex};

use crate::{
    connection::{Container, State},
    creature::{Character, Creature},
    map::{Direction, VIEWPORT_X, VIEWPORT_Y, get_map_description},
};

const GAME_WORLD_IP: [u8; 4] = [127, 0, 0, 1];
const GAME_WORLD_PORT: u16 = 7171;

pub enum LoginPacketType {
    Motd = 0x14,
    CharacterList = 0x64,
}

pub enum MagicEffect {
    Puff = 0x02,
    EnergyArea = 0x0A,
}

pub enum ServerPacketType {
    GameInit = 0x0A,
    ThingTransformed = 0x6B,
    CreatureTurn = 0x63,
    FullMap = 0x64,
    MapNorth = 0x65,
    MapEast = 0x66,
    MapSouth = 0x67,
    MapWest = 0x68,
    TileUpdate = 0x69,
    AddThing = 0x6A,
    RemoveThing = 0x6C,
    ThingMoved = 0x6D,
    OpenContainer = 0x6E,
    CloseContainer = 0x6F,
    AddItemToContainer = 0x70,
    RemoveItemFromContainer = 0x72,
    AddItemToInventory = 0x78,
    RemoveItemFromInventory = 0x79,
    MagicEffect = 0x83,
    CreatureSpeak = 0xAA,
}

pub enum SpeechType {
    Say = 0x01,
}

pub fn write_str(s: &str) -> Vec<u8> {
    let mut bytes: Vec<u8> = Vec::new();
    bytes.extend_from_slice(&(s.len() as u16).to_le_bytes());
    for byte in s.as_bytes().iter() {
        bytes.push(*byte);
    }
    return bytes;
}

pub fn write_position(pos: &(u16, u16, u8)) -> Vec<u8> {
    let mut payload: Vec<u8> = Vec::new();
    payload.extend_from_slice(&pos.0.to_le_bytes());
    payload.extend_from_slice(&pos.1.to_le_bytes());
    payload.push(pos.2);
    return payload;
}

pub fn creature_added_payload(pos: &(u16, u16, u8), creature: Creature) -> Vec<u8> {
    let mut payload: Vec<u8> = Vec::new();
    payload.push(ServerPacketType::AddThing as u8);
    payload.extend_from_slice(&write_position(pos));
    payload.extend_from_slice(&creature.as_bytes());
    return payload;
}

pub fn thing_moved_payload(from: &(u16, u16, u8), stack_pos: u8, to: &(u16, u16, u8)) -> Vec<u8> {
    let mut payload: Vec<u8> = Vec::new();
    payload.push(ServerPacketType::ThingMoved as u8);
    payload.extend_from_slice(&write_position(from));
    payload.push(stack_pos);
    payload.extend_from_slice(&write_position(to));
    return payload;
}
pub fn add_item_to_container_payload(client_item_id: &u16, container_id: u8) -> Vec<u8> {
    let mut payload: Vec<u8> = Vec::new();
    payload.push(ServerPacketType::AddItemToContainer as u8);
    payload.push(container_id);
    payload.extend_from_slice(&client_item_id.to_le_bytes());
    return payload;
}

pub fn remove_item_from_container_payload(container_id: u8, slot: u8) -> Vec<u8> {
    let mut payload: Vec<u8> = Vec::new();
    payload.push(ServerPacketType::RemoveItemFromContainer as u8);
    payload.push(container_id);
    payload.push(slot);
    return payload;
}

pub fn add_item_to_inventory_payload(client_item_id: u16, slot: u8) -> Vec<u8> {
    let mut payload: Vec<u8> = Vec::new();
    payload.push(ServerPacketType::AddItemToInventory as u8);
    payload.push(slot);
    payload.extend_from_slice(&client_item_id.to_le_bytes());
    return payload;
}

pub fn remove_item_from_inventory_payload(slot: u8) -> Vec<u8> {
    let mut payload: Vec<u8> = Vec::new();
    payload.push(ServerPacketType::RemoveItemFromInventory as u8);
    payload.push(slot);
    return payload;
}
pub fn remove_thing_payload(pos: &(u16, u16, u8), stack_pos: u8) -> Vec<u8> {
    let mut payload: Vec<u8> = Vec::new();
    payload.push(ServerPacketType::RemoveThing as u8);
    payload.extend_from_slice(&write_position(pos));
    payload.push(stack_pos);
    return payload;
}

pub fn add_thing_payload(to: &(u16, u16, u8), item_id: &u16) -> Vec<u8> {
    let mut payload: Vec<u8> = Vec::new();
    payload.push(ServerPacketType::AddThing as u8);
    payload.extend_from_slice(&write_position(to));
    payload.extend_from_slice(&item_id.to_le_bytes());
    return payload;
}
pub fn container_payload(container: &Container, name: &str, capacity: u8, parent: u8) -> Vec<u8> {
    let mut payload: Vec<u8> = Vec::new();
    payload.push(ServerPacketType::OpenContainer as u8);
    payload.push(container.container_id);
    payload.extend_from_slice(&container.client_id.to_le_bytes());
    payload.extend_from_slice(&write_str(&name));
    payload.push(capacity);
    payload.push(parent);
    payload.push(container.items.len() as u8);
    for inner_item in container.items.iter() {
        payload.extend_from_slice(&inner_item.client_id.to_le_bytes());
    }
    return payload;
}

pub fn close_container_payload(container_id: u8) -> Vec<u8> {
    let mut payload: Vec<u8> = Vec::new();
    payload.push(ServerPacketType::CloseContainer as u8);
    payload.push(container_id);
    return payload;
}

pub fn enter_game_payload(
    state: Arc<Mutex<State>>,
    pos: &(u16, u16, u8),
    character_id: u32,
) -> Vec<u8> {
    let mut payload: Vec<u8> = Vec::new();
    payload.push(ServerPacketType::GameInit as u8);
    payload.extend_from_slice(&character_id.to_le_bytes());
    payload.extend_from_slice(&50u16.to_le_bytes()); // beat 
    payload.push(0); // can report bugs
    payload.push(ServerPacketType::FullMap as u8);
    payload.extend_from_slice(&write_position(&pos));
    payload.extend_from_slice(&get_map_description(
        state.clone(),
        pos.0 - VIEWPORT_X,
        pos.0 + VIEWPORT_X + 1,
        pos.1 - VIEWPORT_Y,
        pos.1 + VIEWPORT_Y + 1,
        0,
        7,
    ));
    payload.push(0x82); //world light
    payload.push(0x6F);
    payload.push(0xD7);
    return payload;
}

pub fn login_payload(characters: &Vec<Character>) -> Vec<u8> {
    let mut payload: Vec<u8> = Vec::new();
    payload.push(LoginPacketType::CharacterList as u8);
    payload.push(characters.len() as u8);
    for character in characters.iter() {
        payload.extend_from_slice(&write_str(&character.name));
        payload.extend_from_slice(&write_str(&character.world));
        payload.extend_from_slice(&GAME_WORLD_IP);
        payload.extend_from_slice(&GAME_WORLD_PORT.to_le_bytes());
    }
    payload.extend_from_slice(&1u16.to_le_bytes()); // premium days
    return payload;
}

pub fn map_direction_payload(
    state: Arc<Mutex<State>>,
    direction: Direction,
    to: (u16, u16, u8),
) -> Vec<u8> {
    let mut payload: Vec<u8> = Vec::new();
    payload.push(direction.packet_id());
    let (from_map_x, to_map_x, form_map_y, to_map_y) = direction.map_description_bounds(to);
    let map_desc = get_map_description(
        state.clone(),
        from_map_x,
        to_map_x,
        form_map_y,
        to_map_y,
        0,
        to.2,
    );
    payload.extend_from_slice(&map_desc);
    return payload;
}

pub fn thing_transformed_payload(pos: &(u16, u16, u8), stack_pos: u8, id: Option<u16>) -> Vec<u8> {
    let mut payload: Vec<u8> = Vec::new();
    payload.push(ServerPacketType::ThingTransformed as u8);
    payload.extend_from_slice(&write_position(&pos));
    payload.push(stack_pos);
    if let Some(i) = id {
        payload.extend_from_slice(&i.to_le_bytes());
    }
    return payload;
}

pub fn magic_effect_payload(pos: &(u16, u16, u8), effect_type: u8) -> Vec<u8> {
    let mut payload: Vec<u8> = Vec::new();
    payload.push(ServerPacketType::MagicEffect as u8);
    payload.extend_from_slice(&write_position(&pos));
    payload.push(effect_type + 1);
    return payload;
}

pub fn speech_payload(
    text: &str,
    speaker_name: &str,
    speech_type: SpeechType,
    pos: &(u16, u16, u8),
) -> Vec<u8> {
    let mut payload: Vec<u8> = Vec::new();
    payload.push(ServerPacketType::CreatureSpeak as u8);
    payload.extend_from_slice(&write_str(speaker_name));
    payload.push(speech_type as u8);
    payload.extend_from_slice(&write_position(pos));
    payload.extend_from_slice(&write_str(text));
    return payload;
}

pub fn creature_turn_payload(direction: Direction, creature_id: u32) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.push(ServerPacketType::CreatureTurn as u8);
    payload.push(0);
    payload.extend_from_slice(&creature_id.to_le_bytes());
    payload.push(direction as u8);
    return payload;
}
