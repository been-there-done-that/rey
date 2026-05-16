"use client"

import { useRouter } from "next/navigation"
import { Separator } from "@/components/ui/separator"
import { SidebarTrigger } from "@/components/ui/sidebar"
import { Button } from "@/components/ui/button"
import { useAuth } from "@/lib/auth-store"
import { authApi } from "@/lib/api"

export function SiteHeader() {
  const router = useRouter()
  const token = useAuth((s) => s.token)
  const logout = useAuth((s) => s.logout)

  async function handleLogout() {
    if (token) {
      try {
        await authApi(token).post("api/auth/logout")
      } catch {
        // ignore errors during logout
      }
    }
    logout()
    router.push("/login")
  }

  return (
    <header className="flex h-(--header-height) shrink-0 items-center gap-2 border-b transition-[width,height] ease-linear group-has-data-[collapsible=icon]/sidebar-wrapper:h-(--header-height)">
      <div className="flex w-full items-center gap-1 px-4 lg:gap-2 lg:px-6">
        <SidebarTrigger className="-ml-1" />
        <Separator
          orientation="vertical"
          className="mx-2 h-4 data-vertical:self-auto"
        />
        <h1 className="text-base font-medium">Documents</h1>
        <div className="ml-auto">
          <Button variant="ghost" size="sm" onClick={handleLogout}>
            Logout
          </Button>
        </div>
      </div>
    </header>
  )
}
