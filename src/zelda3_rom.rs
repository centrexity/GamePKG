//! Native RAM-only resource packing adapted from snesrev/zelda3 (MIT).
use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
const ROM_SHA256: &str = "66871d66be19ad2c34c927d6b14cd8eb6fc3181965b6e517cb361f7316009cfb";
const NAMES: [&str; 165] = [
    "kSoundBank_intro",
    "kSoundBank_indoor",
    "kSoundBank_ending",
    "kDungeonRoom",
    "kDungeonRoomOffs",
    "kDungeonRoomDoorOffs",
    "kDungeonRoomHeaders",
    "kDungeonRoomHeadersOffs",
    "kDungeonRoomChests",
    "kDungeonRoomTeleMsg",
    "kDungeonPitsHurtPlayer",
    "kEntranceData_rooms",
    "kEntranceData_relativeCoords",
    "kEntranceData_scrollX",
    "kEntranceData_scrollY",
    "kEntranceData_playerX",
    "kEntranceData_playerY",
    "kEntranceData_cameraX",
    "kEntranceData_cameraY",
    "kEntranceData_blockset",
    "kEntranceData_floor",
    "kEntranceData_palace",
    "kEntranceData_doorwayOrientation",
    "kEntranceData_startingBg",
    "kEntranceData_quadrant1",
    "kEntranceData_quadrant2",
    "kEntranceData_doorSettings",
    "kEntranceData_musicTrack",
    "kStartingPoint_rooms",
    "kStartingPoint_relativeCoords",
    "kStartingPoint_scrollX",
    "kStartingPoint_scrollY",
    "kStartingPoint_playerX",
    "kStartingPoint_playerY",
    "kStartingPoint_cameraX",
    "kStartingPoint_cameraY",
    "kStartingPoint_blockset",
    "kStartingPoint_floor",
    "kStartingPoint_palace",
    "kStartingPoint_doorwayOrientation",
    "kStartingPoint_startingBg",
    "kStartingPoint_quadrant1",
    "kStartingPoint_quadrant2",
    "kStartingPoint_doorSettings",
    "kStartingPoint_entrance",
    "kStartingPoint_musicTrack",
    "kDungeonRoomDefault",
    "kDungeonRoomDefaultOffs",
    "kDungeonRoomOverlay",
    "kDungeonRoomOverlayOffs",
    "kDungeonSecrets",
    "kDungAttrsForTile_Offs",
    "kDungAttrsForTile",
    "kMovableBlockDataInit",
    "kTorchDataInit",
    "kTorchDataJunk",
    "kEnemyDamageData",
    "kLinkGraphics",
    "kDungeonSprites",
    "kDungeonSpriteOffs",
    "kMap32ToMap16_0",
    "kMap32ToMap16_1",
    "kMap32ToMap16_2",
    "kMap32ToMap16_3",
    "kSprGfx",
    "kBgGfx",
    "kOverworldMapGfx",
    "kLightOverworldTilemap",
    "kDarkOverworldTilemap",
    "kPredefinedTileData",
    "kMap16ToMap8",
    "kGeneratedWishPondItem",
    "kGeneratedBombosArr",
    "kGeneratedEndSequence15",
    "kEnding_Credits_Text",
    "kEnding_Credits_Offs",
    "kEnding_MapData",
    "kEnding0_Offs",
    "kEnding0_Data",
    "kPalette_DungBgMain",
    "kPalette_MainSpr",
    "kPalette_ArmorAndGloves",
    "kPalette_Sword",
    "kPalette_Shield",
    "kPalette_SpriteAux3",
    "kPalette_MiscSprite_Indoors",
    "kPalette_SpriteAux1",
    "kPalette_OverworldBgMain",
    "kPalette_OverworldBgAux12",
    "kPalette_OverworldBgAux3",
    "kPalette_PalaceMapBg",
    "kPalette_PalaceMapSpr",
    "kHudPalData",
    "kOverworldMapPaletteData",
    "kDialogue",
    "kDialogueFont",
    "kDialogueMap",
    "kDungMap_FloorLayout",
    "kDungMap_Tiles",
    "kBgTilemap_0",
    "kBgTilemap_1",
    "kBgTilemap_2",
    "kBgTilemap_3",
    "kBgTilemap_4",
    "kBgTilemap_5",
    "kOverworld_Hibytes_Comp",
    "kOverworld_Lobytes_Comp",
    "kOverworldMapIsSmall",
    "kOverworldAuxTileThemeIndexes",
    "kOverworldBgPalettes",
    "kOverworld_SignText",
    "kOwMusicSets",
    "kOwMusicSets2",
    "kBirdTravel_ScreenIndex",
    "kBirdTravel_Map16LoadSrcOff",
    "kBirdTravel_ScrollX",
    "kBirdTravel_ScrollY",
    "kBirdTravel_LinkXCoord",
    "kBirdTravel_LinkYCoord",
    "kBirdTravel_CameraXScroll",
    "kBirdTravel_CameraYScroll",
    "kBirdTravel_Unk1",
    "kBirdTravel_Unk3",
    "kWhirlpoolAreas",
    "kOverworld_Entrance_Area",
    "kOverworld_Entrance_Pos",
    "kOverworld_Entrance_Id",
    "kFallHole_Area",
    "kFallHole_Pos",
    "kFallHole_Entrances",
    "kExitData_ScreenIndex",
    "kExitDataRooms",
    "kExitData_Map16LoadSrcOff",
    "kExitData_ScrollX",
    "kExitData_ScrollY",
    "kExitData_XCoord",
    "kExitData_YCoord",
    "kExitData_CameraXScroll",
    "kExitData_CameraYScroll",
    "kExitData_NormalDoor",
    "kExitData_FancyDoor",
    "kExitData_Unk1",
    "kExitData_Unk3",
    "kSpExit_Top",
    "kSpExit_Bottom",
    "kSpExit_Left",
    "kSpExit_Right",
    "kSpExit_Tab4",
    "kSpExit_Tab5",
    "kSpExit_Tab6",
    "kSpExit_Tab7",
    "kSpExit_LeftEdgeOfMap",
    "kSpExit_Dir",
    "kSpExit_SprGfx",
    "kSpExit_AuxGfx",
    "kSpExit_PalBg",
    "kSpExit_PalSpr",
    "kOverworldSecrets_Offs",
    "kOverworldSecrets",
    "kOverworldSpriteOffs",
    "kOverworldSprites",
    "kOverworldSpriteGfx",
    "kOverworldSpritePalettes",
    "kMap8DataToTileAttr",
    "kSomeTileAttr",
];
const SPRITES: [usize; 108] = [
    1110016, 1111552, 1113088, 1147392, 1148928, 1150464, 1152000, 1153536, 1155072, 1156608,
    1158144, 1159680, 1376252, 1410516, 1411766, 1413054, 1414136, 1415590, 1417010, 1418199,
    1419505, 1420998, 1422360, 1423687, 1425104, 1426505, 1427829, 1429116, 1430420, 1431724,
    1433024, 1434444, 1435880, 1437233, 1438630, 1440045, 1441466, 1475327, 1476832, 1478209,
    1479391, 1480835, 1481936, 1483374, 1483381, 1484679, 1485318, 1486493, 1487871, 1489022,
    1490539, 1491261, 1492505, 1493947, 1495281, 1496641, 1497434, 1498521, 1499768, 1501024,
    1502501, 1503759, 1504951, 1505887, 1506601, 1540557, 1541773, 1543010, 1544149, 1545511,
    1546571, 1547943, 1549070, 1550341, 1551608, 1553058, 1554425, 1555603, 1557047, 1558414,
    1559893, 1561276, 1562671, 1563884, 1565132, 1565547, 1566786, 1567544, 1568088, 1568876,
    1570045, 1571385, 1572742, 1606748, 1608097, 1609060, 1610141, 1611280, 1611863, 1612580,
    1613266, 1613887, 1615085, 1615802, 1617631, 1617677, 1619232, 1620307,
];
const BACKGROUNDS: [usize; 115] = [
    1161216, 1162466, 1163615, 1164917, 1166212, 1167180, 1168078, 1169190, 1169871, 1171140,
    1172371, 1173651, 1174909, 1176195, 1177414, 1178657, 1179634, 1213592, 1214990, 1216304,
    1217318, 1218564, 1219931, 1221234, 1222398, 1223287, 1224067, 1225063, 1226013, 1226816,
    1228116, 1229257, 1230653, 1232006, 1233304, 1234609, 1235818, 1237226, 1238717, 1239889,
    1241397, 1242821, 1244273, 1278249, 1279635, 1280941, 1282327, 1283593, 1284897, 1286260,
    1287705, 1289003, 1290252, 1291509, 1292779, 1294015, 1295310, 1296407, 1297256, 1298357,
    1299552, 1300674, 1301882, 1303142, 1304495, 1305829, 1307205, 1308400, 1309744, 1343721,
    1345083, 1346172, 1347370, 1348422, 1349613, 1350850, 1352051, 1353245, 1354589, 1355907,
    1356989, 1358158, 1359374, 1360529, 1361850, 1363172, 1364443, 1365693, 1366903, 1367761,
    1368748, 1369940, 1371054, 1371982, 1372937, 1373940, 1374805, 1376140, 1376147, 1376154,
    1376161, 1376168, 1376175, 1376182, 1376189, 1376196, 1376203, 1376210, 1376217, 1376224,
    1376231, 1376238, 1376245, 1619232, 1620307,
];
const ALPHABET: [&str; 95] = [
    "A",
    "B",
    "C",
    "D",
    "E",
    "F",
    "G",
    "H",
    "I",
    "J",
    "K",
    "L",
    "M",
    "N",
    "O",
    "P",
    "Q",
    "R",
    "S",
    "T",
    "U",
    "V",
    "W",
    "X",
    "Y",
    "Z",
    "a",
    "b",
    "c",
    "d",
    "e",
    "f",
    "g",
    "h",
    "i",
    "j",
    "k",
    "l",
    "m",
    "n",
    "o",
    "p",
    "q",
    "r",
    "s",
    "t",
    "u",
    "v",
    "w",
    "x",
    "y",
    "z",
    "0",
    "1",
    "2",
    "3",
    "4",
    "5",
    "6",
    "7",
    "8",
    "9",
    "!",
    "?",
    "-",
    ".",
    ",",
    "[...]",
    ">",
    "(",
    ")",
    "[Ankh]",
    "[Waves]",
    "[Snake]",
    "[LinkL]",
    "[LinkR]",
    "\"",
    "[Up]",
    "[Down]",
    "[Left]",
    "[Right]",
    "'",
    "[1HeartL]",
    "[1HeartR]",
    "[2HeartL]",
    "[3HeartL]",
    "[3HeartR]",
    "[4HeartL]",
    "[4HeartR]",
    " ",
    "<",
    "[A]",
    "[B]",
    "[X]",
    "[Y]",
];
const DICTIONARY: [&str; 97] = [
    "    ", "   ", "  ", "'s ", "and ", "are ", "all ", "ain", "and", "at ", "ast", "an", "at",
    "ble", "ba", "be", "bo", "can ", "che", "com", "ck", "des", "di", "do", "en ", "er ", "ear",
    "ent", "ed ", "en", "er", "ev", "for", "fro", "give ", "get", "go", "have", "has", "her", "hi",
    "ha", "ight ", "ing ", "in", "is", "it", "just", "know", "ly ", "la", "lo", "man", "ma", "me",
    "mu", "n't ", "non", "not", "open", "ound", "out ", "of", "on", "or", "per", "ple", "pow",
    "pro", "re ", "re", "some", "se", "sh", "so", "st", "ter ", "thin", "ter", "tha", "the", "thi",
    "to", "tr", "up", "ver", "with", "wa", "we", "wh", "wi", "you", "Her", "Tha", "The", "Thi",
    "You",
];
const COMMAND_LENGTHS: [usize; 25] = [
    1, 1, 1, 1, 2, 2, 2, 2, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 1, 1, 1, 1, 1,
];
const COMMAND_NAMES: [&str; 24] = [
    "NextPic",
    "Choose",
    "Item",
    "Name",
    "Window",
    "Number",
    "Position",
    "ScrollSpd",
    "Selchg",
    "Unused_Crash",
    "Choose3",
    "Choose2",
    "Scroll",
    "1",
    "2",
    "3",
    "Color",
    "Wait",
    "Sound",
    "Speed",
    "Unused_Mark",
    "Unused_Mark2",
    "Unused_Clear",
    "Waitkey",
];
struct Rom<'a>(&'a [u8]);
impl Rom<'_> {
    fn offset(a: usize) -> Result<usize> {
        if a & 0x8000 == 0 {
            bail!("invalid Zelda3 ROM bank address");
        }
        Ok(((a >> 16) & 0x7f) * 0x8000 + (a & 0x7fff))
    }
    fn b(&self, a: usize) -> Result<u8> {
        self.0
            .get(Self::offset(a)?)
            .copied()
            .context("Zelda3 ROM read out of bounds")
    }
    fn w(&self, a: usize) -> Result<usize> {
        Ok(self.b(a)? as usize | (self.b(a + 1)? as usize) << 8)
    }
    fn p(&self, a: usize) -> Result<usize> {
        Ok(self.w(a)? | (self.b(a + 2)? as usize) << 16)
    }
    fn bytes(&self, a: usize, n: usize) -> Result<Vec<u8>> {
        let p = Self::offset(a)?;
        self.0
            .get(p..p.checked_add(n).context("ROM size overflow")?)
            .map(|x| x.to_vec())
            .context("Zelda3 ROM range out of bounds")
    }
    fn compressed(&self, a: usize, be: bool) -> Result<(Vec<u8>, Vec<u8>)> {
        let start = Self::offset(a)?;
        let mut pos = start;
        let mut result = Vec::new();
        let next = |p: &mut usize| -> Result<u8> {
            let b = *self.0.get(*p).context("truncated ROM compression")?;
            *p += 1;
            Ok(b)
        };
        loop {
            let b = next(&mut pos)?;
            if b == 255 {
                break;
            }
            let (cmd, n) = if b & 0xe0 != 0xe0 {
                (b & 0xe0, (b & 31) as usize + 1)
            } else {
                (
                    (b << 3) & 0xe0,
                    (((b & 3) as usize) << 8 | next(&mut pos)? as usize) + 1,
                )
            };
            if result.len() + n > 65536 {
                bail!("Zelda3 compressed resource too large");
            }
            match cmd {
                0 => {
                    for _ in 0..n {
                        result.push(next(&mut pos)?);
                    }
                }
                0x20 => {
                    let b = next(&mut pos)?;
                    result.resize(result.len() + n, b);
                }
                0x40 => {
                    let a = next(&mut pos)?;
                    let b = next(&mut pos)?;
                    for i in 0..n {
                        result.push(if i & 1 == 0 { a } else { b });
                    }
                }
                0x60 => {
                    let b = next(&mut pos)?;
                    for i in 0..n {
                        result.push(b.wrapping_add(i as u8));
                    }
                }
                _ => {
                    let a = next(&mut pos)?;
                    let b = next(&mut pos)?;
                    let offset = if be {
                        u16::from_be_bytes([a, b])
                    } else {
                        u16::from_le_bytes([a, b])
                    } as usize;
                    for i in 0..n {
                        result.push(
                            *result
                                .get(offset + i)
                                .context("invalid ROM compression backreference")?,
                        );
                    }
                }
            }
        }
        Ok((result, self.0[start..pos].to_vec()))
    }
}
fn words(values: impl IntoIterator<Item = usize>) -> Vec<u8> {
    values
        .into_iter()
        .flat_map(|v| (v as u16).to_le_bytes())
        .collect()
}
fn pack(parts: &[Vec<u8>]) -> Vec<u8> {
    if parts.is_empty() {
        return Vec::new();
    }
    let mut offsets = Vec::new();
    let mut offset = 0;
    for part in &parts[..parts.len() - 1] {
        offset += part.len();
        offsets.push(offset);
    }
    let wide = offset >= 65536 || parts.len() > 8192;
    let mut result = Vec::new();
    for offset in offsets {
        if wide {
            result.extend_from_slice(&(offset as u32).to_le_bytes());
        } else {
            result.extend_from_slice(&(offset as u16).to_le_bytes());
        }
    }
    for part in parts {
        result.extend_from_slice(part);
    }
    result
        .extend_from_slice(&((parts.len() - 1 + if wide { 8192 } else { 0 }) as u16).to_le_bytes());
    result
}
type Assets = BTreeMap<&'static str, Vec<u8>>;
fn checked_rom(bytes: &[u8]) -> Result<Rom<'_>> {
    let bytes = if bytes.len() == 1049088 {
        &bytes[512..]
    } else {
        bytes
    };
    if bytes.len() != 1048576 || hex::encode(Sha256::digest(bytes)) != ROM_SHA256 {
        bail!(
            "Zelda3 requires the supported USA 1 MiB ROM, optionally with a 512-byte copier header"
        );
    }
    Ok(Rom(bytes))
}
fn assets(rom: &Rom<'_>) -> Result<Assets> {
    let mut out = Assets::new();
    misc(rom, &mut out)?;
    graphics(rom, &mut out)?;
    entrances(rom, &mut out)?;
    overworld(rom, &mut out)?;
    dungeon(rom, &mut out)?;
    dialogue(rom, &mut out)?;
    music(rom, &mut out)?;
    Ok(out)
}
fn misc(rom: &Rom<'_>, out: &mut Assets) -> Result<()> {
    out.insert("kOverworldMapGfx", rom.bytes(0x18c000, 16384)?);
    out.insert("kLightOverworldTilemap", rom.bytes(0xac727, 4096)?);
    out.insert("kDarkOverworldTilemap", rom.bytes(0xad727, 1024)?);
    out.insert("kPredefinedTileData", rom.bytes(0x9b52, 12876)?);
    out.insert("kMap16ToMap8", rom.bytes(0x8f8000, 30016)?);
    out.insert("kGeneratedWishPondItem", rom.bytes(0x888450, 256)?);
    out.insert("kGeneratedBombosArr", rom.bytes(0x8890fc, 256)?);
    out.insert("kGeneratedEndSequence15", rom.bytes(0x8ead25, 256)?);
    out.insert("kEnding_Credits_Text", rom.bytes(0x8eb178, 1989)?);
    out.insert("kEnding_Credits_Offs", rom.bytes(0x8eb93d, 788)?);
    out.insert("kEnding_MapData", rom.bytes(0x8eb038, 320)?);
    out.insert("kEnding0_Offs", rom.bytes(0x8ec2e1, 34)?);
    out.insert("kEnding0_Data", rom.bytes(0x8ebf4c, 917)?);
    out.insert("kPalette_DungBgMain", rom.bytes(0x9bd734, 3600)?);
    out.insert("kPalette_MainSpr", rom.bytes(0x9bd218, 240)?);
    out.insert("kPalette_ArmorAndGloves", rom.bytes(0x9bd308, 150)?);
    out.insert("kPalette_Sword", rom.bytes(0x9bd630, 24)?);
    out.insert("kPalette_Shield", rom.bytes(0x9bd648, 24)?);
    out.insert("kPalette_SpriteAux3", rom.bytes(0x9bd39e, 168)?);
    out.insert("kPalette_MiscSprite_Indoors", rom.bytes(0x9bd446, 154)?);
    out.insert("kPalette_SpriteAux1", rom.bytes(0x9bd4e0, 336)?);
    out.insert("kPalette_OverworldBgMain", rom.bytes(0x9be6c8, 420)?);
    out.insert("kPalette_OverworldBgAux12", rom.bytes(0x9be86c, 840)?);
    out.insert("kPalette_OverworldBgAux3", rom.bytes(0x9be604, 196)?);
    out.insert("kPalette_PalaceMapBg", rom.bytes(0x9be544, 192)?);
    out.insert("kPalette_PalaceMapSpr", rom.bytes(0x9bd70a, 42)?);
    out.insert("kHudPalData", rom.bytes(0x9bd660, 128)?);
    out.insert("kOverworldMapPaletteData", rom.bytes(0x8adb27, 512)?);
    out.insert("kLinkGraphics", rom.bytes(0x108000, 56 * 16 * 32)?);
    out.insert("kMap8DataToTileAttr", rom.bytes(0x8e9459, 512)?);
    out.insert("kSomeTileAttr", rom.bytes(0x9bf110, 3824)?);
    out.insert("kDungAttrsForTile_Offs", rom.bytes(0x8e9000, 42)?);
    out.insert("kDungAttrsForTile", rom.bytes(0x8e902a, 1024)?);
    out.insert("kMovableBlockDataInit", rom.bytes(0x84f1de, 396)?);
    out.insert("kTorchDataInit", rom.bytes(0x84f36a, 288)?);
    out.insert("kTorchDataJunk", rom.bytes(0x84f48a, 96)?);
    out.insert("kEnemyDamageData", rom.compressed(0x83e800, true)?.0);
    for (name, address) in [
        ("kMap32ToMap16_0", 0x838000),
        ("kMap32ToMap16_1", 0x83b400),
        ("kMap32ToMap16_2", 0x848000),
        ("kMap32ToMap16_3", 0x84b400),
    ] {
        out.insert(name, rom.bytes(address, 2218 * 6)?);
    }
    Ok(())
}
fn graphics(rom: &Rom<'_>, out: &mut Assets) -> Result<()> {
    let sprites = SPRITES
        .iter()
        .enumerate()
        .map(|(i, &a)| {
            if i < 12 {
                rom.bytes(a, 0x600)
            } else {
                rom.compressed(a, false).map(|x| x.1)
            }
        })
        .collect::<Result<Vec<_>>>()?;
    out.insert("kSprGfx", pack(&sprites));
    let backgrounds = BACKGROUNDS
        .iter()
        .map(|&a| rom.compressed(a, false).map(|x| x.1))
        .collect::<Result<Vec<_>>>()?;
    out.insert("kBgGfx", pack(&backgrounds));
    out.insert(
        "kDialogueFont",
        pack(&[pack(&[
            rom.bytes(0x8e8000, 4096)?,
            rom.bytes(0x8ecadf, 99)?,
        ])]),
    );
    out.insert(
        "kDialogueMap",
        pack(&[pack(&[b"us".to_vec(), vec![0, 0, 0]])]),
    );
    let mut layouts = Vec::new();
    let mut tiles = Vec::new();
    for (i, n) in [75, 125, 50, 75, 175, 75, 50, 75, 50, 200, 150, 75, 100, 200]
        .into_iter()
        .enumerate()
    {
        let layout = rom.bytes(0xa0000 + rom.w(0x8af605 + i * 2)?, n)?;
        tiles.push(rom.bytes(
            0xa0000 + rom.w(0x8afbe4 + i * 2)?,
            layout.iter().filter(|&&b| b != 15).count(),
        )?);
        layouts.push(layout);
    }
    out.insert("kDungMap_FloorLayout", pack(&layouts));
    out.insert("kDungMap_Tiles", pack(&tiles));
    for (name, address) in [
        ("kBgTilemap_0", 0xcdd6d),
        ("kBgTilemap_1", 0xce7bf),
        ("kBgTilemap_2", 0xce2a8),
        ("kBgTilemap_3", 0xce63c),
        ("kBgTilemap_4", 0xce456),
        ("kBgTilemap_5", 0xeda9c),
    ] {
        let mut p = address;
        while rom.b(p)? & 128 == 0 {
            let b = rom.b(p + 2)?;
            let n = (((b as usize) << 8 | rom.b(p + 3)? as usize) & 0x3fff) + 1;
            p += 4 + if b & 64 != 0 { 2 } else { n };
        }
        out.insert(name, rom.bytes(address, p - address + 1)?);
    }
    for (name, table) in [
        ("kOverworld_Hibytes_Comp", 0x82f94d),
        ("kOverworld_Lobytes_Comp", 0x82fb2d),
    ] {
        let parts = (0..160)
            .map(|i| rom.compressed(rom.p(table + i * 3)?, true).map(|x| x.1))
            .collect::<Result<Vec<_>>>()?;
        out.insert(name, pack(&parts));
    }
    Ok(())
}

fn entrances(rom: &Rom<'_>, out: &mut Assets) -> Result<()> {
    for (prefix, count, set) in [("kEntranceData_", 133, 0), ("kStartingPoint_", 7, 1)] {
        for (suffix, addresses, width) in [
            ("rooms", [0x82c813, 0x82db6e], 2),
            ("relativeCoords", [0x82c91d, 0x82db7c], 8),
            ("scrollX", [0x82cd45, 0x82dbb4], 2),
            ("scrollY", [0x82ce4f, 0x82dbc2], 2),
            ("playerX", [0x82d063, 0x82dbde], 2),
            ("playerY", [0x82cf59, 0x82dbd0], 2),
            ("cameraX", [0x82d277, 0x82dbfa], 2),
            ("cameraY", [0x82d16d, 0x82dbec], 2),
            ("blockset", [0x82d381, 0x82dc08], 1),
            ("floor", [0x82d406, 0x82dc0f], 1),
            ("palace", [0x82d48b, 0x82dc16], 1),
            ("startingBg", [0x82d595, 0x82dc1d], 1),
            ("quadrant1", [0x82d61a, 0x82dc24], 1),
            ("quadrant2", [0x82d69f, 0x82dc2b], 1),
            ("doorSettings", [0x82d724, 0x82dc32], 2),
            ("musicTrack", [0x82d82e, 0x82dc4e], 1),
        ] {
            let name = NAMES
                .iter()
                .copied()
                .find(|name| *name == format!("{prefix}{suffix}"))
                .context("unknown engine entrance field")?;
            let mut data = rom.bytes(addresses[set], count * width)?;
            if suffix == "quadrant1" {
                for b in &mut data {
                    *b &= 0x22;
                }
            }
            if suffix == "doorSettings" {
                for b in data.as_chunks_mut::<2>().0 {
                    let value = u16::from_le_bytes(*b);
                    *b = if value == 65535 {
                        value
                    } else {
                        value & 0x9ffe
                    }
                    .to_le_bytes();
                }
            }
            out.insert(name, data);
        }
        let name = if set == 0 {
            "kEntranceData_doorwayOrientation"
        } else {
            "kStartingPoint_doorwayOrientation"
        };
        out.insert(
            name,
            if set == 0 {
                rom.bytes(0x82d510, count)?
            } else {
                vec![0; count]
            },
        );
        if set == 1 {
            out.insert(
                "kStartingPoint_entrance",
                (0..7)
                    .map(|i| rom.w(0x82dc40 + i * 2).map(|v| v as u8))
                    .collect::<Result<Vec<_>>>()?,
            );
        }
    }
    Ok(())
}
fn overworld(rom: &Rom<'_>, out: &mut Assets) -> Result<()> {
    let heads = (0..160)
        .filter(|&i| {
            i >= 128
                || rom
                    .b(0x82a5ec + (i & 63))
                    .is_ok_and(|x| x as usize == i & 63)
        })
        .collect::<Vec<_>>();
    let small = rom.bytes(0x82f88d, 192)?;
    out.insert(
        "kOverworldMapIsSmall",
        (0..192)
            .map(|i| if heads.contains(&i) { small[i] } else { 0 })
            .collect(),
    );
    for (name, address, count, width) in [
        ("kOverworldAuxTileThemeIndexes", 0x80fc9c, 128, 1),
        ("kOverworldBgPalettes", 0x80fd1c, 136, 1),
        ("kOverworld_SignText", 0x87f51d, 128, 2),
        ("kOwMusicSets", 0x82c303, 256, 1),
        ("kOwMusicSets2", 0x82c403, 96, 1),
        ("kOverworldSpriteGfx", 0x80fa41, 256, 1),
        ("kOverworldSpritePalettes", 0x80fb41, 256, 1),
    ] {
        let mut data = vec![0; count * width];
        for i in 0..count {
            let area = match name {
                "kOwMusicSets" | "kOverworldSpriteGfx" | "kOverworldSpritePalettes" => i & 63,
                "kOwMusicSets2" => i + 64,
                _ => i,
            };
            if !heads.contains(&area) {
                continue;
            }
            let value = rom.bytes(address + i * width, width)?;
            let targets = if area < 128 && small[area] == 0 {
                vec![i, i + 1, i + 8, i + 9]
            } else {
                vec![i]
            };
            for j in targets {
                if j < count {
                    data[j * width..(j + 1) * width].copy_from_slice(&value);
                }
            }
        }
        out.insert(name, data);
    }
    for (name, address, count, width, stride) in [
        ("kBirdTravel_ScreenIndex", 0x82eae5, 17, 2, 2),
        ("kBirdTravel_Map16LoadSrcOff", 0x82eb07, 17, 2, 2),
        ("kBirdTravel_ScrollY", 0x82eb29, 17, 2, 2),
        ("kBirdTravel_ScrollX", 0x82eb4b, 17, 2, 2),
        ("kBirdTravel_LinkYCoord", 0x82eb6d, 17, 2, 2),
        ("kBirdTravel_LinkXCoord", 0x82eb8f, 17, 2, 2),
        ("kBirdTravel_CameraYScroll", 0x82ebb1, 17, 2, 2),
        ("kBirdTravel_CameraXScroll", 0x82ebd3, 17, 2, 2),
        ("kBirdTravel_Unk1", 0x82ebf5, 17, 1, 2),
        ("kBirdTravel_Unk3", 0x82ec17, 17, 1, 2),
        ("kWhirlpoolAreas", 0x82ecf8, 8, 2, 2),
        ("kOverworld_Entrance_Area", 0x9bb96f, 129, 2, 2),
        ("kOverworld_Entrance_Pos", 0x9bba71, 129, 2, 2),
        ("kOverworld_Entrance_Id", 0x9bbb73, 129, 1, 1),
        ("kExitDataRooms", 0x82dd8a, 79, 2, 2),
        ("kExitData_ScreenIndex", 0x82de28, 79, 1, 1),
        ("kExitData_Map16LoadSrcOff", 0x82de77, 79, 2, 2),
        ("kExitData_ScrollY", 0x82df15, 79, 2, 2),
        ("kExitData_ScrollX", 0x82dfb3, 79, 2, 2),
        ("kExitData_YCoord", 0x82e051, 79, 2, 2),
        ("kExitData_XCoord", 0x82e0ef, 79, 2, 2),
        ("kExitData_CameraYScroll", 0x82e18d, 79, 2, 2),
        ("kExitData_CameraXScroll", 0x82e22b, 79, 2, 2),
        ("kExitData_Unk1", 0x82e2c9, 79, 1, 1),
        ("kExitData_Unk3", 0x82e318, 79, 1, 1),
        ("kExitData_NormalDoor", 0x82e367, 79, 2, 2),
        ("kExitData_FancyDoor", 0x82e405, 79, 2, 2),
    ] {
        let mut data = Vec::new();
        for i in 0..count {
            data.extend(rom.bytes(address + i * stride, width)?);
        }
        out.insert(name, data);
    }
    let mut travel_order = (9..17)
        .map(|i| Ok((rom.w(0x82eae5 + i * 2)?, i)))
        .collect::<Result<Vec<_>>>()?;
    travel_order.sort();
    let order = (0..9)
        .chain(travel_order.iter().map(|(_, i)| *i))
        .collect::<Vec<_>>();
    for name in NAMES
        .iter()
        .copied()
        .filter(|n| n.starts_with("kBirdTravel_"))
    {
        let data = out.get(name).context("missing travel field")?;
        let width = data.len() / 17;
        let mut reordered = Vec::new();
        for &i in &order {
            reordered.extend_from_slice(&data[i * width..(i + 1) * width]);
        }
        out.insert(name, reordered);
    }
    out.insert(
        "kWhirlpoolAreas",
        words(
            travel_order
                .iter()
                .map(|(_, i)| rom.w(0x82ecf8 + (i - 9) * 2))
                .collect::<Result<Vec<_>>>()?,
        ),
    );
    let mut holes = (0..19)
        .map(|i| {
            Ok((
                rom.b(0x9bb84c + i)?,
                rom.w(0x9bb800 + i * 2)?,
                rom.w(0x9bb826 + i * 2)?,
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    holes.sort();
    out.insert("kFallHole_Entrances", holes.iter().map(|h| h.0).collect());
    out.insert("kFallHole_Pos", words(holes.iter().map(|h| h.1 & 0x1ffe)));
    out.insert("kFallHole_Area", words(holes.iter().map(|h| h.2)));
    let special = (0..79)
        .map(|i| rom.w(0x82dd8a + i * 2))
        .collect::<Result<Vec<_>>>()?;
    for (name, address, width) in [
        ("kSpExit_Top", 0x82e6e1, 2),
        ("kSpExit_Bottom", 0x82e701, 2),
        ("kSpExit_Left", 0x82e721, 2),
        ("kSpExit_Right", 0x82e741, 2),
        ("kSpExit_Tab4", 0x82e761, 2),
        ("kSpExit_Tab6", 0x82e781, 2),
        ("kSpExit_Tab5", 0x82e7a1, 2),
        ("kSpExit_Tab7", 0x82e7c1, 2),
        ("kSpExit_LeftEdgeOfMap", 0x82e7e1, 2),
        ("kSpExit_Dir", 0x82e801, 1),
        ("kSpExit_SprGfx", 0x82e811, 1),
        ("kSpExit_AuxGfx", 0x82e821, 1),
        ("kSpExit_PalBg", 0x82e831, 1),
        ("kSpExit_PalSpr", 0x82e841, 1),
    ] {
        let mut data = vec![0; 16 * width];
        for i in 0..16 {
            if special.contains(&(i + 0x180)) {
                data[i * width..(i + 1) * width]
                    .copy_from_slice(&rom.bytes(address + i * width, width)?);
            }
        }
        out.insert(name, data);
    }
    let mut secrets = Vec::new();
    let mut offsets = vec![None; 128];
    for &area in heads.iter().filter(|&&i| i < 128) {
        let mut p = 0x9b0000 | rom.w(0x9bc2f9 + area * 2)?;
        if rom.w(p)? == 65535 {
            continue;
        }
        let start = secrets.len();
        offsets[area] = Some(start);
        if small[area] == 0 {
            for a in [area + 1, area + 8, area + 9] {
                offsets[a] = Some(start);
            }
        }
        while rom.w(p)? != 65535 {
            secrets.extend(rom.bytes(p, 3)?);
            p += 3;
        }
        secrets.extend([255, 255]);
    }
    out.insert(
        "kOverworldSecrets_Offs",
        words(offsets.iter().map(|i| i.unwrap_or(secrets.len() - 2))),
    );
    out.insert("kOverworldSecrets", secrets);
    let mut sprites = vec![255];
    let mut offsets = vec![0; 144 * 3];
    for (start, end, table, stages) in [
        (0, 64, 0x89c881, vec![0]),
        (0, 64, 0x89c901, vec![1]),
        (0, 64, 0x89ca21, vec![2]),
        (64, 144, 0x89ca21, vec![1, 2]),
    ] {
        for &area in heads.iter().filter(|&&i| i >= start && i < end) {
            let mut p = 0x890000 + rom.w(table + area * 2)?;
            if rom.b(p)? == 255 {
                continue;
            }
            for &stage in &stages {
                offsets[stage * 144 + area] = sprites.len();
            }
            while rom.b(p)? != 255 {
                sprites.extend(rom.bytes(p, 3)?);
                p += 3;
            }
            sprites.push(255);
        }
    }
    out.insert("kOverworldSpriteOffs", words(offsets));
    out.insert("kOverworldSprites", sprites);
    Ok(())
}
// The YAML round trip in the reference loader restores the original object
// records, while always emitting a third-layer door list and clearing reserved bits.
fn layer(
    rom: &Rom<'_>,
    p: &mut usize,
    data: &mut Vec<u8>,
    force_doors: bool,
) -> Result<Option<usize>> {
    while rom.w(*p)? != 65535 && rom.w(*p)? != 65520 {
        data.extend(rom.bytes(*p, 3)?);
        *p += 3;
    }
    let doors = rom.w(*p)? == 65520;
    *p += 2;
    let mut records = Vec::new();
    if doors {
        while rom.w(*p)? != 65535 {
            records.extend([rom.b(*p)? & 0xf3, rom.b(*p + 1)?]);
            *p += 2;
        }
        *p += 2;
    }
    let offset = if force_doors || !records.is_empty() {
        data.extend([0xf0, 0xff]);
        let offset = data.len();
        data.extend(records);
        Some(offset)
    } else {
        None
    };
    data.extend([255, 255]);
    Ok(offset)
}
fn dungeon(rom: &Rom<'_>, out: &mut Assets) -> Result<()> {
    let mut data = Vec::new();
    let mut offsets = Vec::new();
    let mut doors = Vec::new();
    let mut headers = Vec::new();
    let mut header_offsets = Vec::new();
    let mut chests = Vec::new();
    let mut sprites = vec![0, 255];
    let mut sprite_offsets = vec![0; 320];
    let mut secrets = vec![0; 640];
    let mut secret_offsets = vec![None; 320];
    for i in 0..320 {
        let mut p = rom.p(0x1f8000 + i * 3)?;
        offsets.push(data.len());
        data.extend(rom.bytes(p, 2)?);
        p += 2;
        layer(rom, &mut p, &mut data, false)?;
        layer(rom, &mut p, &mut data, false)?;
        doors.push(layer(rom, &mut p, &mut data, true)?.context("missing third layer door list")?);
        let mut p = 0x40000 | rom.w(0x4f502 + i * 2)?;
        if p == 0x4ffef {
            p = 0x82edc5;
        }
        let mut header = rom.bytes(p, 14)?;
        header[0] &= 0xfd;
        header[8] &= 3;
        let overlap = (0..=header.len().min(headers.len()))
            .rev()
            .find(|&n| headers[headers.len() - n..] == header[..n])
            .unwrap();
        header_offsets.push(headers.len() - overlap);
        headers.extend_from_slice(&header[overlap..]);
        for j in 0..168 {
            let room = rom.w(0x81e96e + j * 3)?;
            if room & 0x7fff == i {
                chests.extend(words([room]));
                chests.push(rom.b(0x81e970 + j * 3)?);
            }
        }
        let mut p = 0x890000 + rom.w(0x89d62e + i * 2)?;
        let sort = rom.b(p)?;
        p += 1;
        if sort != 0 || rom.b(p)? != 255 {
            sprite_offsets[i] = sprites.len();
            sprites.push(sort);
            while rom.b(p)? != 255 {
                let mut record = rom.bytes(p, 3)?;
                if record[1] >= 0xe0 && record[2] != 0xe4 {
                    record[0] &= 0x9f;
                }
                sprites.extend(record);
                p += 3;
            }
            sprites.push(255);
        }
        let mut p = 0x810000 | rom.w(0x81db69 + i * 2)?;
        if rom.w(p)? != 65535 {
            secret_offsets[i] = Some(secrets.len());
            while rom.w(p)? != 65535 {
                secrets.extend(rom.bytes(p, 3)?);
                p += 3;
            }
            secrets.extend([255, 255]);
        }
    }
    for (i, offset) in secret_offsets.into_iter().enumerate() {
        let offset = offset.unwrap_or(secrets.len() - 2);
        secrets[i * 2..i * 2 + 2].copy_from_slice(&(offset as u16).to_le_bytes());
    }
    for (name, data) in [
        ("kDungeonRoom", data),
        ("kDungeonRoomOffs", words(offsets)),
        ("kDungeonRoomDoorOffs", words(doors)),
        ("kDungeonRoomHeaders", headers),
        ("kDungeonRoomHeadersOffs", words(header_offsets)),
        ("kDungeonRoomChests", chests),
        ("kDungeonSprites", sprites),
        ("kDungeonSpriteOffs", words(sprite_offsets)),
        ("kDungeonSecrets", secrets),
    ] {
        out.insert(name, data);
    }
    out.insert("kDungeonRoomTeleMsg", rom.bytes(0x87f61d, 640)?);
    let mut pits = (0..57)
        .map(|i| rom.w(0x80990c + i * 2))
        .collect::<Result<Vec<_>>>()?;
    pits.sort();
    pits.dedup();
    out.insert("kDungeonPitsHurtPlayer", words(pits));
    for (name, offname, table, count) in [
        (
            "kDungeonRoomDefault",
            "kDungeonRoomDefaultOffs",
            0x84ef2f,
            8,
        ),
        (
            "kDungeonRoomOverlay",
            "kDungeonRoomOverlayOffs",
            0x84ecc0,
            19,
        ),
    ] {
        let mut data = Vec::new();
        let mut offsets = Vec::new();
        for i in 0..count {
            let mut p = rom.p(table + i * 3)?;
            offsets.push(data.len());
            layer(rom, &mut p, &mut data, false)?;
        }
        out.insert(name, data);
        out.insert(offname, words(offsets));
    }
    Ok(())
}
fn dialogue(rom: &Rom<'_>, out: &mut Assets) -> Result<()> {
    let mut messages = Vec::new();
    let mut text = String::new();
    let mut p = 0x9c8000;
    for _ in 0..1048576 {
        let c = rom.b(p)? as usize;
        p += 1;
        match c {
            255 => break,
            128 => {
                p = 0x8edf40;
                text.clear();
            }
            127 => {
                messages.push(std::mem::take(&mut text));
            }
            0..=102 => text.push_str(ALPHABET.get(c).context("invalid dialogue alphabet index")?),
            103..=126 => {
                let cmd = c - 103;
                let name = COMMAND_NAMES.get(cmd).context("invalid dialogue command")?;
                if COMMAND_LENGTHS[cmd] == 2 {
                    let param = rom.b(p)?;
                    p += 1;
                    text.push_str(&format!("[{name} {param:02}]"));
                } else {
                    text.push_str(&format!("[{name}]"));
                }
            }
            136..=254 => text.push_str(
                DICTIONARY
                    .get(c - 136)
                    .context("invalid dialogue dictionary index")?,
            ),
            _ => bail!("invalid dialogue bank marker"),
        }
    }
    if messages.len() == 396 {
        messages.insert(
            4,
            "[Speed 00]0- [Number 00]. 1- [Number 01][2]2- [Number 02]. 3- [Number 03]".into(),
        );
    }
    if messages.len() != 397 {
        bail!("unexpected USA dialogue count");
    }
    let dictionary = DICTIONARY
        .iter()
        .map(|word| {
            word.chars()
                .map(|c| {
                    ALPHABET
                        .iter()
                        .rposition(|a| *a == c.to_string())
                        .map(|i| i as u8)
                        .context("unknown dictionary glyph")
                })
                .collect::<Result<Vec<_>>>()
        })
        .collect::<Result<Vec<_>>>()?;
    let mut encoded = Vec::new();
    for message in messages {
        let mut bytes = Vec::new();
        let mut text = message.as_str();
        while !text.is_empty() {
            if let Some((i, word)) = DICTIONARY
                .iter()
                .enumerate()
                .find(|(_, word)| text.starts_with(**word))
            {
                bytes.push(i as u8 + 136);
                text = &text[word.len()..];
                continue;
            }
            if text.starts_with('[') {
                let end = text.find(']').context("unterminated dialogue token")?;
                let token = &text[..end + 1];
                if let Some(i) = ALPHABET.iter().rposition(|a| *a == token) {
                    bytes.push(i as u8);
                } else {
                    let mut parts = text[1..end].split(' ');
                    let command = parts.next().unwrap();
                    let i = COMMAND_NAMES
                        .iter()
                        .position(|a| *a == command)
                        .context("unknown dialogue command")?;
                    bytes.push(i as u8 + 103);
                    if let Some(param) = parts.next() {
                        bytes.push(param.parse()?);
                    }
                }
                text = &text[end + 1..];
            } else {
                let c = text.chars().next().unwrap();
                let i = ALPHABET
                    .iter()
                    .rposition(|a| *a == c.to_string())
                    .context("unknown dialogue glyph")?;
                bytes.push(i as u8);
                text = &text[c.len_utf8()..];
            }
        }
        encoded.push(bytes);
    }
    out.insert(
        "kDialogue",
        pack(&[pack(&[pack(&dictionary), pack(&encoded)])]),
    );
    Ok(())
}
fn music(rom: &Rom<'_>, out: &mut Assets) -> Result<()> {
    // SPC destination extents for the pinned USA layout. These are loader
    // metadata only: every byte of music, instruments and samples comes from ROM.
    for (name, address, ranges) in [
        (
            "kSoundBank_intro",
            0x998000,
            &[
                (0x17c0, 0x1090),
                (0x2880, 0x20b),
                (0x2b00, 0x688),
                (0x3c00, 0x70),
                (0x3d00, 0xae),
                (0x3e00, 0xe1),
                (0x4000, 0x7aa0),
                (0xd000, 0x2dae),
            ][..],
        ),
        (
            "kSoundBank_indoor",
            0x9b8000,
            &[(0x2b00, 0x50c), (0xd000, 0x2cbe)][..],
        ),
        (
            "kSoundBank_ending",
            0x9ad380,
            &[(0x2900, 0x1038), (0xd000, 0x1060)][..],
        ),
    ] {
        let mut memory = vec![None; 65536];
        let mut p = Rom::offset(address)?;
        for _ in 0..256 {
            let header = rom.0.get(p..p + 4).context("truncated SPC bank")?;
            let size = u16::from_le_bytes([header[0], header[1]]) as usize;
            if size == 0 {
                break;
            }
            let target = u16::from_le_bytes([header[2], header[3]]) as usize;
            p += 4;
            let data = rom.0.get(p..p + size).context("truncated SPC block")?;
            let target = memory
                .get_mut(target..target + size)
                .context("SPC block out of bounds")?;
            for (target, &source) in target.iter_mut().zip(data) {
                *target = Some(source);
            }
            p += size;
        }
        let mut bank = Vec::new();
        for &(target, size) in ranges {
            bank.extend(words([size, target]));
            for value in &memory[target..target + size] {
                bank.push(value.context("missing SPC resource byte")?);
            }
        }
        bank.extend([0, 0]);
        out.insert(name, bank);
    }
    Ok(())
}
pub fn decode(bytes: &[u8]) -> Result<Vec<u8>> {
    let rom = checked_rom(bytes)?;
    let mut assets = assets(&rom)?;
    if assets.len() != NAMES.len() {
        bail!("incomplete Zelda3 resource layout");
    }
    let keys = NAMES
        .iter()
        .flat_map(|name| name.as_bytes().iter().copied().chain([0]))
        .collect::<Vec<_>>();
    let mut result = b"Zelda3_v0     \n\0".to_vec();
    result.extend(Sha256::digest(&keys));
    result.extend([0; 32]);
    result.extend((NAMES.len() as u32).to_le_bytes());
    result.extend((keys.len() as u32).to_le_bytes());
    for name in NAMES {
        result.extend(
            (assets.get(name).context("missing Zelda3 resource")?.len() as u32).to_le_bytes(),
        );
    }
    result.extend(keys);
    for name in NAMES {
        while result.len() & 3 != 0 {
            result.push(0);
        }
        result.extend(assets.remove(name).unwrap());
    }
    if result.len() > 8388608 {
        bail!("Zelda3 resource limit exceeded");
    }
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_roms_rejected() {
        for bytes in [Vec::new(), vec![0; 1048576], vec![0; 1049088]] {
            assert!(checked_rom(&bytes).is_err());
        }
    }
    #[test]
    #[ignore = "requires the player's local ROM and development reference hashes"]
    fn native_resources_match_reference() {
        let bytes = std::fs::read(std::env::var_os("GAMEPKG_ZELDA3_TEST_ROM").unwrap()).unwrap();
        let expected: BTreeMap<String, (usize, String)> = serde_json::from_slice(
            &std::fs::read(std::env::var_os("GAMEPKG_ZELDA3_REFERENCE").unwrap()).unwrap(),
        )
        .unwrap();
        let resources = assets(&checked_rom(&bytes).unwrap()).unwrap();
        let mut failed = Vec::new();
        for (name, data) in &resources {
            if (data.len(), hex::encode(Sha256::digest(data))) != expected[*name] {
                failed.push(*name);
            }
        }
        println!(
            "{} resources checked; mismatches: {:?}",
            resources.len(),
            failed
        );
        assert!(failed.is_empty());
        assert_eq!(resources.len(), 165);
        let raw = decode(&bytes).unwrap();
        assert_eq!(
            (raw.len(), hex::encode(Sha256::digest(&raw))),
            expected["__blob"]
        );
        let mut headered = vec![0; 512];
        headered.extend_from_slice(&bytes);
        assert_eq!(decode(&headered).unwrap(), raw);
    }
}
