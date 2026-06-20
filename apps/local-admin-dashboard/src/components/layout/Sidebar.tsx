import { Link, useLocation } from "react-router-dom";
import { cn } from "@/lib/utils";
import {
  LayoutDashboard,
  Server,
  Network,
  Wrench,
  ShieldAlert,
  Activity,
  FileKey,
  Users,
  Database,
  Cpu,
  Settings as SettingsIcon,
  UserCircle,
  Search,
  Lightbulb
} from "lucide-react";
import { useTranslation } from "react-i18next";

const navigation = [
  { name: "Overview", href: "/", icon: LayoutDashboard },
  { name: "Agents", href: "/agents", icon: Users },
  { name: "Blackbox AI", href: "/blackbox-ai", icon: Cpu },
  { name: "MCP Servers", href: "/servers", icon: Server },
  { name: "Tools", href: "/tools", icon: Wrench },
  { name: "Resources", href: "/resources", icon: Database },
  { name: "Entities", href: "/entities", icon: UserCircle },
  { name: "Relationships", href: "/relationships", icon: Network },
  { name: "Policy Enforcer", href: "/policies", icon: FileKey },
  { name: "Simulator", href: "/simulator", icon: Activity },
  { name: "Bundles & Deployments", href: "/bundles", icon: Server },
  { name: "Audit & Telemetry", href: "/audit", icon: Activity },
  { name: "Alerts", href: "/alerts", icon: ShieldAlert },
  { name: "Settings", href: "/settings", icon: SettingsIcon },
];

const observationNav = [
  { icon: Search, name: "Auto Discovery", href: "/observation/discovery" },
  { icon: ShieldAlert, name: "Shadow AI Inbox", href: "/observation/shadow" },
  { icon: Lightbulb, name: "Policy Suggestions", href: "/observation/suggestions" },
  { icon: Activity, name: "Token & Cost Ledger", href: "/observation/cost" },
];

export function Sidebar() {
  const location = useLocation();
  const { t } = useTranslation();

  return (
    <div className="flex h-full w-64 flex-col border-r bg-card/50 backdrop-blur-xl">
      <div className="flex h-16 items-center border-b px-6">
        <h1 className="text-xl font-bold bg-gradient-to-r from-primary to-accent bg-clip-text text-transparent">
          Pollen DEK
        </h1>
        <span className="ml-2 rounded-md bg-primary/10 px-2 py-0.5 text-xs font-medium text-primary">
          v2
        </span>
      </div>
      <div className="flex-1 overflow-y-auto py-4">
        <nav className="space-y-1 px-3">
          {navigation.map((item) => {
            const isActive = location.pathname === item.href || (item.href !== "/" && location.pathname.startsWith(item.href));
            return (
              <Link
                key={item.name}
                to={item.href}
                className={cn(
                  isActive
                    ? "bg-primary/10 text-primary font-semibold"
                    : "text-muted-foreground hover:bg-muted/50 hover:text-foreground",
                  "group flex items-center rounded-md px-3 py-2 text-sm font-medium transition-all duration-200"
                )}
              >
                <item.icon
                  className={cn(
                    isActive ? "text-primary" : "text-muted-foreground group-hover:text-foreground",
                    "mr-3 h-5 w-5 flex-shrink-0 transition-colors"
                  )}
                  aria-hidden="true"
                />
                {t(item.name)}
              </Link>
            );
          })}
        </nav>
      </div>

      <div className="flex-1 overflow-y-auto py-4 border-t">
        <h3 className="px-6 text-xs font-semibold uppercase tracking-wider text-muted-foreground mb-2">Observation & Discovery</h3>
        <nav className="space-y-1 px-3">
          {observationNav.map((item) => {
            const isActive = location.pathname === item.href || (item.href !== "/" && location.pathname.startsWith(item.href));
            return (
              <Link
                key={item.name}
                to={item.href}
                className={cn(
                  isActive
                    ? "bg-primary/10 text-primary font-semibold"
                    : "text-muted-foreground hover:bg-muted/50 hover:text-foreground",
                  "group flex items-center rounded-md px-3 py-2 text-sm font-medium transition-all duration-200"
                )}
              >
                <item.icon
                  className={cn(
                    isActive ? "text-primary" : "text-muted-foreground group-hover:text-foreground",
                    "mr-3 h-5 w-5 flex-shrink-0 transition-colors"
                  )}
                  aria-hidden="true"
                />
                {t(item.name)}
              </Link>
            );
          })}
        </nav>
      </div>

      <div className="border-t p-4">
        <div className="flex items-center gap-3 rounded-lg bg-muted/50 p-3">
          <div className="h-8 w-8 rounded-full bg-primary/20 flex items-center justify-center">
            <Users className="h-4 w-4 text-primary" />
          </div>
          <div className="flex flex-col">
            <span className="text-sm font-medium">Local Admin</span>
            <span className="text-xs text-muted-foreground">tenant: local</span>
          </div>
        </div>
      </div>
    </div>
  );
}
