#!/bin/bash
echo -e "\033[0;33mStopping Pollen DEK Local Control Plane...\033[0m"
if pkill -f local-control-plane; then
    echo -e "\033[0;32mStopped successfully.\033[0m"
else
    echo -e "\033[0;37mNot running.\033[0m"
fi
