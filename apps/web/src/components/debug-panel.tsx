"use client"

import { useEffect, useState } from "react"

type Status = "loading" | "ok" | "error"

interface Check {
  label: string
  status: Status
  detail: string
}

export function DebugPanel() {
  const [open, setOpen] = useState(false)
  const [checks, setChecks] = useState<Check[]>([
    { label: "WASM init", status: "loading", detail: "checking..." },
    { label: "generate_key_b64", status: "loading", detail: "checking..." },
    { label: "generate_salt_b64", status: "loading", detail: "checking..." },
    { label: "derive_kek_b64", status: "loading", detail: "checking..." },
    { label: "derive_verification_key_b64", status: "loading", detail: "checking..." },
    { label: "bcrypt_hash_b64", status: "loading", detail: "checking..." },
    { label: "encrypt_key_b64", status: "loading", detail: "checking..." },
    { label: "generate_keypair_b64", status: "loading", detail: "checking..." },
  ])

  useEffect(() => {
    async function runChecks() {
      const results: Check[] = []

      try {
        const wasm = await import("@/wasm-pkg/index")

        results.push({ label: "WASM init", status: "ok", detail: "loaded" })

        try {
          const key = wasm.generate_key_b64()
          results.push({ label: "generate_key_b64", status: "ok", detail: key.slice(0, 16) + "..." })
        } catch (e) {
          results.push({ label: "generate_key_b64", status: "error", detail: String(e) })
        }

        try {
          const salt = wasm.generate_salt_b64()
          results.push({ label: "generate_salt_b64", status: "ok", detail: salt })
        } catch (e) {
          results.push({ label: "generate_salt_b64", status: "error", detail: String(e) })
        }

        try {
          const salt = wasm.generate_salt_b64()
          const kek = wasm.derive_kek_b64("test", salt, 67108864, 2)
          results.push({ label: "derive_kek_b64", status: "ok", detail: kek.slice(0, 16) + "..." })
        } catch (e) {
          results.push({ label: "derive_kek_b64", status: "error", detail: String(e) })
        }

        try {
          const kek = wasm.generate_key_b64()
          const vk = wasm.derive_verification_key_b64(kek)
          results.push({ label: "derive_verification_key_b64", status: "ok", detail: vk.slice(0, 16) + "..." })
        } catch (e) {
          results.push({ label: "derive_verification_key_b64", status: "error", detail: String(e) })
        }

        try {
          const vk = wasm.generate_key_b64()
          const hash = wasm.bcrypt_hash_b64(vk)
          results.push({ label: "bcrypt_hash_b64", status: "ok", detail: hash.slice(0, 20) + "..." })
        } catch (e) {
          results.push({ label: "bcrypt_hash_b64", status: "error", detail: String(e) })
        }

        try {
          const key = wasm.generate_key_b64()
          const wrapping = wasm.generate_key_b64()
          const enc = wasm.encrypt_key_b64(key, wrapping)
          results.push({ label: "encrypt_key_b64", status: "ok", detail: JSON.parse(enc).nonce.slice(0, 16) + "..." })
        } catch (e) {
          results.push({ label: "encrypt_key_b64", status: "error", detail: String(e) })
        }

        try {
          const kp = wasm.generate_keypair_b64()
          results.push({ label: "generate_keypair_b64", status: "ok", detail: JSON.parse(kp).public_key.slice(0, 16) + "..." })
        } catch (e) {
          results.push({ label: "generate_keypair_b64", status: "error", detail: String(e) })
        }
      } catch (e) {
        results.push({ label: "WASM init", status: "error", detail: String(e) })
      }

      setChecks(results)
    }

    runChecks()
  }, [])

  if (!open) {
    return (
      <button
        onClick={() => setOpen(true)}
        className="fixed bottom-2 right-2 z-[9999] size-6 rounded-full bg-foreground/10 text-[10px] font-mono text-muted-foreground hover:bg-foreground/20"
      >
        ?
      </button>
    )
  }

  const errors = checks.filter((c) => c.status === "error").length
  const oks = checks.filter((c) => c.status === "ok").length

  return (
    <div className="fixed bottom-2 right-2 z-[9999] w-80 rounded-lg border bg-background font-mono text-xs shadow-xl">
      <div className="flex items-center justify-between border-b px-3 py-2">
        <span className="font-semibold">
          Debug ({oks}/{checks.length}) {errors > 0 && <span className="text-destructive">{errors} failed</span>}
        </span>
        <button onClick={() => setOpen(false)} className="text-muted-foreground hover:text-foreground">
          ✕
        </button>
      </div>
      <div className="max-h-64 overflow-y-auto p-3">
        {checks.map((c) => (
          <div key={c.label} className="flex gap-2 py-1">
            <span
              className={
                c.status === "ok"
                  ? "text-green-500"
                  : c.status === "error"
                    ? "text-destructive"
                    : "text-yellow-500"
              }
            >
              {c.status === "ok" ? "●" : c.status === "error" ? "✕" : "◌"}
            </span>
            <span className="w-48 shrink-0 text-muted-foreground">{c.label}</span>
            <span className="truncate text-foreground">{c.detail}</span>
          </div>
        ))}
      </div>
    </div>
  )
}
