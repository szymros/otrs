use crate::{event_handler::Command, otb_io::map_loader::OtbMapItem};
use std::collections::HashMap;

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

pub fn get_item_action(item_id: &u16) -> impl Fn((u16, u16, u8), u8, u16) -> Vec<Command> {
    match item_id {
        1644 => {
            let func = |pos: (u16, u16, u8), stack_pos: u8, item_id: u16| {
                vec![Command::TransformThing {
                    pos: pos,
                    stack_pos,
                    from_item_id: 1644,
                    to_item_id: 1645,
                }]
            };
            func
        } // closed door
        1645 => {
            let func = |pos: (u16, u16, u8), stack_pos: u8, item_id: u16| {
                vec![Command::TransformThing {
                    pos: pos,
                    stack_pos,
                    from_item_id: 1645,
                    to_item_id: 1644,
                }]
            };
            func
        } // opened door
        2772 => {
            let func = |pos: (u16, u16, u8), stack_pos: u8, item_id: u16| {
                vec![Command::TransformThing {
                    pos: pos,
                    stack_pos,
                    from_item_id: 2772,
                    to_item_id: 2773,
                }]
            };
            func
        } // switch left
        2773 => {
            let func = |pos: (u16, u16, u8), stack_pos: u8, item_id: u16| {
                vec![Command::TransformThing {
                    pos: pos,
                    stack_pos,
                    from_item_id: 2773,
                    to_item_id: 2772,
                }]
            };
            func
        } // switch right
        _ => {
            let f = |_, _, _| vec![];
            f
        }
    }
}
