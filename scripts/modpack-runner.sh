#!/usr/bin/env bash
# Cài, gỡ hoặc xem trạng thái GitHub Actions runner tự host cho workflow `modpacks`
# (spec P3 mục 3.11, hướng dẫn: docs/ops/runner-modpack.md). Chạy bằng root.
#
#   sudo bash scripts/modpack-runner.sh install <file-chứa-registration-token>
#   sudo bash scripts/modpack-runner.sh uninstall <file-chứa-removal-token>
#   sudo bash scripts/modpack-runner.sh update   # viết lại unit sau khi sửa script
#   sudo bash scripts/modpack-runner.sh status
#
# Token đọc từ file (không qua tham số dòng lệnh để không lộ trong `ps`), đọc xong thì xóa file.
set -euo pipefail

RUNNER_VERSION=2.338.2
RUNNER_SHA256=baebf204307bac6193480852732f0aba2ae97b6c5597c244f4d5558390147c40
RUNNER_USER=mh-runner
RUNNER_HOME=/srv/mh-runner
REPO_URL=https://github.com/Pharos107/MineHost
LABEL=minehost-modpack
UNIT_NAME=mh-runner.service
UNIT_PATH=/etc/systemd/system/$UNIT_NAME
NFT_DIR=/etc/mh-runner
NFT_PATH=$NFT_DIR/nftables.conf
# Cổng test trên loopback, khớp crates/minehost/tests/modpacks.rs: game, controller.
TEST_PORTS="25610, 25611"

die() { echo "lỗi: $*" >&2; exit 1; }

need_root() { [ "$(id -u)" -eq 0 ] || die "cần chạy bằng root (sudo)"; }

read_token() {
  local file=${1:-}
  [ -n "$file" ] && [ -s "$file" ] || die "thiếu file token"
  TOKEN=$(tr -d '[:space:]' < "$file")
  rm -f -- "$file"
  [ -n "$TOKEN" ] || die "file token rỗng"
}

# Gọi config.sh của runner, token đi qua biến môi trường chứ không qua argv (thấy được trong `ps`).
runner_config() {
  (
    cd "$RUNNER_HOME/runner"
    export ACTIONS_RUNNER_INPUT_TOKEN="$TOKEN" HOME="$RUNNER_HOME"
    runuser -p -u "$RUNNER_USER" -- ./config.sh "$@"
  )
}

# Loopback mở ra mọi dịch vụ chỉ nghe ở 127.0.0.1 (MineHost thật không cần mật khẩu khi gọi từ
# chính máy này). Kết nối mới của user runner qua loopback chỉ được tới DNS và cổng của test.
write_nft() {
  install -d -m 0755 "$NFT_DIR"
  cat > "$NFT_PATH" <<EOF
table inet mh_runner
delete table inet mh_runner
table inet mh_runner {
  chain output {
    type filter hook output priority 0; policy accept;
    meta skuid "$RUNNER_USER" oifname "lo" ct state established,related accept
    meta skuid "$RUNNER_USER" oifname "lo" udp dport 53 accept
    meta skuid "$RUNNER_USER" oifname "lo" tcp dport { 53, $TEST_PORTS } accept
    meta skuid "$RUNNER_USER" oifname "lo" counter reject
  }
}
EOF
  nft -c -f "$NFT_PATH" || die "luật nftables không hợp lệ"
}

write_unit() {
  local CPU_HALF=$(( $(nproc) / 2 ))
  [ "$CPU_HALF" -ge 1 ] || CPU_HALF=1
  cat > "$UNIT_PATH" <<EOF
[Unit]
Description=GitHub Actions runner cho workflow modpacks của MineHost
After=network-online.target
Wants=network-online.target

[Service]
User=$RUNNER_USER
Group=$RUNNER_USER
WorkingDirectory=$RUNNER_HOME/runner
ExecStart=$RUNNER_HOME/runner/run.sh
Restart=always
RestartSec=30
# Dấu + : chạy bằng root, ngoài sandbox của unit. Luật mạng có trước khi runner nhận job.
ExecStartPre=+/usr/sbin/nft -f $NFT_PATH
ExecStopPost=-+/usr/sbin/nft delete table inet mh_runner
# Giới hạn theo spec P3 mục 3.11: máy còn chạy server Minecraft thật và các dịch vụ khác.
MemoryMax=12G
# Không cho đẩy RAM của test sang swap (máy chỉ có vài GB swap, dùng chung với dịch vụ thật).
MemorySwapMax=0
CPUWeight=20
# Máy có daemon watchdog khởi động lại cả máy khi loadavg 1 phút vượt ngưỡng (24 trên máy 12
# luồng). Một modpack Forge lớn lúc khởi động đẩy load lên 28. Giới hạn ở nửa số nhân: Java thấy ít
# nhân hơn nên tạo ít luồng hơn, và phần CPU còn lại luôn để cho server thật.
AllowedCPUs=0-$((CPU_HALF - 1))
CPUQuota=$((CPU_HALF * 100))%
IOWeight=20
Nice=10
TasksMax=8192
# Cho MineHost tạo cgroup con giới hạn RAM từng server bên trong service này.
Delegate=yes
# Tắt runner thì dừng luôn mọi Java của test, không để sót process.
KillMode=control-group
TimeoutStopSec=300
# Code trong workflow không đọc được /home (dữ liệu thật của máy) và không ghi ra ngoài thư mục runner.
NoNewPrivileges=yes
ProtectSystem=strict
ProtectHome=yes
PrivateTmp=yes
ReadWritePaths=$RUNNER_HOME
# Không với tới mạng LAN, Docker, Tailscale; chỉ Internet và localhost (localhost còn bị lọc theo
# cổng bằng nftables, xem write_nft).
IPAddressAllow=localhost
IPAddressDeny=10.0.0.0/8 172.16.0.0/12 192.168.0.0/16 100.64.0.0/10 169.254.0.0/16 fc00::/7 fe80::/10

[Install]
WantedBy=multi-user.target
EOF
}

install_runner() {
  need_root
  read_token "${1:-}"
  [ "$(uname -m)" = x86_64 ] || die "chỉ hỗ trợ x86_64"
  if ! id "$RUNNER_USER" >/dev/null 2>&1; then
    useradd --system --create-home --home-dir "$RUNNER_HOME" --shell /usr/sbin/nologin "$RUNNER_USER"
  fi
  chmod 750 "$RUNNER_HOME"
  TMP_DIR=$(mktemp -d)
  trap 'rm -rf "$TMP_DIR"' EXIT
  local tmp=$TMP_DIR
  local tarball="actions-runner-linux-x64-$RUNNER_VERSION.tar.gz"
  curl -fsSL -o "$tmp/$tarball" \
    "https://github.com/actions/runner/releases/download/v$RUNNER_VERSION/$tarball"
  echo "$RUNNER_SHA256  $tmp/$tarball" | sha256sum -c --quiet || die "sai SHA-256 của runner"
  install -d -o "$RUNNER_USER" -g "$RUNNER_USER" "$RUNNER_HOME/runner"
  # Thư mục tạm của mktemp chỉ root đọc được, nên giải nén bằng root rồi giao lại cho user runner.
  tar -xzf "$tmp/$tarball" -C "$RUNNER_HOME/runner" --no-same-owner
  chown -R "$RUNNER_USER:$RUNNER_USER" "$RUNNER_HOME/runner"
  runner_config --unattended --replace --url "$REPO_URL" --name "$(hostname)-modpack" \
    --labels "$LABEL" --work _work
  write_nft
  write_unit
  systemctl daemon-reload
  systemctl enable --now "$UNIT_NAME"
  echo "đã cài runner $(hostname)-modpack, nhãn $LABEL"
}

uninstall_runner() {
  need_root
  read_token "${1:-}"
  if systemctl list-unit-files "$UNIT_NAME" >/dev/null 2>&1; then
    systemctl disable --now "$UNIT_NAME" || true
  fi
  if [ -x "$RUNNER_HOME/runner/config.sh" ]; then
    runner_config remove || \
      echo "cảnh báo: không gỡ được runner khỏi GitHub, gỡ tay ở Settings > Actions > Runners" >&2
  fi
  rm -f "$UNIT_PATH"
  nft delete table inet mh_runner 2>/dev/null || true
  rm -rf -- "$NFT_DIR"
  systemctl daemon-reload
  if id "$RUNNER_USER" >/dev/null 2>&1; then
    userdel "$RUNNER_USER"
  fi
  rm -rf -- "$RUNNER_HOME"
  echo "đã gỡ runner và xóa $RUNNER_HOME"
}

# Viết lại unit và luật mạng theo bản script hiện tại, không cần token. Khởi động lại runner, nên
# job đang chạy (nếu có) bị hủy.
update_unit() {
  need_root
  [ -f "$UNIT_PATH" ] || die "chưa cài runner"
  write_nft
  write_unit
  systemctl daemon-reload
  systemctl restart "$UNIT_NAME"
  echo "đã cập nhật unit $UNIT_NAME"
}

case "${1:-}" in
  install) install_runner "${2:-}" ;;
  uninstall) uninstall_runner "${2:-}" ;;
  update) update_unit ;;
  status) systemctl status "$UNIT_NAME" --no-pager ;;
  *) die "dùng: $0 install|uninstall <file-token> | update | status" ;;
esac
