use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        mpsc::{Receiver, Sender, TryRecvError},
    },
    thread::sleep,
    time::{Duration, Instant},
};

use crate::{
    StaticData,
    creature::{Character, Creature},
    event_handler::{Command, ServerEvent},
    otb_io::map_loader
};
use tokio::net::TcpStream;

const GAME_WORLD_IP: [u8; 4] = [127, 0, 0, 1];
const GAME_WORLD_PORT: u16 = 7171;
const VIEWPORT_X: u16 = 8;
const VIEWPORT_Y: u16 = 6;
const GROUND_SPEED:u32 = 150;

pub struct State {
    pub map: HashMap<(u16, u16, u8), map_loader::Tile>,
}

pub enum Direction {
    North,
    South,
    East,
    West,
}

impl Direction {
    fn move_in_dir(&self, from: (u16, u16, u8)) -> (u16, u16, u8) {
        return match self {
            Direction::North => (from.0, from.1 - 1, from.2),
            Direction::South => (from.0, from.1 + 1, from.2),
            Direction::East => (from.0 + 1, from.1, from.2),
            Direction::West => (from.0 - 1, from.1, from.2),
        };
    }
    fn map_description_bounds(&self, pos: (u16, u16, u8)) -> (u16, u16, u16, u16) {
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
    fn packet_id(&self) -> u8 {
        return match self {
            Direction::North => ServerPacketType::MapNorth as u8,
            Direction::South => ServerPacketType::MapSouth as u8,
            Direction::East => ServerPacketType::MapEast as u8,
            Direction::West => ServerPacketType::MapWest as u8,
        };
    }
}

pub enum LoginPacketType {
    Motd = 0x14,
    CharacterList = 0x64,
}

pub enum ClientPacketType {
    MoveNorth = 0x65,
    PutIntoGame = 0x0A,
}

pub enum ServerPacketType {
    GameInit = 0x0A,
    ThingMoved = 0x6d,
    FullMap = 0x64,
    MapNorth = 0x65,
    MapEast = 0x66,
    MapSouth = 0x67,
    MapWest = 0x68,
    TileUpdate = 0x69,
    RemoveCreature = 0x6C,
    CreatureAdded = 0x6A
}

pub struct Connection {
    pub id: u32,
    pub read_buffer: Vec<u8>,
    pub read_idx: usize,
    pub write_buffer: Vec<u8>,
    pub write_idx: usize,
    pub socket: TcpStream,
    pub is_logged_in: bool,
    pub state: Arc<Mutex<State>>,
    pub event_handler_in: Sender<Command>,
    pub event_receiver: Receiver<ServerEvent>,
    pub character: Option<Character>,
    pub data: Arc<StaticData>,
    last_moved: Instant,
    pub is_moving: bool
}

impl Connection {
    pub fn new(
        id: u32,
        socket: TcpStream,
        state: Arc<Mutex<State>>,
        event_handler_in: Sender<Command>,
        event_receiver: Receiver<ServerEvent>,
        data: Arc<StaticData>,
    ) -> Self {
        return Connection {
            id,
            socket,
            read_buffer: vec![0; 4096],
            write_buffer: vec![0; 4096],
            is_logged_in: false,
            write_idx: 0,
            read_idx: 0,
            state,
            event_handler_in,
            character: None,
            data,
            event_receiver,
            last_moved: Instant::now(),
            is_moving: false
        };
    }

    pub async fn handle_events(&mut self) {
        let mut payload: Vec<u8> = Vec::new();
        loop {
            match self.event_receiver.try_recv() {
                Ok(event) => match event {
                    ServerEvent::CreatureAdded { cords, creature } => {
                        println!("creature to client received");
                        let tile_update =
                            Connection::tile_update_payload(cords, creature);
                        payload.extend_from_slice(&tile_update);
                    }
                    ServerEvent::CretureMoved {
                        from,
                        to,
                        stack_pos,
                    } => {
                        let creature_moved = Self::thing_moved_payload(from, stack_pos, to);
                        payload.extend_from_slice(&creature_moved);
                    }
                    ServerEvent::EnterGame => {
                        self.send_enter_game().await;
                    }
                    ServerEvent::CreatureRemoved { cords, stack_pos } => {
                        self.send_creature_removed(cords, stack_pos).await;
                    }
                },
                Err(TryRecvError::Empty) => break,
                Err(_) => panic!("Event handler for connection closed"),
            }
        }
        if payload.len() > 0 {
            self.send_packet(&payload).await;
        }
    }

    pub async fn login(&mut self) {
        let mut login_payload: Vec<u8> = Vec::new();
        login_payload.push(LoginPacketType::CharacterList as u8);
        login_payload.push(self.data.characters.len() as u8);
        for character in self.data.characters.iter() {
            login_payload.extend_from_slice(&Self::str_fmt(&character.name));
            login_payload.extend_from_slice(&Self::str_fmt(&character.world));
            login_payload.extend_from_slice(&GAME_WORLD_IP);
            login_payload.extend_from_slice(&GAME_WORLD_PORT.to_le_bytes());
        }
        login_payload.extend_from_slice(&1u16.to_le_bytes()); // premium days
        println!("seinding packet login");
        self.send_packet(&login_payload).await;
    }

    pub fn get_map_description(
        &self,
        from_x: u16,
        to_x: u16,
        from_y: u16,
        to_y: u16,
        from_z: u8,
        to_z: u8,
    ) -> Vec<u8> {
        let mut map_description: Vec<u8> = Vec::new();
        let mut skip: i32 = -1;
        for z in (from_z..=to_z).rev() {
            for x in from_x..=to_x {
                for y in from_y..=to_y {
                    let state = self.state.lock().unwrap();
                    match state.map.get(&(x, y, z as u8)) {
                        Some(tile) => {
                            if skip >= 0 {
                                map_description.push(skip as u8);
                                map_description.push(0xFF);
                            }
                            skip = 0;
                            map_description
                                .extend_from_slice(&tile.floor_item_client_id.to_le_bytes());
                            for c in &tile.creatures {
                                map_description.extend_from_slice(&c.as_bytes());
                            }
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

    pub async fn send_enter_game(&mut self) {
        let mut payload: Vec<u8> = Vec::new();
        let (pos_x, pos_y, pos_z) = self.character.as_ref().unwrap().position;
        payload.push(ServerPacketType::GameInit as u8);
        payload.extend_from_slice(&(self.id as u32).to_le_bytes());
        payload.extend_from_slice(&50u16.to_le_bytes()); // beat 
        payload.push(0); // can report bugs
        payload.push(ServerPacketType::FullMap as u8);
        payload.extend_from_slice(&pos_x.to_le_bytes());
        payload.extend_from_slice(&pos_y.to_le_bytes());
        payload.push(pos_z);
        payload.extend_from_slice(&self.get_map_description(
            pos_x - VIEWPORT_X,
            pos_x + VIEWPORT_X + 1,
            pos_y - VIEWPORT_Y,
            pos_y + VIEWPORT_Y + 1,
            0,
            7,
        ));
        self.send_packet(&mut payload).await;
    }

    async fn send_packet(&mut self, payload: &[u8]) {
        self.write_buffer = vec![0; 4096];
        self.write_idx = 0;
        self.write_buffer[0..2].copy_from_slice(&(payload.len() as u16).to_le_bytes());
        self.write_idx += 2;
        let i = self.write_idx + payload.len();
        self.write_buffer[self.write_idx..i].copy_from_slice(payload);
        self.write_idx = i;
        self.socket.writable().await.unwrap();
        self.socket
            .try_write(&self.write_buffer[0..self.write_idx])
            .unwrap();
    }

    pub fn get_packet_id(&mut self) -> u8 {
        if self.read_buffer.len() > 0 {
            let id = self.read_buffer[self.read_idx + 2];
            self.read_idx += 3;
            return id;
        }
        return 0;
    }

    pub fn str_fmt(s: &str) -> Vec<u8> {
        let mut bytes: Vec<u8> = Vec::new();
        bytes.extend_from_slice(&(s.len() as u16).to_le_bytes());
        for byte in s.as_bytes().iter() {
            bytes.push(*byte);
        }
        return bytes;
    }

    pub fn read_u8(&mut self) -> u8 {
        let byte = self.read_buffer[self.read_idx];
        self.read_idx += 1;
        return byte;
    }

    pub fn read_u16_le(&mut self) -> u16 {
        let first = self.read_u8();
        let second = self.read_u8();
        return first as u16 | (second as u16) << 8;
    }

    pub fn read_u32_le(&mut self) -> u32 {
        let first = self.read_u16_le();
        let second = self.read_u16_le();
        return first as u32 | (second as u32) << 16;
    }

    pub fn read_str(&mut self) -> String {
        let mut text = String::new();
        let str_len = self.read_u16_le();
        for _ in 0..str_len {
            text.push(self.read_u8() as char);
        }
        return text;
    }

    pub fn handle_enter_game_packet(&mut self) {
        let _client_os = self.read_u16_le();
        let _version = self.read_u16_le();
        let _ = self.read_u8();
        let _account_number = self.read_u32_le();
        let name = self.read_str();
        let _password = self.read_str();
        for character in self.data.characters.iter() {
            if character.name == name {
                self.character = Some(character.clone());
                self.character.as_mut().unwrap().id = self.id;
                let _ = self.event_handler_in.send(Command::EnterGame {
                    character_creature: self.character.as_ref().unwrap().as_creature(),
                    cords: character.position,
                });
            }
        }
    }

    pub fn _parse_login_payload(&mut self) {
        let _ = self.read_u16_le();
        let _client_version = self.read_u16_le();
        self.read_idx += 12;
        let _account_number = self.read_u32_le();
        let _password = self.read_str();
    }

    pub fn tile_update_payload(
        pos: (u16, u16, u8),
        creature: Creature,
    ) -> Vec<u8> {
        let mut payload: Vec<u8> = Vec::new();
        payload.push(ServerPacketType::CreatureAdded as u8);
        payload.extend_from_slice(&pos.0.to_le_bytes());
        payload.extend_from_slice(&pos.1.to_le_bytes());
        payload.push(pos.2);
        payload.extend_from_slice(&creature.as_bytes());
        return payload;
    }

    pub fn thing_moved_payload(
        from: (u16, u16, u8),
        stack_pos: u8,
        to: (u16, u16, u8),
    ) -> Vec<u8> {
        let mut payload: Vec<u8> = Vec::new();
        payload.push(ServerPacketType::ThingMoved as u8);
        payload.extend_from_slice(&from.0.to_le_bytes());
        payload.extend_from_slice(&from.1.to_le_bytes());
        payload.push(from.2);
        payload.push(stack_pos);
        payload.extend_from_slice(&to.0.to_le_bytes());
        payload.extend_from_slice(&(to.1).to_le_bytes());
        payload.push(to.2);
        return payload;
    }

    pub async fn send_move_character(&mut self, direction: Direction) {
        // let mut sleep_time = GROUND_SPEED * 1000 / self.character.as_ref().unwrap().speed as u32;
        let sleep_time = 50;
        sleep(Duration::from_millis(sleep_time as u64));
        let mut payload: Vec<u8> = Vec::new();
        let from = self.character.as_ref().unwrap().position;
        let to = direction.move_in_dir(from);
        self.character.as_mut().unwrap().position = to;
        let _ = self.event_handler_in.send(Command::MoveCreature {
            from,
            to,
            creature_id: self.id,
            sender_id: self.id,
        });
        payload.extend_from_slice(&Self::thing_moved_payload(from, 1, to));
        payload.push(direction.packet_id());
        let (from_map_x, to_map_x, form_map_y, to_map_y) = direction.map_description_bounds(from);
        let map_desc =
            self.get_map_description(from_map_x, to_map_x, form_map_y, to_map_y, 0, from.2);
        payload.extend_from_slice(&map_desc);
        self.is_moving = true;
        self.send_packet(&payload).await;
    }

    pub async fn send_creature_removed(&mut self, cords: (u16, u16, u8), stack_pos: u8) {
        let mut payload: Vec<u8> = Vec::new();
        payload.push(ServerPacketType::RemoveCreature as u8);
        payload.extend_from_slice(&cords.0.to_le_bytes());
        payload.extend_from_slice(&cords.1.to_le_bytes());
        payload.push(cords.2);
        payload.push(stack_pos);
        self.send_packet(&payload).await;
    }
}
