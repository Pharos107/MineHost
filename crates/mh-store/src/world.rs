//! Điểm spawn đọc từ `level.dat` (spec P2, mục 6.7).
//!
//! Nằm ở `mh-store` vì đây là file trong thư mục server, cùng loại với `whitelist.json`.
//! `mh-providers` chỉ gọi API bên ngoài. Spec mục 7 ghi `fastnbt` ở `mh-providers`;
//! đặt thư viện cạnh hàm đọc file để không thêm cạnh phụ thuộc giữa hai crate.

use std::fs::File;
use std::io::Read;
use std::path::Path;

use fastnbt::Value;
use flate2::read::GzDecoder;

use crate::StoreError;

/// `level.dat` chỉ là metadata của world, không chứa chunk. Chặn gzip phình bộ nhớ.
const MAX_LEVEL_DAT: usize = 16 * 1024 * 1024;

/// Tọa độ tâm khối spawn: `+0.5` cho X và Z, Y giữ nguyên.
pub fn world_spawn(level_dat: &Path) -> Result<(f64, f64, f64), StoreError> {
    let root = read_nbt(level_dat)?;
    let data = compound_get(&root, "Data")
        .ok_or_else(|| StoreError::invalid_world(level_dat, "thiếu khóa Data"))?;
    let (x, y, z) = spawn_block(data)
        .ok_or_else(|| StoreError::invalid_world(level_dat, "không có SpawnX/Y/Z hay spawn.pos"))?;
    Ok(((x as f64) + 0.5, y as f64, (z as f64) + 0.5))
}

fn read_nbt(path: &Path) -> Result<Value, StoreError> {
    let file = File::open(path).map_err(|err| StoreError::io(path, err))?;
    let bytes = gunzip_limited(path, file)?;
    fastnbt::from_bytes(&bytes).map_err(|err| StoreError::invalid_world(path, err))
}

fn gunzip_limited(path: &Path, file: File) -> Result<Vec<u8>, StoreError> {
    let mut decoder = GzDecoder::new(file);
    let mut bytes = Vec::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = match decoder.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(err) => return Err(StoreError::invalid_world(path, err)),
        };
        if bytes.len().saturating_add(n) > MAX_LEVEL_DAT {
            return Err(StoreError::invalid_world(path, "giải nén lớn hơn 16 MiB"));
        }
        bytes.extend_from_slice(&buf[..n]);
    }
    Ok(bytes)
}

fn spawn_block(data: &Value) -> Option<(i64, i64, i64)> {
    legacy_spawn(data).or_else(|| new_spawn(data))
}

fn legacy_spawn(data: &Value) -> Option<(i64, i64, i64)> {
    Some((
        int_field(data, "SpawnX")?,
        int_field(data, "SpawnY")?,
        int_field(data, "SpawnZ")?,
    ))
}

/// Minecraft 26.x có thể chuyển spawn sang `Data.spawn.pos` (mảng 3 int, list hoặc IntArray).
/// Chưa kiểm chứng với bản 26.x thật; chỉ dùng khi không còn `SpawnX`/`SpawnY`/`SpawnZ`.
fn new_spawn(data: &Value) -> Option<(i64, i64, i64)> {
    let spawn = compound_get(data, "spawn")?;
    let pos = compound_get(spawn, "pos")?;
    three_ints(pos)
}

fn int_field(data: &Value, key: &str) -> Option<i64> {
    compound_get(data, key)?.as_i64()
}

fn compound_get<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    match value {
        Value::Compound(map) => map.get(key),
        _ => None,
    }
}

fn three_ints(value: &Value) -> Option<(i64, i64, i64)> {
    match value {
        Value::List(items) => three_values(items),
        Value::IntArray(items) => three_i32(items),
        _ => None,
    }
}

fn three_values(items: &[Value]) -> Option<(i64, i64, i64)> {
    if items.len() != 3 {
        return None;
    }
    Some((items[0].as_i64()?, items[1].as_i64()?, items[2].as_i64()?))
}

fn three_i32(items: &[i32]) -> Option<(i64, i64, i64)> {
    if items.len() != 3 {
        return None;
    }
    Some((
        i64::from(items[0]),
        i64::from(items[1]),
        i64::from(items[2]),
    ))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::collections::HashMap;
    use std::io::Write;

    use fastnbt::{IntArray, Value};
    use flate2::Compression;
    use flate2::write::GzEncoder;

    use super::world_spawn;

    fn compound(pairs: &[(&str, Value)]) -> Value {
        Value::Compound(
            pairs
                .iter()
                .map(|(key, value)| ((*key).to_owned(), value.clone()))
                .collect::<HashMap<_, _>>(),
        )
    }

    fn write_level(data: Value) -> tempfile::TempDir {
        let root = compound(&[("Data", data)]);
        let nbt = fastnbt::to_bytes(&root).unwrap();
        let mut gzip = GzEncoder::new(Vec::new(), Compression::fast());
        gzip.write_all(&nbt).unwrap();
        let bytes = gzip.finish().unwrap();
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("level.dat"), bytes).unwrap();
        dir
    }

    #[test]
    fn legacy_spawn_is_block_center() {
        let dir = write_level(compound(&[
            ("SpawnX", Value::Int(10)),
            ("SpawnY", Value::Int(64)),
            ("SpawnZ", Value::Int(-5)),
        ]));
        let got = world_spawn(&dir.path().join("level.dat")).unwrap();
        assert_eq!(got, (10.5, 64.0, -4.5));
    }

    #[test]
    fn spawn_pos_array_when_legacy_keys_are_absent() {
        let ints = compound(&[(
            "spawn",
            compound(&[("pos", Value::IntArray(IntArray::new(vec![10, 64, -5])))]),
        )]);
        let list = compound(&[(
            "spawn",
            compound(&[(
                "pos",
                Value::List(vec![Value::Int(10), Value::Int(64), Value::Int(-5)]),
            )]),
        )]);
        for data in [ints, list] {
            let dir = write_level(data);
            let got = world_spawn(&dir.path().join("level.dat")).unwrap();
            assert_eq!(got, (10.5, 64.0, -4.5));
        }
    }

    #[test]
    fn missing_spawn_is_config_invalid() {
        let dir = write_level(compound(&[(
            "LevelName",
            Value::String("world".to_owned()),
        )]));
        let err = world_spawn(&dir.path().join("level.dat")).unwrap_err();
        assert_eq!(err.code(), mh_core::ErrorCode::ConfigInvalid);
    }
}
