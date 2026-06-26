import {
  Activity,
  Bot,
  BrainCircuit,
  Database,
  FileKey,
  IdCard,
  Server,
  Wrench,
  type LucideIcon,
} from "lucide-react";
import type { GraphNode } from "./types";

export function entityIcon(type: string): LucideIcon {
  if (type === "agent") return Bot;
  if (type === "tool") return Wrench;
  if (type === "resource") return Database;
  if (type === "policy") return FileKey;
  if (type === "identity") return IdCard;
  if (type === "provider") return Server;
  if (type === "model") return BrainCircuit;
  return Activity;
}

export function entityRoute(node: GraphNode) {
  if (node.href) return node.href;
  const selected = encodeURIComponent(node.entity_id);
  if (node.type === "agent") return `/agents?selected=${selected}`;
  if (node.type === "tool") return `/tools?selected=${selected}`;
  if (node.type === "resource") return `/resources?selected=${selected}`;
  if (node.type === "policy") return `/policies?selected=${selected}`;
  if (node.type === "identity") return `/identities?selected=${selected}`;
  return `/entity-graph?selected=${selected}`;
}

export function toneForStatus(status?: string | null) {
  const normalized = (status || "").toLowerCase();
  if (["active", "registered", "enforce", "protected", "ok"].includes(normalized)) {
    return "success";
  }
  if (["deny", "denied", "blocked", "failed", "critical", "restricted"].includes(normalized)) {
    return "danger";
  }
  if (["warn", "warning", "medium", "confidential", "degraded"].includes(normalized)) {
    return "warning";
  }
  if (["observed", "observe", "shadow", "draft"].includes(normalized)) {
    return "info";
  }
  return "neutral";
}

export function labelForMode(mode?: string | null) {
  const value = (mode || "observe").toLowerCase();
  if (value === "enforce") return "Enforce";
  if (value === "approval" || value === "ask") return "Approval";
  if (value === "warn") return "Warn";
  if (value === "govern") return "Govern";
  return "Observe";
}

export function formatNumber(value?: number | null) {
  return new Intl.NumberFormat().format(value || 0);
}

export function formatMoney(value?: number | null) {
  return new Intl.NumberFormat(undefined, {
    style: "currency",
    currency: "USD",
    maximumFractionDigits: (value || 0) < 1 ? 4 : 2,
  }).format(value || 0);
}
