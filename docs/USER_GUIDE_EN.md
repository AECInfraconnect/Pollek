# Pollek Local Enforcement Kit User Guide

## Overview

Pollek Local Enforcement Kit (Distributed Enforcement Kernel) is an endpoint security and policy enforcement tool.

## Key Components

- **Pollek Local Enforcement Kit Core (`Pollek-Local Enforcement Kit`)**: The background service that manages identity, downloads policies, and controls enforcement.
- **Pollek Local Enforcement Kit CLI (`Pollek-dekctl`)**: Command-line tool to enroll, manage, and troubleshoot the Local Enforcement Kit.
- **Pollek MCP Proxy (`Pollek-mcp-proxy`)**: A local proxy for Model Context Protocol (MCP) tool usage, authorizing requests before they reach the tools.

## Local Admin Dashboard Features

The Local Admin Dashboard (accessible at `http://127.0.0.1:3000` when running the Local Control Plane) provides several powerful features for managing your local Local Enforcement Kit instance:

### 1. Dry-run Simulator

Test your draft or active policies without affecting live traffic.

- Navigate to **Simulator**.
- Enter your subject, action, resource, and context as JSON.
- Specify an expected decision to test for regressions.
- Click **Run Simulation** to see the actual decision and blast-radius compared to active policies.

### 2. Audit Export

Download your decision logs for external analysis or compliance reporting.

- Navigate to **Decision Logs**.
- Click **Export CSV** or **Export JSON** to download the logs currently stored in your local telemetry database.

### 3. Connector Configuration

Configure and verify connections to external Policy Decision Points (PDPs) like OPA, OpenFGA, and Cedar.

- Navigate to **Settings**.
- Add a new connector configuration.
- Click **Test Connection** to immediately verify if the endpoint is reachable from the Local Control Plane.

## Configuration

Configuration is located at `~/.Pollek/Local Enforcement Kit/` by default during the beta phase, utilizing `bootstrap.json`.

## Logs

Logs can be viewed using the `Pollek-dekctl logs` command, or found in the `~/.Pollek/Local Enforcement Kit/logs/` directory.
