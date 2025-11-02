use crate::{
    StaticData,
    connection::{Container, State},
    creature::Creature,
    item::Item,
    item::get_item_action,
    map::Direction,
    otb_io::item_loader::ItemType,
};
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
        pos: (u16, u16, u8),
        creature: Creature,
    },
    AddNewConnection {
        tx: Sender<ServerEvent>,
        connection_id: u32,
    },
    MoveCreature {
        from: (u16, u16, u8),
        to: (u16, u16, u8),
        direction: Direction,
        creature_id: u32,
    },
    TurnCreature {
        pos: (u16, u16, u8),
        creature_id: u32,
        direction: Direction,
    },
    EnterGame {
        character_creature: Creature,
        pos: (u16, u16, u8),
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
    UseItem {
        sender_id: u32,
        pos: (u16, u16, u8),
        stack_pos: u8,
        item: Item,
        index: u8,
    },
    UseItemOnTarget {
        pos: (u16, u16, u8),
        stack_pos: u8,
        item: Item,
        target_pos: (u16, u16, u8),
        target_stack_pos: u8,
    },
    AddToContainer {
        item: Item,
        sender_id: u32,
        slot: u8,
        container: Container,
    },
    RemoveItemFromContainer {
        pos: (u16, u16, u8),
        stack_pos: u8,
        slot: u8,
        sender_id: u32,
    },
    TransformThing {
        pos: (u16, u16, u8),
        stack_pos: u8,
        from_item_id: u16,
        to_item_id: u16,
    },
    CreatureSpeech {
        pos: (u16, u16, u8),
        text: String,
        creature_name: String,
        speech_type: u8,
    },
}

#[derive(Clone)]
pub enum ServerEvent {
    CreatureAdded {
        pos: (u16, u16, u8),
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
        pos: (u16, u16, u8),
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
        pos: (u16, u16, u8),
        stack_pos: u8,
        index: u8,
        item: Item,
        name: String,
        parent_id: Option<u8>,
        capacity: u8,
    },
    AddedToContainer {
        pos: (u16, u16, u8),
        stack_pos: u8,
        item: Item,
    },
    RemovedFromContainer {
        pos: (u16, u16, u8),
        stack_pos: u8,
        slot: u8,
    },
    ThingTransformed {
        pos: (u16, u16, u8),
        stack_pos: u8,
        to_item_id: u16,
    },
    CreatureSpoke {
        pos: (u16, u16, u8),
        text: String,
        creature_name: String,
        speech_type: u8,
    },
    CreatureTurned {
        pos: (u16, u16, u8),
        stack_pos: u8,
        direction: Direction,
        creature_id: u32,
    },
}

pub async fn event_handler(
    event_rx: Receiver<Command>,
    loopback_tx: Sender<Command>,
    state: Arc<Mutex<State>>,
    data: Arc<StaticData>,
) {
    let mut connections: HashMap<u32, ((u16, u16, u8), Sender<ServerEvent>)> = HashMap::new();
    loop {
        match event_rx.recv() {
            Ok(command) => match command {
                Command::PutCreature { pos, creature } => {
                    put_creature(state.clone(), &mut connections, pos, creature.clone());
                    let event = ServerEvent::CreatureAdded {
                        pos,
                        creature: creature.clone(),
                    };
                    broadcast_event(&connections, pos, None, event);
                }
                Command::AddNewConnection { tx, connection_id } => {
                    connections.insert(connection_id, ((0, 0, 0), tx));
                }
                Command::MoveCreature {
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
                        broadcast_event(&connections, to, None, event);
                    }
                }
                Command::EnterGame {
                    character_creature,
                    pos,
                } => {
                    put_creature(
                        state.clone(),
                        &mut connections,
                        pos,
                        character_creature.clone(),
                    );
                    let creature_event = ServerEvent::CreatureAdded {
                        pos,
                        creature: character_creature.clone(),
                    };
                    broadcast_event(
                        &connections,
                        pos,
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
                        pos: char_pos,
                        stack_pos: 1,
                    };
                    broadcast_event(&connections, char_pos, None, event);
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
                        broadcast_event(&connections, location, None, event);
                    };
                }
                Command::UseItem {
                    sender_id,
                    pos,
                    stack_pos,
                    item,
                    index,
                } => {
                    let state_handle = state.lock().unwrap();
                    let mut item = Some(item);
                    if pos.0 != 0xFFFF {
                        let tile = state_handle.map.get(&pos).unwrap();
                        if let Some(it) = tile.get_item_at_stack_pos(stack_pos) {
                            item = Some(it.clone());
                        }
                    }
                    if let Some(it) = item {
                        let item_data = data.item_data.get(&it.client_id).unwrap();
                        if let ItemType::Container = item_data.item_type {
                            let (_, tx) = connections.get(&sender_id).unwrap();
                            let parent_id: Option<u8> = if pos.0 == 0xFFFF && pos.1 & 0x40 == 0x40 {
                                Some((pos.1 & 0x0F) as u8)
                            } else {
                                None
                            };
                            let _ = tx.send(ServerEvent::OpenContainer {
                                pos,
                                stack_pos,
                                index,
                                item: it.clone(),
                                name: item_data.item_name.clone(),
                                parent_id,
                                capacity: 20,
                            });
                        } else {
                            let item_action = get_item_action(&it.client_id);
                            let commands = item_action(pos, stack_pos, it.client_id);
                            for command in commands.iter() {
                                let _ = loopback_tx.send(command.clone());
                            }
                        }
                    }
                }
                Command::AddToContainer {
                    sender_id,
                    item,
                    slot,
                    container,
                } => {
                    // TODO: handle drag and drop onto container
                    let mut state_handle = state.lock().unwrap();
                    // let mut is_target_container: bool = false;
                    // let target_item = container.items.get(slot as usize);
                    // if let Some(it) = target_item {
                    //     let target_item_data = data.item_data.get(&it.client_id);
                    //     if let ItemType::Container = target_item_data.unwrap().item_type {
                    //         is_target_container = true;
                    //     }
                    // }
                    let event = ServerEvent::AddedToContainer {
                        pos: container.pos,
                        stack_pos: container.stack_pos,
                        item: item.clone(),
                    };

                    if container.pos.0 == 0xFFFF {
                        let (_, tx) = connections.get(&sender_id).unwrap();
                        let _ = tx.send(event);
                    } else {
                        state_handle.map.entry(container.pos).and_modify(|tile| {
                            // if is_target_container {
                            // tile.bot_items[container.stack_pos as usize - 1].items
                            //     [slot as usize]
                            //     .add_item(item.clone());
                            // } else {
                            tile.bot_items[container.stack_pos as usize - 1].add_item(item.clone());
                            broadcast_event(&connections, container.pos, None, event);
                            // }
                        });
                    }
                }
                Command::RemoveItemFromContainer {
                    pos,
                    stack_pos,
                    slot,
                    sender_id,
                } => {
                    let state_clone = state.clone();
                    let mut state_handle = state_clone.lock().unwrap();
                    let event = ServerEvent::RemovedFromContainer {
                        pos,
                        stack_pos,
                        slot,
                    };
                    if pos.0 == 0xFFFF {
                        let (_, tx) = connections.get(&sender_id).unwrap();
                        let _ = tx.send(event);
                    } else {
                        state_handle.map.entry(pos).and_modify(|tile| {
                            tile.bot_items[stack_pos as usize - 1]
                                .items
                                .remove(slot as usize);
                        });
                        broadcast_event(&connections, pos, None, event);
                    }
                }
                Command::TransformThing {
                    pos,
                    stack_pos,
                    from_item_id,
                    to_item_id,
                } => {
                    let mut state_handle = state.lock().unwrap();
                    if pos.0 != 0xFFFF {
                        let tile = state_handle.map.get_mut(&pos).unwrap();
                        if let Some(item) = tile.get_item_at_stack_pos(stack_pos) {
                            if item.client_id == from_item_id {
                                tile.change_at_stack_pos(stack_pos, to_item_id);
                                let event = ServerEvent::ThingTransformed {
                                    pos,
                                    stack_pos,
                                    to_item_id,
                                };
                                broadcast_event(&connections, pos, None, event);
                            }
                        };
                    }
                }
                Command::CreatureSpeech {
                    pos,
                    text,
                    creature_name,
                    speech_type,
                } => {
                    let event = ServerEvent::CreatureSpoke {
                        pos,
                        text,
                        creature_name,
                        speech_type,
                    };
                    broadcast_event(&connections, pos, None, event);
                }
                Command::UseItemOnTarget {
                    pos,
                    stack_pos,
                    item,
                    target_pos,
                    target_stack_pos,
                } => {}
                Command::TurnCreature {
                    pos,
                    creature_id,
                    direction,
                } => {
                    if let Some(event) =
                        handle_turn_creature(state.clone(), pos, creature_id, direction)
                    {
                        broadcast_event(&connections, pos, None, event);
                    }
                }
            },
            Err(_) => break,
        }
    }
}

fn broadcast_event(
    connections: &HashMap<u32, ((u16, u16, u8), Sender<ServerEvent>)>,
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
    pos: (u16, u16, u8),
    creature: Creature,
) {
    if connections.contains_key(&creature.id) {
        connections.entry(creature.id).and_modify(|((x, y, z), _)| {
            *x = pos.0;
            *y = pos.1;
            *z = pos.2;
        });
    }
    {
        let mut state_hanlde = state.lock().unwrap();
        state_hanlde
            .map
            .entry(pos)
            .and_modify(|tile| tile.creatures.push(creature));
    }
}

fn remove_creature_from_tile(
    state: Arc<Mutex<State>>,
    pos: (u16, u16, u8),
    creature_id: u32,
) -> Option<(Creature, u8)> {
    let mut removed_creature: Option<(Creature, u8)> = None;
    let mut stack_pos: u8 = 0;
    let mut state_handle = state.lock().unwrap();
    state_handle.map.entry(pos).and_modify(|tile| {
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
            connections.entry(creature_id).and_modify(|(pos, _)| {
                *pos = to;
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

fn handle_turn_creature(
    state: Arc<Mutex<State>>,
    pos: (u16, u16, u8),
    creature_id: u32,
    direction: Direction,
) -> Option<ServerEvent> {
    let mut state_handle = state.lock().unwrap();
    let mut event: Option<ServerEvent> = None;
    state_handle.map.entry(pos).and_modify(|tile| {
        for (idx, creature) in tile.creatures.iter_mut().enumerate() {
            if creature.id == creature_id {
                creature.look_dir = direction.clone();
                let stack_pos = (idx + tile.top_items.len()) as u8 + 1;
                event = Some(ServerEvent::CreatureTurned {
                    pos,
                    stack_pos,
                    direction: direction.clone(),
                    creature_id,
                });
            }
        }
    });
    return event;
}
