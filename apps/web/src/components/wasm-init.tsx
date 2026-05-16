"use client"

import { useEffect } from "react"
import { initWasm } from "@/lib/wasm-loader"

export function WasmInit() {
  useEffect(() => {
    initWasm().catch(() => {})
  }, [])
  return null
}
