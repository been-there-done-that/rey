"use client"

import { useEffect, useState } from "react"
import { useRouter, usePathname } from "next/navigation"
import { useAuth } from "@/lib/auth-store"
import { api } from "@/lib/api"

export function ProtectedRoute({ children }: { children: React.ReactNode }) {
  const router = useRouter()
  const pathname = usePathname()
  const token = useAuth((s) => s.token)
  const logout = useAuth((s) => s.logout)
  const [checking, setChecking] = useState(true)
  const [authorized, setAuthorized] = useState(false)

  useEffect(() => {
    if (!token) {
      setChecking(false)
      setAuthorized(false)
      router.replace(`/login?redirect=${encodeURIComponent(pathname)}`)
      return
    }

    let cancelled = false
    const controller = new AbortController()

    api.get("api/auth/me", { signal: controller.signal })
      .json()
      .then(() => {
        if (!cancelled) {
          setAuthorized(true)
          setChecking(false)
        }
      })
      .catch(() => {
        if (!cancelled) {
          logout()
          setChecking(false)
          router.replace(`/login?redirect=${encodeURIComponent(pathname)}`)
        }
      })

    return () => {
      cancelled = true
      controller.abort()
    }
  }, [token, pathname, router, logout])

  if (checking) {
    return (
      <div className="flex h-screen items-center justify-center">
        <div className="h-6 w-6 animate-spin rounded-full border-2 border-primary border-t-transparent" />
      </div>
    )
  }

  if (!authorized) {
    return null
  }

  return <>{children}</>
}
