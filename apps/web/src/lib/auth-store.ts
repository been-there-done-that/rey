import { create } from "zustand"

interface AuthState {
  token: string | null
  setToken: (token: string | null) => void
  logout: () => void
}

export const useAuth = create<AuthState>((set) => ({
  token: typeof window !== "undefined" ? localStorage.getItem("token") : null,
  setToken: (token) => {
    if (token) {
      localStorage.setItem("token", token)
    } else {
      localStorage.removeItem("token")
    }
    set({ token })
  },
  logout: () => {
    localStorage.removeItem("token")
    set({ token: null })
  },
}))
