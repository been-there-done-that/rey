import ky from "ky"

const apiBase = ky.create({
  prefix: process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:8080",
  timeout: 15000,
})

export function authApi(token: string) {
  return apiBase.extend({
    headers: { Authorization: `Bearer ${token}` },
  })
}

export { apiBase as api }
