import { authApi } from "./api"
import { streamDecrypt, decryptKey } from "./auth-crypto"

export interface DecryptedFile {
  blob: Blob
  mimeType: string
  metadata: Record<string, unknown> | null
}

export async function decryptFileKey(
  encryptedKeyB64: string,
  keyNonceB64: string,
  masterKeyB64: string
): Promise<string> {
  return decryptKey(encryptedKeyB64, keyNonceB64, masterKeyB64)
}

export async function decryptThumbnail(
  encryptedThumbnailB64: string,
  thumbDecryptionHeaderB64: string,
  fileKeyB64: string
): Promise<Blob> {
  const decryptedB64 = await streamDecrypt(
    thumbDecryptionHeaderB64,
    encryptedThumbnailB64,
    fileKeyB64
  )

  const decryptedBytes = base64ToArrayBuffer(decryptedB64)
  return new Blob([decryptedBytes], { type: "image/jpeg" })
}

export async function downloadAndDecryptFile(
  token: string,
  fileId: number,
  fileKeyB64: string,
  fileDecryptionHeaderB64: string,
  mimeType: string
): Promise<DecryptedFile> {
  const downloadRes = await authApi(token)
    .get(`api/files/${fileId}/download`)
    .json<{ url: string }>()

  const encryptedResponse = await fetch(downloadRes.url)
  if (!encryptedResponse.ok) throw new Error("Failed to download file")

  const encryptedBytes = await encryptedResponse.arrayBuffer()
  const encryptedB64 = arrayBufferToBase64(encryptedBytes)

  const decryptedB64 = await streamDecrypt(
    fileDecryptionHeaderB64,
    encryptedB64,
    fileKeyB64
  )

  const decryptedBytes = base64ToArrayBuffer(decryptedB64)
  return {
    blob: new Blob([decryptedBytes], { type: mimeType }),
    mimeType,
    metadata: null,
  }
}

function arrayBufferToBase64(buf: ArrayBuffer): string {
  const bytes = new Uint8Array(buf)
  let binary = ""
  for (let i = 0; i < bytes.byteLength; i++) {
    binary += String.fromCharCode(bytes[i])
  }
  return btoa(binary)
}

function base64ToArrayBuffer(b64: string): ArrayBuffer {
  const binary = atob(b64)
  const bytes = new Uint8Array(binary.length)
  for (let i = 0; i < binary.length; i++) {
    bytes[i] = binary.charCodeAt(i)
  }
  return bytes.buffer
}
