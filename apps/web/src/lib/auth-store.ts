import { create } from "zustand"

interface AuthState {
  token: string | null
  masterKey: string | null
  setToken: (token: string | null) => void
  setMasterKey: (key: string | null) => void
  logout: () => void
}

export const useAuth = create<AuthState>((set) => ({
  token: typeof window !== "undefined" ? localStorage.getItem("token") : null,
  masterKey: typeof window !== "undefined" ? localStorage.getItem("master_key") : null,
  setToken: (token) => {
    if (token) {
      localStorage.setItem("token", token)
    } else {
      localStorage.removeItem("token")
    }
    set({ token })
  },
  setMasterKey: (key) => {
    if (key) {
      localStorage.setItem("master_key", key)
    } else {
      localStorage.removeItem("master_key")
    }
    set({ masterKey: key })
  },
  logout: () => {
    localStorage.removeItem("token")
    localStorage.removeItem("master_key")
    set({ token: null, masterKey: null })
  },
}))
