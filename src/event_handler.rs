use crate::connection::Container;
use crate::creature::Creature;
use crate::map::{Direction, Item};
use crate::otb_io::item_loader::{self, ItemData, ItemType};
use crate::{StaticData, connection::State};
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        mpsc::{Receiver, Sender},
    },
};

#[derive(Clone)]
pub enum Command {
    PutCreature {
        cords: (u16, u16, u8),
        creature: Creature,
    },
    AddNewConnection {
        tx: Sender<ServerEvent>,
        connection_id: u32,
    },
    MoveCreature {
        sender_id: u32,
        from: (u16, u16, u8),
        to: (u16, u16, u8),
        direction: Direction,
        creature_id: u32,
    },
    EnterGame {
        character_creature: Creature,
        cords: (u16, u16, u8),
    },
    Logout {
        sender_id: u32,
    },
    MoveItem {
        from: (u16, u16, u8),
        to: (u16, u16, u8),
        stack_pos: u8,
        item: Item,
        count: u8,
    },
    RemoveItem {
        cords: (u16, u16, u8),
        stack_pos: u8,
    },
    AddItem {
        cords: (u16, u16, u8),
        stack_pos: u8,
        item_id: u16,
    },
    UseItem {
        sender_id: u32,
        cords: (u16, u16, u8),
        stack_pos: u8,
        item: Item,
        index: u8,
    },
    AddToContainer {
        cords: (u16, u16, u8),
        stack_pos: u8,
        item: Item,
        sender_id: u32,
        slot: u8,
        container: Container,
    },
    RemoveItemFromContainer {
        cords: (u16, u16, u8),
        stack_pos: u8,
        slot: u8,
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
        creature_id: u32,
        direction: Direction,
    },
    CreatureRemoved {
        cords: (u16, u16, u8),
        stack_pos: u8,
    },
    ItemMoved {
        from: (u16, u16, u8),
        to: (u16, u16, u8),
        stack_pos: u8,
        item_id: u16,
    },
    EnterGame,
    OpenContainer {
        cords: (u16, u16, u8),
        stack_pos: u8,
        index: u8,
        item: Item,
        name: String,
        parent_id: Option<u16>,
        capacity: u8,
    },
    AddedToContainer {
        cords: (u16, u16, u8),
        stack_pos: u8,
        item: Item,
        slot: u8,
        is_target_container: bool,
    },
    RemovedFromContainer {
        cords: (u16, u16, u8),
        stack_pos: u8,
        slot: u8,
    },
}

pub async fn event_handler(
    event_rx: Receiver<Command>,
    state: Arc<Mutex<State>>,
    data: Arc<StaticData>,
) {
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
                    broadcast_event(connections.clone(), cords, None, event);
                }
                Command::AddNewConnection { tx, connection_id } => {
                    connections.insert(connection_id, ((0, 0, 0), tx));
                }
                Command::MoveCreature {
                    sender_id,
                    from,
                    to,
                    creature_id,
                    direction,
                } => {
                    let server_event = handle_move_creature(
                        state.clone(),
                        &mut connections,
                        from,
                        to,
                        creature_id,
                        direction,
                    );
                    if let Some(event) = server_event {
                        broadcast_event(connections.clone(), to, None, event);
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
                    broadcast_event(
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
                    broadcast_event(connections.clone(), char_pos, None, event);
                }
                Command::MoveItem {
                    from,
                    to,
                    stack_pos,
                    item,
                    count,
                } => {
                    if let Some(event) =
                        handle_move_item(state.clone(), from, to, stack_pos, count, item)
                    {
                        let location = if from.0 == 0xFFFF { to } else { from };
                        broadcast_event(connections.clone(), location, None, event);
                    };
                }
                Command::UseItem {
                    sender_id,
                    cords,
                    stack_pos,
                    item,
                    index,
                } => {
                    let state_c = state.clone();
                    let state_handle = state_c.lock().unwrap();
                    let mut item = Some(item);
                    if cords.0 != 0xFFFF {
                        let tile = state_handle.map.get(&cords).unwrap();
                        // TODO: use actual stackpos
                        if let Some(it) = tile.bot_items.first() {
                            item = Some(it.clone());
                        }
                    }
                    if let Some(it) = item {
                        let item_data = data.item_data.get(&it.client_id).unwrap();
                        match item_data.item_type {
                            ItemType::Container => {
                                let (_, tx) = connections.get(&sender_id).unwrap();
                                let parent_id: Option<u16> =
                                    if cords.0 == 0xFFFF && cords.1 & 0x40 == 0x40 {
                                        Some(cords.1 & 0x0F)
                                    } else {
                                        None
                                    };
                                let _ = tx.send(ServerEvent::OpenContainer {
                                    cords,
                                    stack_pos,
                                    index,
                                    item: it.clone(),
                                    name: item_data.item_name.clone(),
                                    parent_id,
                                    capacity: 20,
                                });
                            }
                            _ => (),
                        };
                    }
                }
                Command::RemoveItem { cords, stack_pos } => todo!(),
                Command::AddItem {
                    cords,
                    stack_pos,
                    item_id,
                } => todo!(),
                Command::AddToContainer {
                    sender_id,
                    cords,
                    stack_pos,
                    item,
                    slot,
                    container,
                } => {
                    let state_clone = state.clone();
                    let mut state_handle = state_clone.lock().unwrap();
                    let mut is_target_container: bool = false;
                    let target_item = container.items.get(slot as usize);
                    if let Some(it) = target_item {
                        let target_item_data = data.item_data.get(&it.client_id);
                        if let ItemType::Container = target_item_data.unwrap().item_type {
                            is_target_container = true;
                        }
                    }
                    let event = ServerEvent::AddedToContainer {
                        cords,
                        stack_pos,
                        item: item.clone(),
                        slot,
                        is_target_container,
                    };

                    if cords.0 == 0xFFFF {
                            let (_, tx) = connections.get(&sender_id).unwrap();
                            let _ = tx.send(event);
                    } else {
                        state_handle.map.entry(cords).and_modify(|tile| {
                            if is_target_container {
                                tile.bot_items[stack_pos as usize - 1].items[slot as usize]
                                    .add_item(item.clone());
                            } else {
                                tile.bot_items[stack_pos as usize - 1].add_item(item.clone());
                                broadcast_event(connections.clone(), cords, None, event);
                            }
                        });
                    }
                }
                Command::RemoveItemFromContainer {
                    cords,
                    stack_pos,
                    slot,
                    sender_id,
                } => {
                    let state_clone = state.clone();
                    let mut state_handle = state_clone.lock().unwrap();
                    let event = ServerEvent::RemovedFromContainer {
                        cords,
                        stack_pos,
                        slot,
                    };
                    if cords.0 == 0xFFFF {
                        let (_, tx) = connections.get(&sender_id).unwrap();
                        let _ = tx.send(event);
                    } else {
                        state_handle.map.entry(cords).and_modify(|tile| {
                            tile.bot_items[stack_pos as usize - 1]
                                .items
                                .remove(slot as usize);
                        });
                        broadcast_event(connections.clone(), cords, None, event);
                    }
                }
            },
            Err(_) => break,
        }
    }
}

fn broadcast_event(
    connections: HashMap<u32, ((u16, u16, u8), Sender<ServerEvent>)>,
    event_pos: (u16, u16, u8),
    sender_id: Option<u32>,
    event: ServerEvent,
) {
    for (id, ((connection_x, connection_y, _), sender)) in connections.iter() {
        let (e_x, e_y, _) = event_pos;
        if e_x.abs_diff(*connection_x) < 9 && e_y.abs_diff(*connection_y) < 9 {
            if let Some(client_id) = sender_id {
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
) -> Option<(Creature, u8)> {
    let mut removed_creature: Option<(Creature, u8)> = None;
    let mut stack_pos: u8 = 0;
    let mut state_handle = state.lock().unwrap();
    state_handle.map.entry(cords).and_modify(|tile| {
        let mut creature_idx: Option<usize> = None;
        for (idx, creature) in tile.creatures.iter().enumerate() {
            if creature.id == creature_id {
                stack_pos = tile.top_items.len() as u8 + idx as u8 + 1;
                removed_creature = Some((creature.clone(), stack_pos));
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
    connections: &mut HashMap<u32, ((u16, u16, u8), Sender<ServerEvent>)>,
    from: (u16, u16, u8),
    to: (u16, u16, u8),
    creature_id: u32,
    direction: Direction,
) -> Option<ServerEvent> {
    let creature_to_move: Option<(Creature, u8)> =
        remove_creature_from_tile(state.clone(), from, creature_id);
    let mut state_handle = state.lock().unwrap();
    if let Some((creature, stack_pos)) = creature_to_move {
        state_handle
            .map
            .entry(to)
            .and_modify(|tile| tile.creatures.push(creature));
        let event = ServerEvent::CretureMoved {
            from,
            to,
            stack_pos,
            creature_id,
            direction,
        };
        if connections.contains_key(&creature_id) {
            connections.entry(creature_id).and_modify(|(cords, _)| {
                *cords = to;
            });
        }
        return Some(event);
    }
    return None;
}

fn handle_move_item(
    state: Arc<Mutex<State>>,
    from: (u16, u16, u8),
    to: (u16, u16, u8),
    stack_pos: u8,
    _count: u8,
    item: Item,
) -> Option<ServerEvent> {
    let mut state_handle = state.lock().unwrap();
    let mut item: Option<Item> = Some(item);
    if from.0 != 0xFFFF {
        state_handle
            .map
            .entry(from)
            .and_modify(|tile| item = tile.bot_items.pop());
    }
    if let Some(it) = item {
        if to.0 != 0xFFFF {
            state_handle
                .map
                .entry(to)
                .and_modify(|tile| tile.bot_items.push(it.clone()));
        }
        return Some(ServerEvent::ItemMoved {
            from,
            to,
            stack_pos,
            item_id: it.client_id,
        });
    }
    return None;
}
