#!/usr/bin/env bash
# sync-nas.sh — 同步 TagFlow 源码到 NAS 供 docker compose build。
#
# 白名单同步：只传编译必需的 tagflow-core / tagflow-ui / Dockerfile /
# .dockerignore / docker-compose.yml，排除一切产物 / 文档 / 开发工具 / 密钥 /
# 数据库；--delete-excluded 清理 NAS 上的无关残留。
#
# 用法：
#   ./scripts/sync-nas.sh             # 正式同步
#   ./scripts/sync-nas.sh --dry-run   # 预览（不实际传输/删除）
#
# 覆盖默认目标（可选）：
#   REMOTE=user@host DEST=/path ./scripts/sync-nas.sh

set -eo pipefail
cd "$(dirname "$0")/.."   # 切到仓库根，保证 ./ 指向仓库根

REMOTE="${REMOTE:-saye@fnos.pve.saye}"
DEST="${DEST:-/vol2/1000/docker/tagflow/src}"

DRY=()
case "${1:-}" in
  --dry-run|-n) DRY=(--dry-run); echo "[dry-run] 仅预览，不实际传输/删除" ;;
  "") ;;
  *) echo "用法: $0 [--dry-run]" >&2; exit 2 ;;
esac

echo "→ ${REMOTE}:${DEST}"
rsync -azv --delete --delete-excluded --no-o --no-g "${DRY[@]}" \
  --exclude 'target' --exclude 'node_modules' --exclude 'dist' \
  --exclude 'cache' --exclude '.sqlx' --exclude 'test_dir' \
  --exclude '.env' --exclude '*.db*' --exclude '.DS_Store' \
  --include 'tagflow-core/' --include 'tagflow-core/**' \
  --include 'tagflow-ui/' --include 'tagflow-ui/**' \
  --include 'Dockerfile' --include '.dockerignore' --include 'docker-compose.yml' \
  --exclude '*' \
  -e ssh \
  ./ "${REMOTE}:${DEST}"

echo "✓ 完成。NAS 构建：ssh ${REMOTE} 'cd ${DEST} && docker compose build'"
