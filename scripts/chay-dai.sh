#!/usr/bin/env bash
# Lần chạy dài của P2 (tiêu chí 1 của spec P2, PR-34): N server Paper chạy liên tục, ghi RSS của
# controller và trạng thái từng server mỗi 10 phút. Chỉ dùng trên Linux có systemd --user, vì
# controller phải sống sau khi đóng SSH (ops/linux-systemd.md).
#
#   scripts/chay-dai.sh start <binary> [số server] [giờ]   dựng dữ liệu, bật controller và vòng ghi
#   scripts/chay-dai.sh status                              xem vài dòng ghi gần nhất
#   scripts/chay-dai.sh stop                                bấm Thoát, dừng vòng ghi
#
# Biến môi trường: MH_DIR (mặc định ~/mh-chay-dai), MH_PORT (7979), MH_GAME_PORT (25600),
# MH_MC (1.21.4), MH_MEM_MAX (768), MH_RAM_CAP (giới hạn RAM của cả nhóm, 11G),
# MH_MIN_DISK_GB (5: ổ còn ít hơn thì tự dừng để không làm đầy ổ của dịch vụ khác).
set -euo pipefail

DIR=${MH_DIR:-$HOME/mh-chay-dai}
PORT=${MH_PORT:-7979}
GAME_PORT=${MH_GAME_PORT:-25600}
MC=${MH_MC:-1.21.4}
# Paper 1.21 nhàn rỗi với heap 512 MB chiếm khoảng 0,9 GB RSS; 768 MB thì khoảng 1,2 GB.
MEM_MAX=${MH_MEM_MAX:-512}
RAM_CAP=${MH_RAM_CAP:-10G}
MIN_DISK_GB=${MH_MIN_DISK_GB:-5}
API="http://127.0.0.1:$PORT/api/v1"
UNIT=mh-chay-dai
LOG="$DIR/ket-qua.csv"

say() { printf '%s %s\n' "$(date -Is)" "$*" | tee -a "$DIR/su-kien.log" >&2; }

# Mọi request thay đổi dữ liệu cần `X-MineHost` (kiểm tra CSRF của P1).
api() {
  local method=$1 path=$2 body=${3:-}
  if [ -n "$body" ]; then
    curl -sf -X "$method" -H 'X-MineHost: 1' -H 'Content-Type: application/json' \
      -d "$body" "$API$path"
  else
    curl -sf -X "$method" -H 'X-MineHost: 1' "$API$path"
  fi
}

wait_for() {
  local what=$1 limit=$2; shift 2
  local end=$((SECONDS + limit))
  until "$@"; do
    [ $SECONDS -lt $end ] || { say "hết giờ chờ: $what"; return 1; }
    sleep 5
  done
}

job_done() {
  local state
  state=$(api GET "/jobs/$1" | jq -r '.state')
  [ "$state" != running ] || return 1
  [ "$state" = succeeded ] || { say "Job $1 lỗi: $state"; exit 1; }
}

# `state` có thể là chuỗi hoặc object có `kind`, tùy phiên bản DTO.
STATE_JQ='(.state | if type == "object" then (.kind // .state) else . end)'

server_running() {
  [ "$(api GET "/servers/$1" | jq -r "$STATE_JQ")" = running ]
}

start_controller() {
  local bin=$1
  mkdir -p "$DIR/data"
  install -m 0755 "$bin" "$DIR/minehost"
  printf 'listen = "127.0.0.1:%s"\nopen_browser = false\n' "$PORT" > "$DIR/data/config.toml"
  # MemoryMax giữ cả nhóm (controller và mọi Java) trong một cgroup: hết RAM thì OOM chỉ chạm
  # vào nhóm này, không chạm dịch vụ khác trên máy. KillMode=mixed như unit trong ops.
  systemd-run --user --unit="$UNIT" --collect -p MemoryMax="$RAM_CAP" -p CPUWeight=20 \
    -p Nice=10 -p KillMode=mixed -p TimeoutStopSec=180 \
    "$DIR/minehost" --data-dir "$DIR/data" --no-browser --no-tray
  wait_for "controller trả lời" 60 api GET /health >/dev/null
}

create_servers() {
  local count=$1 i created id job
  for i in $(seq 1 "$count"); do
    created=$(api POST /servers "$(jq -nc --arg n "Chạy dài $i" --arg mc "$MC" \
      --argjson port $((GAME_PORT + i - 1)) --argjson max "$MEM_MAX" \
      '{name: $n, loader: "paper", minecraft: $mc, port: $port,
        memory_mb: {min: 512, max: $max}}')")
    id=$(jq -r .server_id <<<"$created")
    job=$(jq -r .job_id <<<"$created")
    wait_for "cài server $i" 900 job_done "$job"
    echo "$id" >> "$DIR/servers.txt"
    say "đã tạo server $i: $id (cổng $((GAME_PORT + i - 1)))"
  done
}

start_servers() {
  local id
  while read -r id; do
    api POST "/servers/$id/eula" '{"accept": true}' >/dev/null
    api POST "/servers/$id/actions/start" '{}' >/dev/null
    wait_for "server $id chạy" 600 server_running "$id"
    say "đã bật $id"
  done < "$DIR/servers.txt"
}

controller_pid() {
  systemctl --user show -p MainPID --value "$UNIT"
}

# Một dòng CSV: thời điểm, RSS controller (KiB), số server chạy, RAM cả nhóm, ổ trống, pid các Java.
sample() {
  local pid rss servers running pids group free_gb
  pid=$(controller_pid)
  rss=$(awk '/^VmRSS/ {print $2}' "/proc/$pid/status" 2>/dev/null || echo 0)
  servers=$(api GET /servers || echo '[]')
  running=$(jq "[.[] | select($STATE_JQ == \"running\")] | length" <<<"$servers")
  pids=$(jq -r '[.[] | (.pid // 0)] | join(" ")' <<<"$servers")
  group=$(systemctl --user show -p MemoryCurrent --value "$UNIT")
  free_gb=$(df -BG --output=avail "$DIR" | tail -1 | tr -dc 0-9)
  echo "$(date -Is),$rss,$running,$group,$free_gb,$pids" >> "$LOG"
  [ "$free_gb" -ge "$MIN_DISK_GB" ] || { say "ổ chỉ còn ${free_gb} GB, dừng"; return 1; }
}

monitor() {
  local hours=$1 end
  end=$(( $(date +%s) + hours * 3600 ))
  while [ "$(date +%s)" -lt "$end" ]; do
    sample || { quit; return; }
    sleep 600
  done
  say "đủ $hours giờ"
  quit
}

quit() {
  api POST /app/shutdown '{}' >/dev/null || true
  wait_for "controller thoát" 180 bash -c "! systemctl --user is-active -q $UNIT" || true
  sample || true
  say "đã dừng; log controller: journalctl --user -u $UNIT"
}

case ${1:-} in
  start)
    bin=${2:?cần đường dẫn binary minehost}
    count=${3:-10}
    hours=${4:-72}
    [ ! -e "$DIR/servers.txt" ] || { echo "$DIR đã có lần chạy trước, xóa đi hoặc đổi MH_DIR"; exit 1; }
    mkdir -p "$DIR"
    echo "thoi_diem,rss_controller_kib,so_server_chay,ram_ca_nhom_byte,o_trong_gb,pid_java" > "$LOG"
    start_controller "$bin"
    create_servers "$count"
    start_servers
    sample
    systemd-run --user --unit="$UNIT-ghi" --collect \
      -E MH_DIR="$DIR" -E MH_PORT="$PORT" -E MH_MIN_DISK_GB="$MIN_DISK_GB" \
      bash "$(realpath "$0")" monitor "$hours"
    say "đang chạy $count server trong $hours giờ; kết quả ở $LOG"
    ;;
  monitor) monitor "${2:-72}" ;;
  status) tail -5 "$LOG"; tail -5 "$DIR/su-kien.log" ;;
  stop)
    systemctl --user stop "$UNIT-ghi" 2>/dev/null || true
    quit
    ;;
  *) sed -n 2,12p "$0"; exit 2 ;;
esac
