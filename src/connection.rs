use crate::{
    StaticData,
    creature::Character,
    event_handler::{Command, ServerEvent},
    item::Item,
    map::{Direction, Tile},
    payload::{
        MagicEffect, SpeechType, add_item_to_container_payload, add_item_to_inventory_payload,
        add_thing_payload, close_container_payload, container_payload, creature_added_payload,
        creature_turn_payload, enter_game_payload, login_payload, magic_effect_payload,
        map_direction_payload, remove_item_from_container_payload,
        remove_item_from_inventory_payload, remove_thing_payload, speech_payload,
        thing_moved_payload, thing_transformed_payload,
    },
};
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        mpsc::{Receiver, Sender, TryRecvError},
    },
    thread::sleep,
    time::Duration,
    vec,
};
use tokio::net::TcpStream;

pub struct State {
    pub map: HashMap<(u16, u16, u8), Tile>,
}

#[derive(Clone)]
pub struct Container {
    pub container_id: u8,
    pub parent_id: Option<u8>,
    pub client_id: u16,
    pub items: Vec<Item>,
    pub pos: (u16, u16, u8),
    pub stack_pos: u8,
    pub name: String,
    pub capacity: u8,
}

pub struct Connection {
    pub id: u32,
    pub read_buffer: Vec<u8>,
    pub read_idx: usize,
    pub write_buffer: Vec<u8>,
    pub write_idx: usize,
    pub socket: TcpStream,
    pub state: Arc<Mutex<State>>,
    pub event_handler_in: Sender<Command>,
    pub event_receiver: Receiver<ServerEvent>,
    pub character: Option<Character>,
    pub data: Arc<StaticData>,
    pub open_containers: HashMap<u8, Container>,
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
            write_idx: 0,
            read_idx: 0,
            state,
            event_handler_in,
            character: None,
            data,
            event_receiver,
            open_containers: HashMap::new(),
        };
    }

    pub async fn handle_events(&mut self) {
        let mut payload: Vec<u8> = Vec::new();
        loop {
            match self.event_receiver.try_recv() {
                Ok(event) => match event {
                    ServerEvent::CreatureAdded { pos, creature } => {
                        payload.extend_from_slice(&creature_added_payload(&pos, creature));
                        payload.extend_from_slice(&magic_effect_payload(
                            &pos,
                            MagicEffect::EnergyArea as u8,
                        ));
                    }
                    ServerEvent::CretureMoved {
                        from,
                        to,
                        stack_pos,
                        creature_id,
                        direction,
                    } => {
                        payload.extend_from_slice(&thing_transformed_payload(&from, stack_pos, None));
                        payload.extend_from_slice(&creature_turn_payload(direction.clone(), creature_id));
                        payload.extend_from_slice(&thing_moved_payload(&from, stack_pos, &to));
                        if creature_id == self.id {
                            self.character.as_mut().unwrap().position = to;
                            payload.extend_from_slice(&map_direction_payload(
                                self.state.clone(),
                                direction,
                                to,
                            ));
                        }
                    }
                    ServerEvent::EnterGame => {
                        payload.extend_from_slice(&enter_game_payload(
                            self.state.clone(),
                            &self.character.as_ref().unwrap().position,
                            self.id,
                        ));
                    }
                    ServerEvent::CreatureRemoved { pos, stack_pos } => {
                        payload.extend_from_slice(&remove_thing_payload(&pos, stack_pos));
                        payload.extend_from_slice(&magic_effect_payload(
                            &pos,
                            MagicEffect::Puff as u8,
                        ));
                    }
                    ServerEvent::ItemMoved {
                        from,
                        to,
                        stack_pos,
                        item_id,
                    } => {
                        let mut container_id: Option<u8> = None;
                        for (id, container) in self.open_containers.iter() {
                            if from == container.pos && container.stack_pos == stack_pos {
                                container_id = Some(*id);
                            }
                        }
                        if let Some(c_id) = container_id {
                            payload.extend_from_slice(&close_container_payload(c_id));
                            self.open_containers.remove(&c_id);
                        }

                        if from.0 < 0xFFFF {
                            payload.extend_from_slice(&remove_thing_payload(&from, stack_pos));
                        }
                        if to.0 < 0xFFFF {
                            payload.extend_from_slice(&add_thing_payload(&to, &item_id));
                        }
                    }
                    ServerEvent::OpenContainer {
                        index,
                        item,
                        name,
                        parent_id,
                        capacity,
                        pos,
                        stack_pos,
                    } => {
                        let mut has_parent = 0;
                        let mut container = Container {
                            container_id: index,
                            parent_id: None,
                            client_id: item.client_id,
                            items: item.items,
                            pos: pos,
                            stack_pos,
                            name,
                            capacity,
                        };
                        if let Some(p_id) = parent_id {
                            has_parent = 1;
                            if container.container_id == p_id {
                                container.parent_id = Some(0xFF - p_id);
                                let parent_container = self.open_containers.remove(&p_id).unwrap();
                                self.open_containers
                                    .insert(container.parent_id.unwrap(), parent_container);
                            } else {
                                container.parent_id = parent_id;
                            }
                        }
                        self.open_containers.insert(index, container.clone());
                        payload.extend_from_slice(&container_payload(
                            &container,
                            &container.name,
                            capacity,
                            has_parent,
                        ));
                    }
                    ServerEvent::AddedToContainer {
                        pos,
                        stack_pos,
                        item,
                    } => {
                        for (container_id, container) in self.open_containers.iter_mut() {
                            if pos == container.pos && container.stack_pos == stack_pos {
                                if pos.0 == 0xFFFF && pos.1 & 0x40 != 0x40 {
                                    let char = self.character.as_mut().unwrap();
                                    let inventory_item =
                                        char.inventory.clone().get_from_slot(pos.1);
                                    if let Some(mut it) = inventory_item {
                                        it.add_item(item.clone());
                                        char.inventory.equip(pos.1, it);
                                    }
                                }
                                let mut items = vec![item.clone()];
                                items.append(&mut container.items);
                                container.items = items;
                                payload.extend_from_slice(&add_item_to_container_payload(
                                    &item.client_id,
                                    *container_id,
                                ));
                            }
                        }
                    }
                    ServerEvent::RemovedFromContainer {
                        pos,
                        stack_pos,
                        slot,
                    } => {
                        for (container_id, container) in self.open_containers.iter_mut() {
                            if pos == container.pos && container.stack_pos == stack_pos {
                                if pos.0 == 0xFFFF && pos.1 & 0x40 != 0x40 {
                                    let char = self.character.as_mut().unwrap();
                                    let inventory_item =
                                        char.inventory.clone().get_from_slot(pos.1);
                                    if let Some(mut it) = inventory_item {
                                        it.items.remove(slot as usize);
                                        char.inventory.equip(pos.1, it);
                                    }
                                }
                                container.items.remove(slot as usize);
                                payload.extend_from_slice(&remove_item_from_container_payload(
                                    *container_id,
                                    slot,
                                ));
                            }
                        }
                    }
                    ServerEvent::ThingTransformed {
                        pos,
                        stack_pos,
                        to_item_id,
                    } => {
                        payload.extend_from_slice(&thing_transformed_payload(
                            &pos,
                            stack_pos,
                            Some(to_item_id),
                        ));
                    }
                    ServerEvent::CreatureSpoke {
                        pos,
                        text,
                        creature_name,
                        speech_type,
                    } => payload.extend_from_slice(&speech_payload(
                        &text,
                        &creature_name,
                        SpeechType::Say,
                        &pos,
                    )),
                    ServerEvent::CreatureTurned {
                        pos,
                        stack_pos,
                        direction,
                        creature_id,
                    } => {
                        payload
                            .extend_from_slice(&thing_transformed_payload(&pos, stack_pos, None));
                        payload.extend_from_slice(&creature_turn_payload(direction, creature_id));
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
        self.send_packet(&login_payload(&self.data.characters))
            .await;
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

    pub fn read_position(&mut self) -> (u16, u16, u8) {
        let x = self.read_u16_le();
        let y = self.read_u16_le();
        let z = self.read_u8();
        return (x, y, z);
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
                    pos: character.position,
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

    pub fn handle_move_character_packets(&mut self, direction: Direction) {
        // let sleep_time = GROUND_SPEED * 1000 / self.character.as_ref().unwrap().speed as u32;
        let sleep_time = 58;
        sleep(Duration::from_millis(sleep_time as u64));
        let from = self.character.as_ref().unwrap().position;
        let to = direction.move_in_dir(from);
        let _ = self.event_handler_in.send(Command::MoveCreature {
            from,
            to,
            direction: direction.clone(),
            creature_id: self.id,
        });
    }

    pub fn handle_creature_turn_packets(&mut self, direction: Direction) {
        let character = self.character.as_ref().unwrap();
        let pos = character.position;
        let creature_id = self.id;
        let _ = self.event_handler_in.send(Command::TurnCreature {
            pos,
            creature_id,
            direction,
        });
    }

    pub async fn handle_move_item(&mut self) {
        // TODO: handle drag and drop onto container
        let from = self.read_position();
        let item_id = self.read_u16_le();
        let stack_pos = self.read_u8();
        let to = self.read_position();
        let count = self.read_u8();
        let mut payload: Vec<u8> = Vec::new();
        let mut item: Option<Item> = None;
        let character = self.character.as_mut().unwrap();
        let mut commands: Vec<Command> = Vec::new();
        if from.0 == 0xFFFF {
            // from container
            if from.1 & 0x40 == 0x40 {
                let from_container_id = (from.1 & 0x0F) as u8;
                let container = self.open_containers.get(&from_container_id).unwrap();
                item = container.items.get(from.2 as usize).cloned();
                commands.push(Command::RemoveItemFromContainer {
                    pos: container.pos,
                    stack_pos: container.stack_pos,
                    slot: from.2,
                    sender_id: self.id,
                });
            } else {
                // from inventory
                item = character.inventory.remove_from_slot(from.1);
                payload.extend_from_slice(&remove_item_from_inventory_payload(from.1 as u8));
            }
            if let Some(it) = item {
                if to.0 == 0xFFFF {
                    // to container
                    if to.1 & 0x40 == 0x40 {
                        // TODO: consider handling the addition here
                        let to_container_id = (to.1 & 0x0F) as u8;
                        let container = self.open_containers.get_mut(&to_container_id).unwrap();
                        commands.push(Command::AddToContainer {
                            item: it.clone(),
                            sender_id: self.id,
                            slot: to.2,
                            container: container.clone(),
                        });
                    } else {
                        // to inventory
                        character.inventory.equip(to.1, it.clone());
                        payload.extend_from_slice(&add_item_to_inventory_payload(
                            it.client_id,
                            to.1 as u8,
                        ));
                    }
                } else {
                    // to ground
                    commands.push(Command::MoveItem {
                        from,
                        to,
                        stack_pos,
                        count,
                        item: it,
                    });
                }
            }
        }
        // from ground
        else {
            let state_handle = self.state.lock().unwrap();
            let tile = state_handle.map.get(&from).unwrap();
            item = tile.get_item_at_stack_pos(stack_pos);
            if let Some(it) = item {
                commands.push(Command::MoveItem {
                    from,
                    to,
                    stack_pos,
                    item: it.clone(),
                    count,
                });
                if to.0 == 0xFFFF {
                    if to.1 & 0x40 == 0x40 {
                        // TODO: consider handling the addition here
                        let to_container_id = (to.1 & 0x0F) as u8;
                        let container = self.open_containers.get(&to_container_id).unwrap();
                        commands.push(Command::AddToContainer {
                            item: it,
                            sender_id: self.id,
                            slot: to.2,
                            container: container.clone(),
                        });
                    } else {
                        self.character
                            .as_mut()
                            .unwrap()
                            .inventory
                            .equip(to.1, it.clone());
                        payload
                            .extend_from_slice(&add_item_to_inventory_payload(item_id, to.1 as u8));
                    }
                }
            }
        }
        for command in commands.iter() {
            let _ = self.event_handler_in.send(command.clone());
        }
        if payload.len() > 0 {
            self.send_packet(&payload).await;
        }
    }

    pub fn handle_use_item(&mut self) {
        let from = self.read_position();
        let item_id = self.read_u16_le();
        let stack_pos = self.read_u8();
        let index = self.read_u8();
        let item: Item;
        if from.0 == 0xFFFF {
            if from.1 & 0x40 == 0x40 {
                let container_id = (from.1 & 0x0F) as u8;
                let container = &self.open_containers.get(&container_id).unwrap();
                item = container.items[from.2 as usize].clone();
            } else {
                if let Some(it) = self
                    .character
                    .as_ref()
                    .unwrap()
                    .inventory
                    .clone()
                    .get_from_slot(from.1)
                {
                    item = it;
                } else {
                    item = Item {
                        client_id: item_id,
                        items: vec![],
                    };
                }
            }
        } else {
            item = Item {
                client_id: item_id,
                items: vec![],
            };
        }
        let _ = self.event_handler_in.send(Command::UseItem {
            sender_id: self.id,
            pos: from,
            stack_pos,
            item,
            index,
        });
    }

    pub async fn handle_close_container(&mut self) {
        let mut payload: Vec<u8> = Vec::new();
        let container_id = self.read_u8();
        self.open_containers.remove(&container_id);
        payload.push(0x6F);
        payload.push(container_id);
        self.send_packet(&payload).await;
    }

    pub async fn handle_container_up(&mut self) {
        let mut payload: Vec<u8> = Vec::new();
        let container_id = self.read_u8();
        let container = self.open_containers.remove(&container_id).unwrap();
        let parent_container = self
            .open_containers
            .remove(&(container.parent_id.unwrap() as u8))
            .unwrap();
        self.open_containers.insert(
            0xFF - &(container.parent_id.unwrap()),
            parent_container.clone(),
        );
        payload.extend_from_slice(&container_payload(
            &parent_container,
            &parent_container.name.clone(),
            parent_container.capacity,
            0,
        ));
        self.send_packet(&payload).await;
    }
    pub fn handle_say_packet(&mut self) {
        let speech_type = self.read_u8();
        let speech_text = self.read_str();
        let char_pos = self.character.as_ref().unwrap().position;
        let char_name = self.character.as_ref().unwrap().name.clone();
        let _ = self.event_handler_in.send(Command::CreatureSpeech {
            pos: char_pos,
            text: speech_text.to_string(),
            creature_name: char_name,
            speech_type,
        });
    }

    pub fn handle_use_item_on_target_packet(&mut self) {
        let pos = self.read_position();
        let item_id = self.read_u16_le();
        let stack_pos = self.read_u8();
        let target_pos = self.read_u16_le();
        let target_stack_pos = self.read_u8();
    }

    pub async fn handle_ping(&mut self) {
        self.send_packet(&vec![0x1E]).await;
    }
}
