import {
  Activity,
  Cpu,
  Database,
  FileKey,
  LayoutDashboard,
  Lightbulb,
  Network,
  Puzzle,
  Route,
  Search,
  Server,
  Settings,
  ShieldAlert,
  ShieldCheck,
  SlidersHorizontal,
  Users,
  Wrench,
  Zap,
} from "lucide-react";

export interface NavItem {
  id: string;
  en: string;
  th: string;
  href: string;
  icon: any;
  modes: string[];
  primary?: boolean;
}

export interface NavGroup {
  id: string;
  en: string;
  th: string;
  items: NavItem[];
}

const ALL: string[] = ["desktop_simple", "desktop_advanced", "enterprise"];
const ADV: string[] = ["desktop_advanced", "enterprise"];
const ENT: string[] = ["enterprise"];

export const NAV: NavGroup[] = [
  {
    id: "command-center",
    en: "Command Center",
    th: "Command Center",
    items: [
      {
        id: "home",
        en: "Overview",
        th: "Overview",
        href: "/",
        icon: LayoutDashboard,
        modes: ALL,
      },
      {
        id: "entity-graph",
        en: "Relationship Map",
        th: "Relationship Map",
        href: "/entity-graph",
        icon: Network,
        modes: ALL,
        primary: true,
      },
      {
        id: "activity-timeline",
        en: "Activity Timeline",
        th: "Activity Timeline",
        href: "/activity-timeline",
        icon: Activity,
        modes: ALL,
      },
    ],
  },
  {
    id: "discover",
    en: "Discover",
    th: "Discover",
    items: [
      {
        id: "scan",
        en: "Scan & Protect",
        th: "Scan & Protect",
        href: "/protect",
        icon: ShieldCheck,
        modes: ALL,
      },
      {
        id: "discovery",
        en: "Auto Discovery",
        th: "Auto Discovery",
        href: "/discovery",
        icon: Search,
        modes: ALL,
      },
      {
        id: "agents",
        en: "Agents & Models",
        th: "Agents & Models",
        href: "/agents",
        icon: Users,
        modes: ALL,
      },
      {
        id: "resources",
        en: "Data Resources",
        th: "Data Resources",
        href: "/resources",
        icon: Database,
        modes: ALL,
      },
      {
        id: "tools",
        en: "Tools",
        th: "Tools",
        href: "/tools",
        icon: Wrench,
        modes: ADV,
      },
      {
        id: "identities",
        en: "Identities",
        th: "Identities",
        href: "/identities",
        icon: Network,
        modes: ADV,
      },
    ],
  },
  {
    id: "govern",
    en: "Govern",
    th: "Govern",
    items: [
      {
        id: "policies",
        en: "Policies",
        th: "Policies",
        href: "/policies",
        icon: FileKey,
        modes: ALL,
      },
      {
        id: "suggestions",
        en: "Policy Suggestions",
        th: "Policy Suggestions",
        href: "/policy-suggestions",
        icon: Lightbulb,
        modes: ALL,
      },
      {
        id: "presets",
        en: "Policy Presets",
        th: "Policy Presets",
        href: "/policy-presets",
        icon: ShieldCheck,
        modes: ADV,
      },
      {
        id: "simulator",
        en: "Simulator",
        th: "Simulator",
        href: "/simulator",
        icon: Activity,
        modes: ADV,
      },
    ],
  },
  {
    id: "protect",
    en: "Protect",
    th: "Protect",
    items: [
      {
        id: "policy-feasibility",
        en: "Policy Feasibility",
        th: "Policy Feasibility",
        href: "/policy-feasibility",
        icon: ShieldCheck,
        modes: ALL,
      },
      {
        id: "deployments",
        en: "Deployments",
        th: "Deployments",
        href: "/deployments",
        icon: Server,
        modes: ALL,
      },
      {
        id: "control-methods",
        en: "Control Methods",
        th: "Control Methods",
        href: "/control-methods",
        icon: SlidersHorizontal,
        modes: ADV,
      },
      {
        id: "pep-layers",
        en: "PEP Layers",
        th: "PEP Layers",
        href: "/pep-layers",
        icon: ShieldAlert,
        modes: ENT,
      },
      {
        id: "pdp-engines",
        en: "PDP Engines",
        th: "PDP Engines",
        href: "/settings/pdp",
        icon: Route,
        modes: ENT,
      },
    ],
  },
  {
    id: "observe",
    en: "Observe",
    th: "Observe",
    items: [
      {
        id: "alerts",
        en: "Alerts & Shadow AI",
        th: "Alerts & Shadow AI",
        href: "/alerts",
        icon: ShieldAlert,
        modes: ALL,
      },
      {
        id: "cost",
        en: "Cost & Tokens",
        th: "Cost & Tokens",
        href: "/cost-ledger",
        icon: Zap,
        modes: ALL,
      },
      {
        id: "local-evidence",
        en: "Local Evidence",
        th: "Local Evidence",
        href: "/local-evidence",
        icon: Database,
        modes: ALL,
      },
      {
        id: "health",
        en: "Health & Diagnostics",
        th: "Health & Diagnostics",
        href: "/health",
        icon: Activity,
        modes: ADV,
      },
    ],
  },
  {
    id: "operations",
    en: "Operations",
    th: "Operations",
    items: [
      {
        id: "capabilities",
        en: "Capabilities",
        th: "Capabilities",
        href: "/capabilities",
        icon: Cpu,
        modes: ALL,
      },
      {
        id: "integrations",
        en: "Integrations",
        th: "Integrations",
        href: "/integrations",
        icon: Wrench,
        modes: ADV,
      },
      {
        id: "plugins",
        en: "Plugin Marketplace",
        th: "Plugin Marketplace",
        href: "/plugin-marketplace",
        icon: Puzzle,
        modes: ADV,
      },
      {
        id: "bundles",
        en: "Bundles & Sync",
        th: "Bundles & Sync",
        href: "/bundles",
        icon: Server,
        modes: ENT,
      },
      {
        id: "settings",
        en: "Global Settings",
        th: "Global Settings",
        href: "/settings",
        icon: Settings,
        modes: ALL,
      },
    ],
  },
];
