//! Ai đang online, theo log của từng server (spec P2, mục 3.2).
//!
//! Supervisor phát `Players(...)` từ log; app gom lại ở đây. Mỗi lần danh sách hoặc file
//! người chơi đổi thì phát id server qua kênh, để WebSocket báo giao diện tải lại.

use std::collections::HashMap;
use std::sync::Mutex;

use mh_core::{PlayerLogEvent, PlayerName, ServerId};
use tokio::sync::broadcast;

#[derive(Debug)]
pub(crate) struct OnlinePlayers {
    by_server: Mutex<HashMap<ServerId, Vec<PlayerName>>>,
    changed: broadcast::Sender<ServerId>,
}

impl OnlinePlayers {
    pub(crate) fn new() -> Self {
        Self {
            by_server: Mutex::new(HashMap::new()),
            changed: broadcast::channel(256).0,
        }
    }

    /// Áp một sự kiện log. `true` khi danh sách thật sự đổi.
    pub(crate) fn apply(&self, id: ServerId, event: &PlayerLogEvent) -> bool {
        let mut map = crate::lock(&self.by_server);
        let names = map.entry(id).or_default();
        match event {
            PlayerLogEvent::Joined(name) => {
                if position(names, name).is_some() {
                    return false;
                }
                names.push(name.clone());
                true
            }
            PlayerLogEvent::Left(name) => match position(names, name) {
                Some(index) => {
                    names.remove(index);
                    true
                }
                None => false,
            },
            PlayerLogEvent::List { names: listed, .. } => {
                if same_names(names, listed) {
                    return false;
                }
                names.clone_from(listed);
                true
            }
        }
    }

    /// Server tắt hoặc bật lại: không ai còn online. `true` khi trước đó có người.
    pub(crate) fn clear(&self, id: ServerId) -> bool {
        crate::lock(&self.by_server)
            .remove(&id)
            .is_some_and(|names| !names.is_empty())
    }

    pub(crate) fn names(&self, id: ServerId) -> Vec<PlayerName> {
        crate::lock(&self.by_server)
            .get(&id)
            .cloned()
            .unwrap_or_default()
    }

    pub(crate) fn count(&self, id: ServerId) -> u32 {
        let len = crate::lock(&self.by_server).get(&id).map_or(0, Vec::len);
        u32::try_from(len).unwrap_or(u32::MAX)
    }

    pub(crate) fn notify(&self, id: ServerId) {
        // Không ai nghe thì thôi: lần sau giao diện tự đọc lại REST.
        let _ = self.changed.send(id);
    }

    pub(crate) fn sender(&self) -> broadcast::Sender<ServerId> {
        self.changed.clone()
    }

    pub(crate) fn subscribe(&self) -> broadcast::Receiver<ServerId> {
        self.changed.subscribe()
    }
}

/// Minecraft không phân biệt hoa thường khi so tên người chơi.
fn position(names: &[PlayerName], name: &PlayerName) -> Option<usize> {
    names
        .iter()
        .position(|have| have.as_str().eq_ignore_ascii_case(name.as_str()))
}

fn same_names(have: &[PlayerName], listed: &[PlayerName]) -> bool {
    have.len() == listed.len() && listed.iter().all(|name| position(have, name).is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name(raw: &str) -> PlayerName {
        PlayerName::parse(raw).unwrap()
    }

    #[test]
    fn join_leave_and_list_keep_one_entry_per_player() {
        let online = OnlinePlayers::new();
        let id = ServerId::generate();
        assert!(online.apply(id, &PlayerLogEvent::Joined(name("Steve"))));
        // Tên khác hoa thường vẫn là một người.
        assert!(!online.apply(id, &PlayerLogEvent::Joined(name("steve"))));
        assert!(online.apply(id, &PlayerLogEvent::Joined(name("Alex"))));
        assert_eq!(online.count(id), 2);
        assert!(online.apply(id, &PlayerLogEvent::Left(name("STEVE"))));
        assert!(!online.apply(id, &PlayerLogEvent::Left(name("Notch"))));
        assert_eq!(online.names(id), vec![name("Alex")]);

        let list = PlayerLogEvent::List {
            online: 2,
            max: 20,
            names: vec![name("Alex"), name("Herobrine")],
        };
        assert!(online.apply(id, &list));
        assert!(!online.apply(id, &list));
        assert_eq!(online.count(id), 2);
        assert!(online.clear(id));
        assert!(!online.clear(id));
        assert_eq!(online.count(id), 0);
    }
}
