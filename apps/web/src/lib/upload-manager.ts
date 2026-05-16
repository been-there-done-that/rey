import { api, authApi } from "./api"
import { useAuth } from "./auth-store"
import { streamEncrypt, generateKey, encryptKey } from "./auth-crypto"
import { generateThumbnail } from "./thumbnail"
import type { UploadFile, UploadProgress, UploadStatus } from "./upload-types"

const MAX_CONCURRENT = 4
const CHUNK_SIZE = 4 * 1024 * 1024

export class UploadManager {
  private queue: UploadFile[] = []
  private listeners: Set<(progress: UploadProgress) => void> = new Set()
  private running = 0

  onProgress(listener: (progress: UploadProgress) => void) {
    this.listeners.add(listener)
    return () => this.listeners.delete(listener)
  }

  async addFiles(files: FileList | File[]) {
    const fileArray = Array.from(files)
    const uploads: UploadFile[] = fileArray.map((f) => ({
      id: crypto.randomUUID(),
      file: f,
      status: "queued",
      progress: 0,
      error: null,
    }))

    this.queue.push(...uploads)
    this.notify()
    this.processQueue()
  }

  private async processQueue() {
    while (this.queue.some((f) => f.status === "queued") && this.running < MAX_CONCURRENT) {
      const next = this.queue.find((f) => f.status === "queued")
      if (!next) break

      this.running++
      this.processFile(next).finally(() => {
        this.running--
        this.processQueue()
      })
    }
  }

  private async processFile(upload: UploadFile) {
    try {
      await this.uploadFile(upload)
    } catch (err) {
      upload.status = "error"
      upload.error = err instanceof Error ? err.message : "Upload failed"
      this.notify()
    }
  }

  private async uploadFile(upload: UploadFile) {
    const token = useAuth.getState().token
    const masterKey = useAuth.getState().masterKey
    if (!token) throw new Error("Not authenticated")
    if (!masterKey) throw new Error("No master key")

    const fileKey = await generateKey()

    // Step 1: Generate thumbnail
    upload.status = "thumbnail"
    upload.progress = 5
    this.notify()

    let thumbBlob: Blob | undefined
    let thumbEncHeader: string | undefined
    let thumbEncCiphertext: string | undefined

    try {
      const thumb = await generateThumbnail(upload.file)
      thumbBlob = thumb.blob
      const thumbBytes = await thumb.blob.arrayBuffer()
      const thumbB64 = arrayBufferToBase64(thumbBytes)
      const thumbEnc = await streamEncrypt(thumbB64, fileKey)
      thumbEncHeader = thumbEnc.header
      thumbEncCiphertext = thumbEnc.ciphertext
    } catch {
      // Thumbnail generation is optional
    }

    // Step 2: Encrypt file
    upload.status = "encrypting"
    upload.progress = 15
    this.notify()

    const fileBytes = await upload.file.arrayBuffer()
    const fileB64 = arrayBufferToBase64(fileBytes)
    const fileEnc = await streamEncrypt(fileB64, fileKey)

    // Step 2.5: Encrypt file key with master key
    const fileKeyEnc = await encryptKey(fileKey, masterKey)

    // Step 3: Create upload on server
    upload.status = "uploading"
    upload.progress = 20
    this.notify()

    const totalSize = fileEnc.ciphertext.length
    const partCount = Math.ceil(totalSize / CHUNK_SIZE)
    const partMd5s = Array(partCount).fill("")

    const deviceId = localStorage.getItem("device_id") || crypto.randomUUID()
    if (!localStorage.getItem("device_id")) {
      localStorage.setItem("device_id", deviceId)
    }

    const uploadInit = await authApi(token)
      .post("api/uploads", {
        json: {
          file_hash: await computeFileHash(upload.file),
          file_size: upload.file.size,
          mime_type: upload.file.type || "application/octet-stream",
          part_size: Math.min(CHUNK_SIZE, totalSize),
          part_count: partCount,
          part_md5s: partMd5s,
        },
        headers: { "x-device-id": deviceId },
      })
      .json<{
        upload_id: string
        status: string
      }>()

    // Fetch presigned URLs
    const presignRes = await authApi(token)
      .post(`api/uploads/${uploadInit.upload_id}/presign`, {
        json: { part_md5s: partMd5s },
      })
      .json<{ urls: string[]; complete_url: string }>()

    // Step 4: Upload encrypted chunks to presigned URLs
    const ciphertextBytes = base64ToArrayBuffer(fileEnc.ciphertext)
    for (let i = 0; i < partCount; i++) {
      const start = i * CHUNK_SIZE
      const end = Math.min(start + CHUNK_SIZE, ciphertextBytes.byteLength)
      const chunk = ciphertextBytes.slice(start, end)

      const url = presignRes.urls[i]
      await fetch(url, {
        method: "PUT",
        body: chunk,
        headers: { "Content-Type": "application/octet-stream" },
      })

      upload.progress = 20 + Math.round((i + 1) / partCount * 60)
      this.notify()
    }

    // Step 5: Complete upload
    await authApi(token).post(`api/uploads/${uploadInit.upload_id}/complete`)

    // Step 6: Register file
    upload.status = "registering"
    upload.progress = 85
    this.notify()

    const metadata = JSON.stringify({
      name: upload.file.name,
      size: upload.file.size,
      type: upload.file.type,
      lastModified: upload.file.lastModified,
    })

    const metadataEncoded = new TextEncoder().encode(metadata)
    const metadataB64 = arrayBufferToBase64(metadataEncoded.buffer)
    const metadataEnc = await streamEncrypt(metadataB64, fileKey)

    await authApi(token).post(`api/uploads/${uploadInit.upload_id}/register`, {
      json: {
        collection_id: "default",
        encrypted_key: fileKeyEnc.ciphertext,
        key_decryption_nonce: fileKeyEnc.nonce,
        file_decryption_header: fileEnc.header,
        thumb_decryption_header: thumbEncHeader || null,
        encrypted_metadata: metadataEnc.ciphertext,
        encrypted_thumbnail: thumbEncCiphertext || null,
        thumbnail_size: thumbBlob?.size || null,
      },
    })

    upload.status = "done"
    upload.progress = 100
    this.notify()
  }

  private notify() {
    const progress: UploadProgress = {
      total: this.queue.length,
      completed: this.queue.filter((f) => f.status === "done").length,
      files: this.queue,
    }
    for (const listener of this.listeners) {
      listener(progress)
    }
  }

  getFiles() {
    return this.queue
  }

  reset() {
    this.queue = []
    this.running = 0
    this.notify()
  }
}

function arrayBufferToBase64(buf: ArrayBufferLike): string {
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

async function computeFileHash(file: File): Promise<string> {
  const bytes = await file.arrayBuffer()
  const hash = await crypto.subtle.digest("SHA-256", bytes)
  return arrayBufferToBase64(hash)
}

export const uploadManager = new UploadManager()
