mod otb_loader;
mod item_loader;
mod protocol;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use tokio::net::{TcpListener, TcpStream};

enum PacketType {
    Motd = 0x14,
    CharList = 0x64,
    PutIntoGame = 0x0F,
}

struct Connection {
    read_buffer: Vec<u8>,
    write_buffer: Vec<u8>,
    w_buff: [u8; 10024],
    write_idx: usize,
    socket: TcpStream,
    is_logged_in: bool,
    state: Arc<Mutex<State>>,
}

impl Connection {
    pub fn new(socket: TcpStream, state: Arc<Mutex<State>>) -> Self {
        return Connection {
            socket,
            read_buffer: vec![0; 4096],
            write_buffer: vec![0; 4096],
            w_buff: [0; 10024],
            is_logged_in: false,
            write_idx: 0,
            state,
        };
    }
    pub fn login(&mut self) {
        let mut login_payload: Vec<u8> = Vec::new();
        // login_payload.push(0x14); // packet id motd
        // login_payload.extend_from_slice(&Self::str_fmt("test")); // motd text
        login_payload.push(PacketType::CharList as u8); //packet id
        login_payload.push(0x01); // num of chars
        login_payload.extend_from_slice(&Self::str_fmt("test")); // char name
        login_payload.extend_from_slice(&Self::str_fmt("test")); // world name
        login_payload.extend_from_slice(&[127, 0, 0, 1]); // world ip
        login_payload.extend_from_slice(&7171u16.to_le_bytes());
        login_payload.extend_from_slice(&0x0001u16.to_le_bytes());
        self.prepare_packet(&login_payload);

        println!("payload {:?}", login_payload);
        println!("write_idx {}", self.write_idx);
        println!("{:?}", &self.w_buff[0..self.write_idx]);
        self.socket
            .try_write(&self.w_buff[0..self.write_idx])
            .unwrap();
    }

    pub async fn put_into_game(&mut self) {
        let viewport_x = 8;
        let viewport_y = 6;
        let pos_x: u16 = 1024;
        let pos_y: u16 = 1024;
        let pos_z: u8 = 7;
        let mut payload: Vec<u8> = Vec::new();
        payload.push(0x0a); // not needed?
        payload.extend_from_slice(&[1, 0, 0, 0, 32, 0, 0]); // u32 player id + magic u16 related to client drawing speed should be 0x32 + byte for flag + other flags
        payload.push(0x64); // start map
        payload.extend_from_slice(&pos_x.to_le_bytes()); // player x
        payload.extend_from_slice(&pos_y.to_le_bytes()); // player y
        payload.push(7); // player z
        // //        let player = entity![
        //     player.clone(),
        //     Name("Skyless".into()),
        //     Health { value: 150, max: 150 },
        //     Direction(Directions::South),
        //     Outfit { r#type: 128, head: 78, body: 69, legs: 58, feet: 76, addons: 0 },
        //     LightInfo { level: 0xFF, color: 0x00 },
        //     Speed(220),
        //     Skull(Skulls::None),
        //     PartyShield(PartyShields::None)
        // ];
        let mut skip: i32 = -1;
        for z in (0..=7).rev() {
            for x in pos_x - viewport_x..=pos_x + viewport_x + 1 {
                for y in pos_y - viewport_y..=pos_y + viewport_y + 1 {
                    let state = self.state.lock().unwrap();
                    match state.map.get(&(x, y, z as u8)) {
                        Some(tile) => {
                            if skip >= 0 {
                                payload.push(skip as u8);
                                payload.push(0xFF);
                            }
                            skip = 0;
                            payload.extend_from_slice(&tile.id.to_le_bytes());
                            if x == pos_x && y == pos_y && z == 7 {
                                payload.extend_from_slice(&0x61u16.to_le_bytes());
                                payload.extend_from_slice(&[0, 0, 0, 0]); // idk
                                payload.extend_from_slice(&[1, 0, 0, 0]); // creature id
                                payload.extend_from_slice(&Self::str_fmt("test")); // name
                                payload.push(0); //health
                                payload.push(0); // dir
                                //outfit
                                payload.extend_from_slice(&128u16.to_le_bytes());
                                payload.push(78);
                                payload.push(69);
                                payload.push(58);
                                payload.push(76);
                                // ligth
                                payload.push(0xFF);
                                payload.push(0xD7);

                                payload.push(220);
                                payload.push(0x0);
                                payload.push(0x0);
                            }
                        }
                        None => {
                            skip += 1;
                            if skip == 0xFF {
                                payload.push(0xFF);
                                payload.push(0xFF);
                                skip = -1;
                            }
                        }
                    }
                }
            }
        }
        if skip >= 0 {
            payload.push(skip as u8);
            payload.push(0xFF);
        }
        // for i in 0..18 {
        //     for _ in 0..14 {
        //         payload.extend_from_slice(&3410u16.to_le_bytes());
        //         payload.push(0x00);
        //         payload.push(0xFF);
        //     }
        //         payload.push(0xFF);
        // }

        self.prepare_packet(&payload);
        self.socket.writable().await.unwrap();
        self.socket
            .try_write(&self.w_buff[0..self.write_idx])
            .unwrap();
    }

    fn prepare_packet(&mut self, payload: &[u8]) {
        self.w_buff[0..2].copy_from_slice(&(payload.len() as u16).to_le_bytes());
        self.write_idx += 2;
        let i = self.write_idx + payload.len();
        self.w_buff[self.write_idx..i].copy_from_slice(payload);
        self.write_idx = i;
    }

    pub fn get_packet_id(&self) -> u8 {
        if self.read_buffer.len() > 0 {
            return self.read_buffer[2];
        }
        return 0;
    }

    pub fn push_byte(&mut self, byte: u8) {
        self.write_buffer[self.write_idx] = byte;
        self.write_idx += 1;
    }

    pub fn push_word(&mut self, word: u16) {
        self.write_buffer[self.write_idx] = word as u8;
        self.write_buffer[self.write_idx + 1] = (word >> 8) as u8;
        self.write_idx += 2;
    }

    pub fn push_str(&mut self, s: &str) {
        self.push_word(s.len() as u16);

        for byte in s.as_bytes().iter() {
            self.push_byte(*byte);
        }
    }

    pub fn str_fmt(s: &str) -> Vec<u8> {
        let mut bytes: Vec<u8> = Vec::new();
        bytes.extend_from_slice(&(s.len() as u16).to_le_bytes());
        for byte in s.as_bytes().iter() {
            bytes.push(*byte);
        }
        return bytes;
    }
}

struct State {
    map: HashMap<(u16, u16, u8), otb_loader::Tile>,
}

#[tokio::main]
async fn main() {
    println!("Hello, world!");

    let map_data = otb_loader::read_file("./data/testmap.otbm");
    let map = otb_loader::create_tile_map(&map_data);
    let item_data = item_loader::read_otb_items("./data/items.otb");
    let state = Arc::new(Mutex::new(State { map }));

    let listener = TcpListener::bind("127.0.0.01:7171").await.unwrap();

    loop {
        let (socket, _) = listener.accept().await.unwrap();
        let sc = state.clone();
        tokio::spawn(async move {
            println!("connection");
            on_new_connection(socket, sc).await;
        });
    }
}

async fn on_new_connection(socket: TcpStream, state: Arc<Mutex<State>>) {
    let mut connection = Connection::new(socket, state);

    loop {
        connection.socket.readable().await.unwrap();
        connection.read_buffer = vec![0; 4096];
        match connection.socket.try_read(&mut connection.read_buffer) {
            Ok(0) => break,
            Err(_) => continue,
            Ok(_) => (),
        };
        // println!("{:?}", connection.read_buffer);
        match connection.get_packet_id() {
            0x01 => {
                println!("login");
                connection.login();
            }
            0x0A => {
                println!("put into game");
                connection.put_into_game().await;
            }
            _ => {}
        }
    }
}
