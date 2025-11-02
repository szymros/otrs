mod connection;
mod creature;
mod event_handler;
mod map;
mod otb_io;
mod payload;
mod item;
use std::{
    collections::HashMap,
    io::ErrorKind,
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver, Sender},
    },
};

use crate::{
    connection::{Connection, State},
    creature::{Character, create_characters},
    event_handler::{Command, ServerEvent, event_handler},
    map::{Direction, create_tile_map},
    otb_io::item_loader::ItemData,
};
use tokio::{io::AsyncWriteExt, net::TcpListener};

struct StaticData {
    item_data: HashMap<u16, ItemData>,
    characters: Vec<Character>,
}

#[tokio::main]
async fn main() {
    let mut connection_counter = 0;
    let (item_data, server_id_to_client_id) =
        otb_io::item_loader::read_otb_items("./data/items.otb");
    let data = Arc::new(StaticData {
        item_data,
        characters: create_characters(),
    });
    let map_data = otb_io::map_loader::read_file("./data/testmap.otbm");
    let map = create_tile_map(&map_data, &data.item_data, &server_id_to_client_id);
    let state = Arc::new(Mutex::new(State { map }));

    let listener = TcpListener::bind("127.0.0.1:7171").await.unwrap();
    let (event_handler_in, event_handler_rx): (Sender<Command>, Receiver<Command>) =
        mpsc::channel();

    let state_clone = state.clone();
    let data_clone = data.clone();
    let event_handler_in_clone = event_handler_in.clone();
    tokio::spawn(async move {
        event_handler(event_handler_rx, event_handler_in_clone,state_clone, data_clone).await;
    });

    loop {
        connection_counter += 1;
        let (mut socket, _) = listener.accept().await.unwrap();
        println!("new connection accepted");
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
                let _ = connection.event_handler_in.send(Command::Logout {
                    sender_id: connection.id,
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
            0x1E => {
                connection.handle_ping().await;
            }
            0x65 => {
                connection
                    .handle_move_character_packets(Direction::North);
            }
            0x66 => {
                connection
                    .handle_move_character_packets(Direction::East);
            }
            0x67 => {
                connection
                    .handle_move_character_packets(Direction::South);
            }
            0x68 => {
                connection
                    .handle_move_character_packets(Direction::West);
            }
            0x69 => {
                let _ = connection.socket.shutdown().await;
                let _ = connection.event_handler_in.send(Command::Logout {
                    sender_id: connection.id,
                });
                break;
            }
            0x6F =>{
                connection.handle_creature_turn_packets(Direction::North);
            }
            0x70 => {
                connection.handle_creature_turn_packets(Direction::East);
            }
            0x71 => {
                connection.handle_creature_turn_packets(Direction::South);
            }
            0x72 => {
                connection.handle_creature_turn_packets(Direction::West);
            }
            0x78 => {
                connection.handle_move_item().await;
            }
            0x82 => {
                connection.handle_use_item();
            }
            0x87 => {
                connection.handle_close_container().await;
            }
            0x88 => {
                connection.handle_container_up().await;
            }
            0x96 =>{
                connection.handle_say_packet();
            }
            _ => {
                println!("packet id not handled");
            }
        }
    }
}
