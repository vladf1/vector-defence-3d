#!/bin/bash
set -euo pipefail

cd "$(dirname "$0")"

if ! command -v npm >/dev/null 2>&1; then
  echo "npm was not found. Install Node.js, then run this launcher again."
  read -r -p "Press Return to close this window..."
  exit 1
fi

if [ ! -d "node_modules" ]; then
  echo "Installing dependencies..."
  npm install
fi

log_file="$(mktemp -t vector-defence-vite.XXXXXX.log)"

cleanup() {
  if [ -n "${server_pid:-}" ] && kill -0 "$server_pid" >/dev/null 2>&1; then
    kill "$server_pid" >/dev/null 2>&1 || true
    wait "$server_pid" >/dev/null 2>&1 || true
  fi
  rm -f "$log_file"
}
trap cleanup EXIT INT TERM

echo "Starting Vector Defence..."
npm run dev -- --host 127.0.0.1 >"$log_file" 2>&1 &
server_pid=$!

url=""
for _ in {1..120}; do
  if ! kill -0 "$server_pid" >/dev/null 2>&1; then
    echo "The dev server stopped before it was ready:"
    cat "$log_file"
    read -r -p "Press Return to close this window..."
    exit 1
  fi

  url="$(grep -Eo 'http://127\.0\.0\.1:[0-9]+/' "$log_file" | head -n 1 || true)"
  if [ -n "$url" ]; then
    break
  fi

  sleep 0.25
done

if [ -z "$url" ]; then
  echo "Timed out waiting for the dev server to publish a local URL:"
  cat "$log_file"
  read -r -p "Press Return to close this window..."
  exit 1
fi

echo "Opening $url"
if [ "${VECTOR_DEFENCE_SKIP_OPEN:-}" != "1" ]; then
  open "$url"
fi

echo
echo "Vector Defence is running at $url"
echo "Close this Terminal window or press Control-C here to stop the server."

wait "$server_pid"
