use mh_core::{BackupSchedule, ServerId};

use super::{BackupCandidate, BackupPlanner};

const DAY: i64 = 20_000 * 86_400;
const HOUR: i64 = 3600;

fn at(hour: i64) -> (i64, u32) {
    (
        DAY + hour * HOUR,
        u32::try_from((hour * HOUR).rem_euclid(86_400)).unwrap(),
    )
}

fn candidate(id: ServerId, schedule: &str, last: Option<i64>, running: bool) -> BackupCandidate {
    BackupCandidate {
        id,
        schedule: BackupSchedule::parse(schedule).unwrap(),
        last,
        running,
    }
}

#[test]
fn first_backup_runs_even_if_server_never_ran() {
    let id = ServerId::generate();
    let mut planner = BackupPlanner::default();
    let (now, of_day) = at(10);
    assert_eq!(
        planner.tick(now, of_day, &[candidate(id, "every_6h", None, false)]),
        [id]
    );
}

#[test]
fn stopped_since_last_backup_is_skipped() {
    let id = ServerId::generate();
    let mut planner = BackupPlanner::default();
    let (now, of_day) = at(10);
    let last = Some(now - 7 * HOUR);
    assert!(
        planner
            .tick(now, of_day, &[candidate(id, "every_6h", last, false)])
            .is_empty()
    );
    // Server bật lên: lượt kế tiếp sao lưu, vì world đã có thể đổi.
    assert_eq!(
        planner.tick(
            now + 60,
            of_day + 60,
            &[candidate(id, "every_6h", last, true)]
        ),
        [id]
    );
}

#[test]
fn not_due_and_off_do_nothing() {
    let id = ServerId::generate();
    let mut planner = BackupPlanner::default();
    let (now, of_day) = at(10);
    assert!(
        planner
            .tick(
                now,
                of_day,
                &[candidate(id, "every_6h", Some(now - HOUR), true)]
            )
            .is_empty()
    );
    assert!(
        planner
            .tick(now, of_day, &[candidate(id, "off", None, true)])
            .is_empty()
    );
    assert!(
        planner
            .tick(
                now,
                of_day,
                &[candidate(
                    id,
                    "daily@04:00",
                    Some(DAY + 4 * HOUR + 60),
                    true
                )]
            )
            .is_empty()
    );
}

#[test]
fn job_in_flight_is_not_started_twice_but_retried_after_an_hour() {
    let id = ServerId::generate();
    let mut planner = BackupPlanner::default();
    let (now, of_day) = at(10);
    let last = Some(now - 7 * HOUR);
    assert_eq!(
        planner.tick(now, of_day, &[candidate(id, "every_6h", last, true)]),
        [id]
    );
    // Phút sau Job chưa ghi xong bản mới.
    assert!(
        planner
            .tick(
                now + 60,
                of_day + 60,
                &[candidate(id, "every_6h", last, true)]
            )
            .is_empty()
    );
    // Job lỗi, một giờ sau vẫn chưa có bản mới: thử lại.
    let (later, later_of_day) = at(11);
    assert_eq!(
        planner.tick(
            later,
            later_of_day,
            &[candidate(id, "every_6h", last, true)]
        ),
        [id]
    );
}

#[test]
fn daily_backup_runs_once_after_its_time() {
    let id = ServerId::generate();
    let mut planner = BackupPlanner::default();
    let yesterday = Some(DAY - 20 * HOUR);
    let (now, of_day) = at(3);
    assert!(
        planner
            .tick(
                now,
                of_day,
                &[candidate(id, "daily@04:00", yesterday, true)]
            )
            .is_empty()
    );
    let (now, of_day) = at(4);
    assert_eq!(
        planner.tick(
            now,
            of_day,
            &[candidate(id, "daily@04:00", yesterday, true)]
        ),
        [id]
    );
    let (now, of_day) = at(5);
    let today = Some(DAY + 4 * HOUR + 30);
    assert!(
        planner
            .tick(now, of_day, &[candidate(id, "daily@04:00", today, true)])
            .is_empty()
    );
}
