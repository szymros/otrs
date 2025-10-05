mod connection;
mod creature;
mod event_handler;
mod otb_io;
use std::{
    collections::HashMap,
    io::ErrorKind,
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver, Sender},
    },
};

use crate::connection::{Connection, State};
use crate::event_handler::{Command, ServerEvent, event_handler};
use creature::Character;
use otb_io::item_loader::Item;
use tokio::{
    io::AsyncWriteExt,
    net::TcpListener,
};

pub fn create_characters() -> Vec<Character> {
    let mut characters: Vec<Character> = Vec::new();
    characters.push(Character {
        id: 1,
        name: "Some Character".to_string(),
        outfit_type: 128,
        outfit: [80, 80, 80, 80],
        health: 100,
        max_health: 100,
        look_dir: 0,
        speed: 220,
        world: "World".to_string(),
        position: (1024, 1024, 7),
    });
    characters.push(Character {
        id: 2,
        name: "Another Character".to_string(),
        outfit_type: 128,
        outfit: [30, 30, 30, 30],
        health: 100,
        max_health: 100,
        look_dir: 0,
        speed: 220,
        world: "World".to_string(),
        position: (1024, 1026, 7),
    });
    return characters;
}

struct StaticData {
    item_data: HashMap<u16, Item>,
    characters: Vec<Character>,
}

#[tokio::main]
async fn main() {
    let mut connection_counter = 0;
    let data = Arc::new(StaticData {
        item_data: otb_io::item_loader::read_otb_items("./data/items.otb"),
        characters: create_characters(),
    });
    let map_data = otb_io::map_loader::read_file("./data/testmap.otbm");
    let map = otb_io::map_loader::create_tile_map(&map_data, &data.item_data);
    let state = Arc::new(Mutex::new(State { map }));

    let listener = TcpListener::bind("127.0.0.1:7171").await.unwrap();
    let (event_handler_in, event_handler_rx): (Sender<Command>, Receiver<Command>) =
        mpsc::channel();

    let state_clone = state.clone();
    tokio::spawn(async move {
        event_handler(event_handler_rx, state_clone).await;
    });

    loop {
        connection_counter += 1;
        let (mut socket, _) = listener.accept().await.unwrap();
        println!("new connedction accepted");
        let (tx, rx): (Sender<ServerEvent>, Receiver<ServerEvent>) = mpsc::channel();
        match event_handler_in.send(Command::AddNewConnection {
            tx,
            connection_id: connection_counter,
        }) {
            Ok(_) => (),
            Err(e) => {
                println!("{}", e);
                let _ = socket.shutdown().await;
                continue;
            }
        };
        let connection = Connection::new(
            connection_counter,
            socket,
            state.clone(),
            event_handler_in.clone(),
            rx,
            data.clone(),
        );
        tokio::spawn(async move {
            println!("Spawned thread for new connection");
            on_new_connection(connection).await;
        });
    }
}

async fn on_new_connection(connection: Connection) {
    let mut connection = connection;
    loop {
        connection.handle_events().await;
        connection.read_buffer = vec![0; 4096];
        match connection.socket.try_read(&mut connection.read_buffer) {
            Ok(0) => {
                let _ = connection.event_handler_in.send(Command::CloseConnection {
                    connection_id: connection.id,
                });
                break;
            }
            Err(ref e) if e.kind() == ErrorKind::WouldBlock => continue,
            Err(e) => println!("Error in reading from socket {}", e),
            Ok(_) => {
                connection.read_idx = 0;
            }
        };
        match connection.get_packet_id() {
            0x01 => {
                connection.login().await;
            }
            0x14 => {
                let _ = connection.socket.shutdown().await;
                let _ = connection.event_handler_in.send(Command::Logout {
                    sender_id: connection.id,
                });
                break;
            }
            0x0A => {
                connection.handle_enter_game_packet();
            }
            0x65 => {
                connection
                    .send_move_character(connection::Direction::North)
                    .await;
            }
            0x66 => {
                connection
                    .send_move_character(connection::Direction::East)
                    .await;
            }
            0x67 => {
                connection
                    .send_move_character(connection::Direction::South)
                    .await;
            }
            0x68 => {
                connection
                    .send_move_character(connection::Direction::West)
                    .await;
            }
            0x69 => {
                let _ = connection.socket.shutdown().await;
                let _ = connection.event_handler_in.send(Command::Logout {
                    sender_id: connection.id,
                });
                break;
            }
            _ => {
                println!("packet id not handled");
            }
        }
    }
}
