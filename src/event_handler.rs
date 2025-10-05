use crate::creature::Creature;
use crate::connection::State;
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        mpsc::{ Receiver, Sender},
    },
};

pub enum Command {
    PutCreature {
        cords: (u16, u16, u8),
        creature: Creature,
    },
    AddNewConnection {
        tx: Sender<ServerEvent>,
        connection_id: u32,
    },
    CloseConnection {
        connection_id: u32,
    },
    MoveCreature {
        sender_id: u32,
        from: (u16, u16, u8),
        to: (u16, u16, u8),
        creature_id: u32,
    },
    EnterGame {
        character_creature: Creature,
        cords: (u16, u16, u8),
    },
    Logout {
        sender_id: u32,
    },
}

#[derive(Clone)]
pub enum ServerEvent {
    CreatureAdded {
        cords: (u16, u16, u8),
        creature: Creature,
    },
    CretureMoved {
        from: (u16, u16, u8),
        to: (u16, u16, u8),
        stack_pos: u8,
    },
    CreatureRemoved {
        cords: (u16, u16, u8),
        stack_pos: u8,
    },
    EnterGame,
}

pub async fn event_handler(event_rx: Receiver<Command>, state: Arc<Mutex<State>>) {
    let mut connections: HashMap<u32, ((u16, u16, u8), Sender<ServerEvent>)> = HashMap::new();
    loop {
        match event_rx.recv() {
            Ok(command) => match command {
                Command::PutCreature { cords, creature } => {
                    put_creature(state.clone(), &mut connections, cords, creature.clone());
                    let event = ServerEvent::CreatureAdded {
                        cords,
                        creature: creature.clone(),
                    };
                    push_events(connections.clone(), cords, None, event);
                }
                Command::AddNewConnection { tx, connection_id } => {
                    connections.insert(connection_id, ((0, 0, 0), tx));
                }
                Command::CloseConnection { connection_id } => (),
                Command::MoveCreature {
                    sender_id,
                    from,
                    to,
                    creature_id,
                } => {
                    let server_event = handle_move_creature(state.clone(), from, to, creature_id);
                    if let Some(event) = server_event {
                        push_events(connections.clone(), to, Some(sender_id), event);
                    }
                }
                Command::EnterGame {
                    character_creature,
                    cords,
                } => {
                    put_creature(
                        state.clone(),
                        &mut connections,
                        cords,
                        character_creature.clone(),
                    );
                    let creature_event = ServerEvent::CreatureAdded {
                        cords,
                        creature: character_creature.clone(),
                    };
                    push_events(
                        connections.clone(),
                        cords,
                        Some(character_creature.id),
                        creature_event,
                    );
                    let enter_game_event = ServerEvent::EnterGame;
                    let (_, tx) = connections.get(&character_creature.id).unwrap();
                    let _ = tx.send(enter_game_event);
                }
                Command::Logout { sender_id } => {
                    let (char_pos, _) = connections.remove(&sender_id).unwrap();
                    let _ = remove_creature_from_tile(state.clone(), char_pos, sender_id);
                    let event = ServerEvent::CreatureRemoved {
                        cords: char_pos,
                        stack_pos: 1,
                    };
                    push_events(connections.clone(), char_pos, None, event);
                }
            },
            Err(_) => break,
        }
    }
}

fn push_events(
    connections: HashMap<u32, ((u16, u16, u8), Sender<ServerEvent>)>,
    event_pos: (u16, u16, u8),
    origin_id: Option<u32>,
    event: ServerEvent,
) {
    for (id, ((connection_x, connection_y, _), sender)) in connections.iter() {
        let (e_x, e_y, _) = event_pos;
        if e_x.abs_diff(*connection_x) < 9 && e_y.abs_diff(*connection_y) < 9 {
            if let Some(client_id) = origin_id {
                if *id == client_id {
                    continue;
                }
            }
            let _ = sender.send(event.clone());
        }
    }
}

fn put_creature(
    state: Arc<Mutex<State>>,
    connections: &mut HashMap<u32, ((u16, u16, u8), Sender<ServerEvent>)>,
    cords: (u16, u16, u8),
    creature: Creature,
) {
    if connections.contains_key(&creature.id) {
        connections.entry(creature.id).and_modify(|((x, y, z), _)| {
            *x = cords.0;
            *y = cords.1;
            *z = cords.2;
        });
    }
    {
        let mut state_hanlde = state.lock().unwrap();
        state_hanlde
            .map
            .entry(cords)
            .and_modify(|tile| tile.creatures.push(creature));
    }
}

fn remove_creature_from_tile(
    state: Arc<Mutex<State>>,
    cords: (u16, u16, u8),
    creature_id: u32,
) -> Option<Creature> {
    let mut removed_creature: Option<Creature> = None;
    let mut state_handle = state.lock().unwrap();
    state_handle.map.entry(cords).and_modify(|tile| {
        let mut creature_idx: Option<usize> = None;
        for (idx, creature) in tile.creatures.iter().enumerate() {
            if creature.id == creature_id {
                removed_creature = Some(creature.clone());
                creature_idx = Some(idx);
            }
        }
        if let Some(idx) = creature_idx {
            tile.creatures.remove(idx);
        }
    });
    return removed_creature;
}

fn handle_move_creature(
    state: Arc<Mutex<State>>,
    from: (u16, u16, u8),
    to: (u16, u16, u8),
    creature_id: u32,
) -> Option<ServerEvent> {
    let creature_to_move: Option<Creature> =
        remove_creature_from_tile(state.clone(), from, creature_id);
    let mut state_handle = state.lock().unwrap();
    if let Some(creature) = creature_to_move {
        state_handle
            .map
            .entry(to)
            .and_modify(|tile| tile.creatures.push(creature));
        let stack_pos: u8 = state_handle.map.get(&to).unwrap().creatures.len() as u8;
        let event = ServerEvent::CretureMoved {
            from,
            to,
            stack_pos,
        };
        return Some(event);
    }
    return None;
}
