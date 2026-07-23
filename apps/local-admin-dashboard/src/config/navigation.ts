import {
  Activity,
  Bot,
  Cpu,
  Database,
  FileKey,
  FolderSearch,
  FolderTree,
  History,
  LayoutDashboard,
  ListChecks,
  Network,
  Puzzle,
  Route,
  ScanSearch,
  Server,
  Settings,
  ShieldAlert,
  ShieldCheck,
  SlidersHorizontal,
  Users,
  Workflow,
  Wrench,
  Zap,
} from "lucide-react";
import type { ComponentType } from "react";
import type { AppMode } from "@/lib/modes";

export interface NavItem {
  id: string;
  en: string;
  th: string;
  href: string;
  icon: ComponentType<{ className?: string }>;
  modes: AppMode[];
  primary?: boolean;
}

export interface NavGroup {
  id: string;
  en: string;
  th: string;
  items: NavItem[];
}

const ALL: AppMode[] = [
  "desktop_simple",
  "desktop_advanced",
  "enterprise_cloud",
];
const ADV: AppMode[] = ["desktop_advanced", "enterprise_cloud"];
const ENT: AppMode[] = ["enterprise_cloud"];

export const NAV: NavGroup[] = [
  {
    id: "home",
    en: "Home",
    th: "หน้าหลัก",
    items: [
      {
        id: "overview",
        en: "Overview",
        th: "ภาพรวม",
        href: "/",
        icon: LayoutDashboard,
        modes: ALL,
        primary: true,
      },
    ],
  },
  {
    id: "activity",
    en: "AI Activity",
    th: "กิจกรรม AI",
    items: [
      {
        id: "scan",
        en: "Find AI Apps",
        th: "ค้นหา AI Apps",
        href: "/scan",
        icon: ScanSearch,
        modes: ALL,
      },
      {
        id: "my-ai-apps",
        en: "My AI Apps",
        th: "AI Apps ของฉัน",
        href: "/my-ai-apps",
        icon: Bot,
        modes: ALL,
      },
      {
        id: "ai-activity",
        en: "AI Activity",
        th: "กิจกรรม AI",
        href: "/activity",
        icon: Activity,
        modes: ALL,
      },
      {
        id: "observe-coverage",
        en: "Observe Coverage",
        th: "Observe Coverage",
        href: "/observe-coverage",
        icon: ShieldCheck,
        modes: ALL,
      },
      {
        id: "signal-correlation",
        en: "Signal Correlation",
        th: "การเชื่อมโยงสัญญาณ",
        href: "/signal-correlation",
        icon: Workflow,
        modes: ALL,
      },
      {
        id: "prompt-guard",
        en: "Prompt Guard",
        th: "Prompt Guard",
        href: "/alerts",
        icon: ShieldAlert,
        modes: ALL,
      },
      {
        id: "create-rule",
        en: "Create Rule",
        th: "สร้างกฎ",
        href: "/protect",
        icon: ShieldCheck,
        modes: ALL,
      },
      {
        id: "allowed-blocked",
        en: "Allowed & Blocked",
        th: "อนุญาตและบล็อก",
        href: "/allowed-blocked",
        icon: ListChecks,
        modes: ALL,
      },
      {
        id: "data-apps",
        en: "Data & Apps",
        th: "ไฟล์ เว็บไซต์ และแอป",
        href: "/data-apps",
        icon: Database,
        modes: ALL,
      },
      {
        id: "cost",
        en: "AI Usage & Cost",
        th: "การใช้งานและค่าใช้จ่าย AI",
        href: "/cost-ledger",
        icon: Zap,
        modes: ALL,
      },
      {
        id: "setup",
        en: "Setup",
        th: "ตั้งค่า",
        href: "/setup",
        icon: Wrench,
        modes: ALL,
      },
      {
        id: "history",
        en: "History",
        th: "ประวัติย้อนหลัง",
        href: "/history",
        icon: History,
        modes: ALL,
      },
    ],
  },
  {
    id: "registry",
    en: "Registry",
    th: "ทะเบียน",
    items: [
      {
        id: "agents",
        en: "Agents & Models",
        th: "Agents และ Models",
        href: "/agents",
        icon: Bot,
        modes: ADV,
      },
      {
        id: "tools-resources",
        en: "Tools & Resources",
        th: "เครื่องมือและทรัพยากร",
        href: "/tools",
        icon: FolderTree,
        modes: ADV,
      },
      {
        id: "identities",
        en: "Identities",
        th: "ตัวตน",
        href: "/identities",
        icon: Users,
        modes: ADV,
      },
      {
        id: "entity-graph",
        en: "Relationship Map",
        th: "แผนผังความสัมพันธ์",
        href: "/entity-graph",
        icon: Network,
        modes: ADV,
      },
    ],
  },
  {
    id: "governance",
    en: "Governance",
    th: "การกำกับดูแล",
    items: [
      {
        id: "policies",
        en: "Policies",
        th: "นโยบาย",
        href: "/policies",
        icon: FileKey,
        modes: ADV,
      },
      {
        id: "policy-presets",
        en: "Policy Presets",
        th: "ชุดกฎสำเร็จรูป",
        href: "/policy-presets",
        icon: ShieldCheck,
        modes: ADV,
      },
      {
        id: "deployments",
        en: "Deployments",
        th: "การปรับใช้กฎ",
        href: "/deployments",
        icon: SlidersHorizontal,
        modes: ADV,
      },
      {
        id: "simulator",
        en: "Simulator",
        th: "จำลองสถานการณ์",
        href: "/simulator",
        icon: Route,
        modes: ADV,
      },
    ],
  },
  {
    id: "observe",
    en: "Advanced Observe",
    th: "การสังเกตขั้นสูง",
    items: [
      {
        id: "activity-timeline",
        en: "Activity Timeline",
        th: "ไทม์ไลน์กิจกรรม",
        href: "/activity-timeline",
        icon: Activity,
        modes: ADV,
      },
      {
        id: "health",
        en: "Health",
        th: "สุขภาพระบบ",
        href: "/health",
        icon: Cpu,
        modes: ADV,
      },
    ],
  },
  {
    id: "system",
    en: "System",
    th: "ระบบ",
    items: [
      {
        id: "capabilities",
        en: "Capabilities",
        th: "ความสามารถของระบบ",
        href: "/capabilities",
        icon: FolderSearch,
        modes: ADV,
      },
      {
        id: "integrations",
        en: "Integrations",
        th: "การเชื่อมต่อ",
        href: "/integrations",
        icon: Puzzle,
        modes: ADV,
      },
      {
        id: "plugin-marketplace",
        en: "Plugins",
        th: "ปลั๊กอิน",
        href: "/plugin-marketplace",
        icon: Puzzle,
        modes: ALL,
      },
      {
        id: "bundles",
        en: "Bundles & Sync",
        th: "แพ็กเกจและซิงก์",
        href: "/bundles",
        icon: Server,
        modes: ENT,
      },
      {
        id: "settings",
        en: "Settings",
        th: "ตั้งค่า",
        href: "/settings",
        icon: Settings,
        modes: ALL,
      },
    ],
  },
];

export function labelForLanguage(
  item: Pick<NavGroup | NavItem, "en" | "th">,
  language: string,
) {
  return language === "th" ? item.th : item.en;
}

export const NAV_ITEMS = NAV.flatMap((group) => group.items);
