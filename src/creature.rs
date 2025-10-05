const KNOWN_CREATURE_ID: u16 = 0x62;
const UNKNOWN_CREATURE_ID: u16 = 0x61;

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
        
        bytes.extend_from_slice(&((self.id +10) as u32).to_le_bytes()); // idk
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
