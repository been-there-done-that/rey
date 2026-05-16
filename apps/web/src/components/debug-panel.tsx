"use client"

import { useState } from "react"
import { useWasmStore } from "@/lib/wasm-store"

export function DebugPanel() {
  const [open, setOpen] = useState(false)
  const [copied, setCopied] = useState(false)
  const status = useWasmStore((s) => s.status)
  const error = useWasmStore((s) => s.error)
  const initTime = useWasmStore((s) => s.initTime)

  const items = [
    { label: "WASM status", value: status },
    { label: "Init time", value: initTime ? `${Date.now() - initTime}ms ago` : "not initialized" },
    { label: "Error", value: error ?? "none" },
  ]

  function copyReport() {
    const lines = items.map((i) => `${i.label}: ${i.value}`)
    navigator.clipboard.writeText(lines.join("\n"))
    setCopied(true)
    setTimeout(() => setCopied(false), 2000)
  }

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

  return (
    <div className="fixed bottom-2 right-2 z-[9999] w-80 rounded-lg border bg-background font-mono text-xs shadow-xl">
      <div className="flex items-center justify-between border-b px-3 py-2">
        <span className="font-semibold">
          Debug{" "}
          {status === "ready" && <span className="text-green-500">●</span>}
          {status === "error" && <span className="text-destructive">●</span>}
          {status === "loading" && <span className="text-yellow-500">◌</span>}
          {status === "idle" && <span className="text-muted-foreground">○</span>}
        </span>
        <div className="flex gap-1">
          <button
            onClick={copyReport}
            className="rounded px-1.5 py-0.5 text-muted-foreground hover:bg-muted hover:text-foreground"
          >
            {copied ? "✓" : "📋"}
          </button>
          <button onClick={() => setOpen(false)} className="rounded px-1.5 py-0.5 text-muted-foreground hover:bg-muted hover:text-foreground">
            ✕
          </button>
        </div>
      </div>
      <div className="p-3">
        {items.map((item) => (
          <div key={item.label} className="flex gap-2 py-1">
            <span className="w-24 shrink-0 text-muted-foreground">{item.label}</span>
            <span className={item.label === "Error" && item.value !== "none" ? "text-destructive" : "text-foreground"}>
              {item.value}
            </span>
          </div>
        ))}
      </div>
    </div>
  )
}
