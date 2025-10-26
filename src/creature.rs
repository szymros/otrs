use crate::map::Item;

const KNOWN_CREATURE_ID: u16 = 0x62;
const UNKNOWN_CREATURE_ID: u16 = 0x61;

#[derive(Clone)]
pub struct Inventory {
    pub head: Option<Item>,
    pub neck: Option<Item>,
    pub backpack: Option<Item>,
    pub armor: Option<Item>,
    pub right: Option<Item>,
    pub left: Option<Item>,
    pub legs: Option<Item>,
    pub feet: Option<Item>,
    pub ring: Option<Item>,
    pub ammo: Option<Item>,
}

impl Inventory {
    pub fn new_empty() -> Inventory {
        return Inventory {
            head: None,
            neck: None,
            backpack: None,
            armor: None,
            right: None,
            left: None,
            legs: None,
            feet: None,
            ring: None,
            ammo: None,
        };
    }

    pub fn get_from_slot(self, slot: u16) -> Option<Item> {
        let item = match slot {
            1 => self.head,
            2 => self.neck,
            3 => self.backpack,
            4 => self.armor,
            5 => self.right,
            6 => self.left,
            7 => self.legs,
            8 => self.feet,
            9 => self.right,
            10 => self.ammo,
            _ => None,
        };
        return item;
    }

    pub fn remove_from_slot(&mut self, slot: u16) -> Option<Item> {
        let removed: Option<Item>;
        match slot {
            1 => {
                removed = self.head.clone();
                self.head = None;
            }
            2 => {
                removed = self.neck.clone();
                self.neck = None;
            }
            3 => {
                removed = self.backpack.clone();
                self.backpack = None;
            }
            4 => {
                removed = self.armor.clone();
                self.armor = None;
            }
            5 => {
                removed = self.right.clone();
                self.right = None;
            }
            6 => {
                removed = self.left.clone();
                self.left = None;
            }
            7 => {
                removed = self.legs.clone();
                self.legs = None;
            }
            8 => {
                removed = self.feet.clone();
                self.feet = None;
            }
            9 => {
                removed = self.right.clone();
                self.right = None;
            }
            10 => {
                removed = self.ammo.clone();
                self.ammo = None;
            }
            _ => removed = None,
        };
        return removed;
    }
    pub fn equip(&mut self, slot: u16, item: Item) {
        match slot {
            1 => {
                self.head = Some(item);
            }
            2 => {
                self.neck = Some(item);
            }
            3 => {
                self.backpack = Some(item);
            }
            4 => {
                self.armor = Some(item);
            }
            5 => {
                self.right = Some(item);
            }
            6 => {
                self.left = Some(item);
            }
            7 => {
                self.legs = Some(item);
            }
            8 => {
                self.feet = Some(item);
            }
            9 => {
                self.right = Some(item);
            }
            10 => {
                self.ammo = Some(item);
            }
            _ => (),
        };
    }
}

#[derive(Clone)]
pub struct Character {
    pub id: u32,
    pub name: String,
    pub world: String,
    pub outfit_type: u16,
    // head, body, legs, feet
    pub outfit: [u8; 4],
    pub position: (u16, u16, u8),
    pub speed: u16,
    pub look_dir: u8,
    pub health: u16,
    pub max_health: u16,
    pub inventory: Inventory,
}

impl Character {
    pub fn as_creature(&self) -> Creature {
        return Creature {
            id: self.id,
            name: self.name.clone(),
            outfit_type: self.outfit_type,
            outfit: self.outfit,
            is_known: true,
            health: self.health,
            max_health: self.health,
            look_dir: self.look_dir,
            light_level: 0x64,
            light_color: 0xD7,
            speed: self.speed,
            shield: 0,
        };
    }
}

#[derive(Clone)]
pub struct Creature {
    pub id: u32,
    pub name: String,
    pub outfit_type: u16,
    pub outfit: [u8; 4],
    pub is_known: bool,
    pub health: u16,
    pub max_health: u16,
    pub look_dir: u8,
    pub light_level: u8,
    pub light_color: u8,
    pub speed: u16,
    pub shield: u8,
}

pub fn str_fmt(s: &str) -> Vec<u8> {
    let mut bytes: Vec<u8> = Vec::new();
    bytes.extend_from_slice(&(s.len() as u16).to_le_bytes());
    for byte in s.as_bytes().iter() {
        bytes.push(*byte);
    }
    return bytes;
}

impl Creature {
    pub fn as_bytes(&self) -> Vec<u8> {
        let mut bytes: Vec<u8> = Vec::new();
        // todo handle known creature
        bytes.extend_from_slice(&UNKNOWN_CREATURE_ID.to_le_bytes());

        bytes.extend_from_slice(&((self.id + 10) as u32).to_le_bytes()); // idk
        bytes.extend_from_slice(&self.id.to_le_bytes());
        bytes.extend_from_slice(&str_fmt(&self.name));
        bytes.push(((self.health / self.max_health) * 100) as u8);
        bytes.push(self.look_dir);
        bytes.extend_from_slice(&self.outfit_type.to_le_bytes());
        bytes.extend_from_slice(&self.outfit);
        bytes.push(self.light_color);
        bytes.push(self.light_level);
        bytes.extend_from_slice(&self.speed.to_le_bytes());
        bytes.push(self.shield);
        return bytes;
    }
}

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
        inventory: Inventory::new_empty(),
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
        inventory: Inventory::new_empty(),
    });
    return characters;
}
