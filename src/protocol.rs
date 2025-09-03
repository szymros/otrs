const WORLD_SERVER_IP_BYTES: [u8; 4] = [127, 0, 0, 1];
const WORLD_SERVER_PORT_BYTES: [u8; 2] = [71, 71];

enum ProtocolLoginPacketType {
    Motd = 0x14,
    CharList = 0x64,
}

enum ProtocolGamePacketType{
    WorldInit = 0xA,
    MapData = 0x64
}

enum InPacketType {
    InitWorld = 0xA,
    Login = 0x1,
}

fn protocol_str_fmt(s: &str) -> Vec<u8> {
    let mut bytes: Vec<u8> = Vec::new();
    bytes.extend_from_slice(&(s.len() as u16).to_le_bytes());
    for byte in s.as_bytes().iter() {
        bytes.push(*byte);
    }
    return bytes;
}

fn motd_payload(text: &str) -> Vec<u8> {
    let mut payload: Vec<u8> = Vec::new();
    payload.push(ProtocolLoginPacketType::Motd as u8); // packet id motd
    payload.extend_from_slice(&protocol_str_fmt(text));
    return payload;
}


fn characters_payload() -> Vec<u8> {
    let mut payload: Vec<u8> = Vec::new();
    payload.push(ProtocolLoginPacketType::CharList as u8); //packet id
    payload.push(0x01); // num of chars
    payload.extend_from_slice(&protocol_str_fmt("test")); // char name
    payload.extend_from_slice(&protocol_str_fmt("test")); // world name
    payload.extend_from_slice(&WORLD_SERVER_IP_BYTES); // world ip
    payload.extend_from_slice(&WORLD_SERVER_PORT_BYTES);
    payload.extend_from_slice(&0x0001u16.to_le_bytes()); // premium days
    return payload;
}

fn world_init_payload() -> Vec<u8> {
    let mut payload: Vec<u8> = Vec::new();
    payload.push(ProtocolGamePacketType::WorldInit as u8);
    payload.extend_from_slice(&[0, 0, 0, 1, 32, 0, 0]); // u32 player id + magic u16 related to client drawing speed should be 0x32 + byte for flag + other flags
    payload.push(ProtocolGamePacketType::MapData as u8); // map data start
    payload.extend_from_slice(&123u16.to_le_bytes()); // player x
    payload.extend_from_slice(&123u16.to_le_bytes());// player y
    payload.push(6); // player z
    for i in 0..(18 * 14) {
        payload.extend_from_slice(&3410u16.to_le_bytes());
        payload.push(0x00); // end tile
        payload.push(0xFF); // end tile
    }
    payload.push(0xFF); // terminator for map data
    return payload;
}

