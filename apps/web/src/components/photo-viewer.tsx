"use client"

import { useEffect, useState, useCallback } from "react"
import { useAuth } from "@/lib/auth-store"
import { decryptFileKey, downloadAndDecryptFile } from "@/lib/file-viewer"
import { XIcon, Loader2Icon } from "lucide-react"

interface PhotoViewerProps {
  fileId: number
  encryptedKey: string
  keyNonce: string
  fileDecryptionHeader: string
  mimeType: string
  thumbUrl?: string
  onClose: () => void
}

export function PhotoViewer({
  fileId,
  encryptedKey,
  keyNonce,
  fileDecryptionHeader,
  mimeType,
  thumbUrl,
  onClose,
}: PhotoViewerProps) {
  const token = useAuth((s) => s.token)
  const masterKey = useAuth((s) => s.masterKey)
  const [fullUrl, setFullUrl] = useState<string | null>(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState(false)

  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose()
    },
    [onClose]
  )

  useEffect(() => {
    document.addEventListener("keydown", handleKeyDown)
    return () => document.removeEventListener("keydown", handleKeyDown)
  }, [handleKeyDown])

  useEffect(() => {
    if (!token || !masterKey) return

    let cancelled = false

    async function loadFullImage() {
      try {
        const fileKey = await decryptFileKey(encryptedKey, keyNonce, masterKey!)

        const { blob } = await downloadAndDecryptFile(
          token!,
          fileId,
          fileKey,
          fileDecryptionHeader,
          mimeType
        )

        if (!cancelled) {
          const url = URL.createObjectURL(blob)
          setFullUrl(url)
          setLoading(false)
        }
      } catch (err) {
        console.error("Failed to load full image:", err)
        if (!cancelled) {
          setError(true)
          setLoading(false)
        }
      }
    }

    loadFullImage()
    return () => {
      cancelled = true
    }
  }, [token, masterKey, fileId, encryptedKey, keyNonce, fileDecryptionHeader, mimeType])

  useEffect(() => {
    return () => {
      if (fullUrl) URL.revokeObjectURL(fullUrl)
    }
  }, [fullUrl])

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/90"
      onClick={onClose}
    >
      <button
        className="absolute right-4 top-4 rounded-full bg-white/10 p-2 text-white hover:bg-white/20"
        onClick={onClose}
      >
        <XIcon className="size-5" />
      </button>

      <div className="relative max-h-[90vh] max-w-[90vw]" onClick={(e) => e.stopPropagation()}>
        {thumbUrl && !fullUrl && (
          <img
            src={thumbUrl}
            alt=""
            className="max-h-[90vh] max-w-[90vw] object-contain blur-sm"
          />
        )}

        {fullUrl && (
          <img
            src={fullUrl}
            alt=""
            className="max-h-[90vh] max-w-[90vw] object-contain"
          />
        )}

        {loading && (
          <div className="absolute inset-0 flex items-center justify-center">
            <Loader2Icon className="size-8 animate-spin text-white/60" />
          </div>
        )}

        {error && (
          <div className="absolute inset-0 flex items-center justify-center text-white/60">
            Failed to load image
          </div>
        )}
      </div>
    </div>
  )
}
