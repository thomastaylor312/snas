#!/usr/bin/env bash
set -euo pipefail

echo "==> Starting SNAS PAM e2e"

NATS_HOST="${SNAS_NATS_SERVER:-127.0.0.1}"
NATS_PORT="${SNAS_NATS_PORT:-4222}"
SOCKET_PATH="${SNAS_PAM_SOCKET_PATH:-/run/snas.sock}"
KV_BUCKET="snas-e2e-$(date +%s)-$RANDOM"

echo "Waiting for NATS at ${NATS_HOST}:${NATS_PORT}..."
for i in {1..60}; do
  if (echo -n > /dev/tcp/${NATS_HOST}/${NATS_PORT}) >/dev/null 2>&1; then
    break
  fi
  sleep 1
done

if ! (echo -n > /dev/tcp/${NATS_HOST}/${NATS_PORT}) >/dev/null 2>&1; then
  echo "NATS not reachable" >&2
  exit 1
fi

echo "Creating local users..."
useradd -m -s /bin/bash foo || true
useradd -m -s /bin/bash bar || true

echo "Launching snas-server..."
setsid snas-server \
  --nats-server "${NATS_HOST}" \
  --nats-port "${NATS_PORT}" \
  --kv-bucket "${KV_BUCKET}" \
  --admin-nats \
  --user-socket \
  --socket-file "${SOCKET_PATH}" \
  >/var/log/snas.log 2>&1 &

for i in {1..30}; do
  if [ -S "${SOCKET_PATH}" ]; then
    break
  fi
  sleep 0.5
done
ls -l "${SOCKET_PATH}" || (echo "Socket not created" && exit 1)

echo "Seeding SNAS users via CLI..."
SNAS_CLI=(snas --nats-server "${NATS_HOST}" --nats-port "${NATS_PORT}")
"${SNAS_CLI[@]}" admin add-user \
  --username foo --password supersecure --group testers
"${SNAS_CLI[@]}" admin add-user \
  --username bar --password temp123 --force-reset

echo "Test 1: Successful auth for foo"
/usr/local/bin/pam_test snas foo supersecure

echo "Test 2: Failed auth for foo with wrong password (expect failure)"
set +e
/usr/local/bin/pam_test snas foo wrongpass
rc=$?
set -e
if [ $rc -eq 0 ]; then
  echo "Expected failure but got success" >&2
  exit 1
fi

echo "Test 3: bar requires password change then succeeds"
/usr/local/bin/pam_test snas bar temp123 newpass newpass

echo "All PAM e2e tests passed"
