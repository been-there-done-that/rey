import { create } from "zustand"

type WasmStatus = "idle" | "loading" | "ready" | "error"

interface WasmState {
  status: WasmStatus
  error: string | null
  initTime: number | null
  setReady: () => void
  setError: (error: string) => void
  setLoading: () => void
}

export const useWasmStore = create<WasmState>((set) => ({
  status: "idle",
  error: null,
  initTime: null,
  setReady: () => set({ status: "ready", initTime: Date.now() }),
  setError: (error) => set({ status: "error", error }),
  setLoading: () => set({ status: "loading", error: null }),
}))
