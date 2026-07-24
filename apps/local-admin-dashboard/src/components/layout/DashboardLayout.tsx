import { Suspense, useEffect, useState } from "react";
import { Outlet } from "react-router-dom";
import { Sidebar } from "./Sidebar";
import { Header } from "./Header";
import { Breadcrumbs } from "./Breadcrumbs";
import { FirstRunWizard } from "../FirstRunWizard";
import { CommandPalette } from "../command/CommandPalette";

export function DashboardLayout() {
  const [mobileMenuOpen, setMobileMenuOpen] = useState(false);
  const [commandPaletteOpen, setCommandPaletteOpen] = useState(false);

  useEffect(() => {
    const down = (event: KeyboardEvent) => {
      if (event.key === "k" && (event.metaKey || event.ctrlKey)) {
        event.preventDefault();
        setCommandPaletteOpen(true);
      }
    };
    document.addEventListener("keydown", down);
    return () => document.removeEventListener("keydown", down);
  }, []);

  return (
    <div className="flex h-screen w-full bg-background overflow-hidden">
      <FirstRunWizard />
      <Sidebar
        mobileMenuOpen={mobileMenuOpen}
        setMobileMenuOpen={setMobileMenuOpen}
      />
      <div className="flex flex-1 flex-col overflow-hidden relative">
        <Header
          toggleMobileMenu={() => setMobileMenuOpen(!mobileMenuOpen)}
          onOpenCommandPalette={() => setCommandPaletteOpen(true)}
        />
        <main className="relative z-10 flex-1 overflow-y-auto p-4 md:p-6">
          <div className="mx-auto w-full max-w-[1600px] [&>div:first-of-type]:pt-0">
            <Breadcrumbs />
            <Suspense
              fallback={
                <div
                  className="flex items-center justify-center py-24 text-sm text-muted-foreground"
                  role="status"
                  aria-live="polite"
                >
                  Loading…
                </div>
              }
            >
              <Outlet />
            </Suspense>
          </div>
        </main>
        <CommandPalette
          open={commandPaletteOpen}
          onClose={() => setCommandPaletteOpen(false)}
        />
      </div>
    </div>
  );
}
