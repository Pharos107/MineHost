//! ZIP không nén viết tay cho file CurseForge "cấm phân phối" của chế độ e2e.
//!
//! Phải trùng **từng byte** với `buildStoredZip` trong `ui/e2e/curseforge-pending.spec.ts`: test
//! Playwright tự dựng lại jar trong trình duyệt rồi kéo vào để khớp SHA-1 của mục chờ. Vì vậy không
//! dùng crate `zip` (header của nó khác giữa các bản và hệ điều hành). Sửa một bên thì sửa cả bên kia.

/// Tạo `.jar` (ZIP không nén) cố định từng byte để Playwright dựng lại khớp SHA-1.
pub(super) fn build_stored_jar(entry_name: &str, body: &[u8]) -> Vec<u8> {
    let name = entry_name.as_bytes();
    let crc = crc32(body);
    let len = body.len() as u32;
    let nlen = name.len() as u16;

    let mut local = vec![0u8; 30 + name.len()];
    local[0..4].copy_from_slice(&0x0403_4b50_u32.to_le_bytes());
    local[4..6].copy_from_slice(&20_u16.to_le_bytes());
    local[14..18].copy_from_slice(&crc.to_le_bytes());
    local[18..22].copy_from_slice(&len.to_le_bytes());
    local[22..26].copy_from_slice(&len.to_le_bytes());
    local[26..28].copy_from_slice(&nlen.to_le_bytes());
    local[30..].copy_from_slice(name);

    let mut cd = vec![0u8; 46 + name.len()];
    cd[0..4].copy_from_slice(&0x0201_4b50_u32.to_le_bytes());
    cd[4..6].copy_from_slice(&20_u16.to_le_bytes());
    cd[6..8].copy_from_slice(&20_u16.to_le_bytes());
    cd[16..20].copy_from_slice(&crc.to_le_bytes());
    cd[20..24].copy_from_slice(&len.to_le_bytes());
    cd[24..28].copy_from_slice(&len.to_le_bytes());
    cd[28..30].copy_from_slice(&nlen.to_le_bytes());
    cd[46..].copy_from_slice(name);

    let cd_offset = (local.len() + body.len()) as u32;
    let mut eocd = vec![0u8; 22];
    eocd[0..4].copy_from_slice(&0x0605_4b50_u32.to_le_bytes());
    eocd[8..10].copy_from_slice(&1_u16.to_le_bytes());
    eocd[10..12].copy_from_slice(&1_u16.to_le_bytes());
    eocd[12..16].copy_from_slice(&(cd.len() as u32).to_le_bytes());
    eocd[16..20].copy_from_slice(&cd_offset.to_le_bytes());

    [local, body.to_vec(), cd, eocd].concat()
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffff_u32;
    for &b in bytes {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = (crc >> 1) ^ if crc & 1 != 0 { 0xedb8_8320 } else { 0 };
        }
    }
    !crc
}
