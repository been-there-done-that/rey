import { useWasmStore } from "./wasm-store"
import init, * as wasm from "@/wasm/zoo_wasm"

const WASM_URL = "/wasm/zoo_wasm_bg.wasm"

let initialized = false

export async function initWasm() {
  if (initialized) return

  const store = useWasmStore.getState()
  if (store.status === "ready") return

  store.setLoading()

  try {
    const start = Date.now()
    await init({ module_or_path: WASM_URL })
    const elapsed = Date.now() - start
    initialized = true
    useWasmStore.getState().setReady()
    console.log(`[zoo-wasm] initialized in ${elapsed}ms`)
  } catch (err) {
    const message = err instanceof Error ? err.message : "Unknown WASM initialization error"
    useWasmStore.getState().setError(message)
    throw err
  }
}

export function getWasm() {
  return wasm
}

export function isWasmReady(): boolean {
  return initialized
}
