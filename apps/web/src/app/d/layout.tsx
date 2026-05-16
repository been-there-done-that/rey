"use client";

import { useEffect } from "react";
import { AppSidebar } from "@/components/app-sidebar";
import { SiteHeader } from "@/components/site-header";
import { ProtectedRoute } from "@/components/protected-route";
import { SidebarInset, SidebarProvider } from "@/components/ui/sidebar";
import { TooltipProvider } from "@/components/ui/tooltip";
import { useFFmpegStore } from "@/lib/ffmpeg-store";

export default function DashboardLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  const loadFFmpeg = useFFmpegStore((s) => s.load);

  useEffect(() => {
    loadFFmpeg().catch(() => {});
  }, [loadFFmpeg]);

  return (
    <ProtectedRoute>
      <TooltipProvider>
        <SidebarProvider
          style={
            {
              "--sidebar-width": "calc(var(--spacing) * 72)",
              "--header-height": "calc(var(--spacing) * 12)",
            } as React.CSSProperties
          }
        >
          <AppSidebar variant="inset" />
          <SidebarInset className="overflow-hidden border md:peer-data-[variant=inset]:border-border">
            <div className="flex flex-col h-full">
              <SiteHeader />
              <div className="flex flex-1 flex-col overflow-y-auto">
                {children}
              </div>
            </div>
          </SidebarInset>
        </SidebarProvider>
      </TooltipProvider>
    </ProtectedRoute>
  );
}
