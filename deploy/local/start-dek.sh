#!/bin/bash
echo -e "\033[0;36mStarting Pollen DEK Local Control Plane...\033[0m"

if [ ! -d "apps/local-admin-dashboard/dist" ]; then
    echo -e "\033[0;33mBuilding Local Admin Dashboard for the first time...\033[0m"
    cd apps/local-admin-dashboard
    npm install
    npm run build
    cd ../..
fi

echo -e "\033[0;33mCompiling the Local Control Plane...\033[0m"
cargo build -p local-control-plane --release

pkill -f local-control-plane || true

echo -e "\033[0;33mStarting Local Control Plane in background...\033[0m"
export DEK_LCP_AUTH_DISABLE=1
nohup ./target/release/local-control-plane > /dev/null 2>&1 &

echo "Waiting for server to start..."
sleep 2
echo -e "\033[0;32mOpening Dashboard at http://127.0.0.1:43891\033[0m"

if command -v open > /dev/null; then
    open "http://127.0.0.1:43891"
elif command -v xdg-open > /dev/null; then
    xdg-open "http://127.0.0.1:43891"
else
  echo "Please open http://127.0.0.1:43891 in your browser."
fi

echo -e "\033[0;36mDone! The Local Control Plane is now running silently in the background.\033[0m"
echo -e "\033[0;37mTo stop it, run: ./stop-dek.sh\033[0m"
