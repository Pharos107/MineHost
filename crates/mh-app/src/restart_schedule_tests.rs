use mh_core::{DailyTime, ServerId};

use super::{RestartAction, RestartPlanner, RestartWarning, ServerSchedule};

/// 04:00 giờ máy. `now` là giây Unix giả, chỉ cần tăng cùng nhịp với `of_day`.
const FOUR: u32 = 4 * 3600;
const BASE: i64 = 1_000_000;

fn at(of_day: u32) -> (i64, u32) {
    (BASE + i64::from(of_day), of_day)
}

fn schedule(id: ServerId, times: &[&str], running: bool) -> ServerSchedule {
    ServerSchedule {
        id,
        times: times.iter().map(|t| DailyTime::parse(t).unwrap()).collect(),
        running,
    }
}

/// Chạy từng giây trong `[from, to]`, gom mọi việc kèm giây xảy ra.
fn run(
    planner: &mut RestartPlanner,
    servers: &[ServerSchedule],
    from: u32,
    to: u32,
) -> Vec<(u32, RestartAction)> {
    let mut out = Vec::new();
    for second in from..=to {
        let (now, of_day) = at(second);
        for action in planner.tick(now, of_day, servers) {
            out.push((second, action));
        }
    }
    out
}

#[test]
fn three_warnings_then_restart_once() {
    let id = ServerId::generate();
    let servers = [schedule(id, &["04:00"], true)];
    let mut planner = RestartPlanner::default();
    let got = run(&mut planner, &servers, FOUR - 600, FOUR + 120);
    assert_eq!(
        got,
        [
            (
                FOUR - 300,
                RestartAction::Warn(id, RestartWarning::FiveMinutes)
            ),
            (
                FOUR - 60,
                RestartAction::Warn(id, RestartWarning::OneMinute)
            ),
            (
                FOUR - 10,
                RestartAction::Warn(id, RestartWarning::TenSeconds)
            ),
            (FOUR, RestartAction::Restart(id)),
        ]
    );
}

#[test]
fn stopped_server_is_skipped_entirely() {
    let id = ServerId::generate();
    let servers = [schedule(id, &["04:00"], false)];
    let mut planner = RestartPlanner::default();
    assert!(run(&mut planner, &servers, FOUR - 600, FOUR + 120).is_empty());
}

#[test]
fn late_start_sends_only_what_is_due() {
    let id = ServerId::generate();
    let servers = [schedule(id, &["04:00"], true)];
    let mut planner = RestartPlanner::default();
    let got = run(&mut planner, &servers, FOUR - 30, FOUR);
    assert_eq!(
        got,
        [
            (
                FOUR - 30,
                RestartAction::Warn(id, RestartWarning::OneMinute)
            ),
            (
                FOUR - 10,
                RestartAction::Warn(id, RestartWarning::TenSeconds)
            ),
            (FOUR, RestartAction::Restart(id)),
        ]
    );
}

#[test]
fn skipped_second_still_restarts() {
    let id = ServerId::generate();
    let servers = [schedule(id, &["04:00"], true)];
    let mut planner = RestartPlanner::default();
    let (now, of_day) = at(FOUR - 1);
    planner.tick(now, of_day, &servers);
    // Lượt kế tiếp tới trễ, đã qua 04:00:00.
    let (now, of_day) = at(FOUR + 1);
    assert_eq!(
        planner.tick(now, of_day, &servers),
        [RestartAction::Restart(id)]
    );
    let (now, of_day) = at(FOUR + 2);
    assert!(planner.tick(now, of_day, &servers).is_empty());
}

#[test]
fn same_second_twice_restarts_once() {
    let id = ServerId::generate();
    let servers = [schedule(id, &["04:00"], true)];
    let mut planner = RestartPlanner::default();
    let (now, of_day) = at(FOUR);
    assert_eq!(
        planner.tick(now, of_day, &servers),
        [RestartAction::Restart(id)]
    );
    assert!(planner.tick(now, of_day, &servers).is_empty());
}

#[test]
fn removed_schedule_cancels_pending_restart() {
    let id = ServerId::generate();
    let mut planner = RestartPlanner::default();
    let on = [schedule(id, &["04:00"], true)];
    let (now, of_day) = at(FOUR - 100);
    planner.tick(now, of_day, &on);
    let off = [schedule(id, &[], true)];
    assert!(run(&mut planner, &off, FOUR - 99, FOUR + 5).is_empty());
    let moved = [schedule(id, &["05:00"], true)];
    assert!(run(&mut planner, &moved, FOUR + 6, FOUR + 60).is_empty());
}
