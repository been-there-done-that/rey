"use client"

import { useState } from "react"
import { useWasmStore } from "@/lib/wasm-store"
import { useFFmpegStore } from "@/lib/ffmpeg-store"

export function DebugPanel() {
  const [open, setOpen] = useState(false)
  const [copied, setCopied] = useState(false)

  const wasmStatus = useWasmStore((s) => s.status)
  const wasmError = useWasmStore((s) => s.error)
  const wasmInitTime = useWasmStore((s) => s.initTime)

  const ffmpegStatus = useFFmpegStore((s) => s.status)
  const ffmpegError = useFFmpegStore((s) => s.error)
  const ffmpegLoadTime = useFFmpegStore((s) => s.loadTime)

  const items = [
    { label: "Crypto WASM", value: wasmStatus },
    { label: "Crypto init", value: wasmInitTime ? `${Date.now() - wasmInitTime}ms ago` : "not initialized" },
    { label: "Crypto error", value: wasmError ?? "none" },
    { label: "FFmpeg", value: ffmpegStatus },
    { label: "FFmpeg load", value: ffmpegLoadTime ? `${ffmpegLoadTime}ms` : "not loaded" },
    { label: "FFmpeg error", value: ffmpegError ?? "none" },
  ]

  function copyReport() {
    const lines = items.map((i) => `${i.label}: ${i.value}`)
    navigator.clipboard.writeText(lines.join("\n"))
    setCopied(true)
    setTimeout(() => setCopied(false), 2000)
  }

  const wasmDot = wasmStatus === "ready" ? "text-green-500"
    : wasmStatus === "error" ? "text-destructive"
    : wasmStatus === "loading" ? "text-yellow-500"
    : "text-muted-foreground"

  const ffmpegDot = ffmpegStatus === "ready" ? "text-green-500"
    : ffmpegStatus === "error" ? "text-destructive"
    : ffmpegStatus === "loading" ? "text-yellow-500"
    : "text-muted-foreground"

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
          <span className={wasmDot}>●</span>
          <span className={ffmpegDot}>●</span>
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
            <span className={item.label.includes("error") && item.value !== "none" ? "text-destructive" : "text-foreground"}>
              {item.value}
            </span>
          </div>
        ))}
      </div>
    </div>
  )
}
