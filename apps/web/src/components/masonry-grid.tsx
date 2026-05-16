"use client"

import { useEffect, useRef, useState, useCallback } from "react"
import { useAuth } from "@/lib/auth-store"
import { fetchFiles, type GalleryFile } from "@/lib/gallery-api"
import { decryptFileKey, decryptThumbnail } from "@/lib/file-viewer"
import { PhotoViewer } from "@/components/photo-viewer"
import { cn } from "@/lib/utils"
import { Loader2Icon, ImageOffIcon } from "lucide-react"

const COLUMN_COUNTS = [2, 3, 4, 5, 6]
const BREAKPOINTS = [640, 768, 1024, 1280, 1536]
const GAP = 4

function getColumnCount(width: number): number {
  for (let i = BREAKPOINTS.length - 1; i >= 0; i--) {
    if (width >= BREAKPOINTS[i]) return COLUMN_COUNTS[i]
  }
  return COLUMN_COUNTS[0]
}

interface TileProps {
  file: GalleryFile
  index: number
}

function Tile({ file }: TileProps) {
  const token = useAuth((s) => s.token)
  const masterKey = useAuth((s) => s.masterKey)
  const [thumbUrl, setThumbUrl] = useState<string | null>(null)
  const [loaded, setLoaded] = useState(false)
  const [error, setError] = useState(false)
  const [viewerOpen, setViewerOpen] = useState(false)

  useEffect(() => {
    if (!token || !masterKey || !file.encrypted_thumbnail || !file.thumb_decryption_header) return

    let cancelled = false

    async function loadThumbnail() {
      try {
        const fileKey = await decryptFileKey(
          file.encrypted_key,
          file.key_decryption_nonce,
          masterKey!
        )

        const thumbBlob = await decryptThumbnail(
          file.encrypted_thumbnail!,
          file.thumb_decryption_header!,
          fileKey
        )

        if (!cancelled) {
          const url = URL.createObjectURL(thumbBlob)
          setThumbUrl(url)
        }
      } catch (err) {
        console.error("Failed to decrypt thumbnail:", err)
        if (!cancelled) setError(true)
      }
    }

    loadThumbnail()
    return () => {
      cancelled = true
      if (thumbUrl) URL.revokeObjectURL(thumbUrl)
    }
  }, [token, masterKey, file])

  const aspectRatio = file.thumbnail_size
    ? Math.max(0.5, Math.min(1.5, 1 + (Math.random() - 0.5) * 0.6))
    : 1

  return (
    <>
      <div
        className={cn(
          "group relative cursor-pointer overflow-hidden rounded-lg bg-muted",
          loaded ? "opacity-100" : "opacity-0",
          "transition-opacity duration-300"
        )}
        style={{ aspectRatio: `${aspectRatio}` }}
        onClick={() => setViewerOpen(true)}
      >
        {thumbUrl && (
          <img
            src={thumbUrl}
            alt=""
            className="absolute inset-0 size-full object-cover transition-transform duration-300 group-hover:scale-105"
            onLoad={() => setLoaded(true)}
            onError={() => setError(true)}
          />
        )}
        {!loaded && !error && (
          <div className="absolute inset-0 flex items-center justify-center">
            <Loader2Icon className="size-6 animate-spin text-muted-foreground" />
          </div>
        )}
        {error && (
          <div className="absolute inset-0 flex flex-col items-center justify-center gap-1 text-muted-foreground">
            <ImageOffIcon className="size-6" />
            <span className="text-[10px]">Failed to load</span>
          </div>
        )}
        <div className="absolute inset-0 bg-gradient-to-t from-black/40 to-transparent opacity-0 transition-opacity group-hover:opacity-100" />
      </div>

      {viewerOpen && (
        <PhotoViewer
          fileId={file.id}
          encryptedKey={file.encrypted_key}
          keyNonce={file.key_decryption_nonce}
          fileDecryptionHeader={file.file_decryption_header}
          mimeType={file.mime_type}
          thumbUrl={thumbUrl || undefined}
          onClose={() => setViewerOpen(false)}
        />
      )}
    </>
  )
}

export function MasonryGrid() {
  const token = useAuth((s) => s.token)
  const [files, setFiles] = useState<GalleryFile[]>([])
  const [loading, setLoading] = useState(true)
  const [columns, setColumns] = useState(4)
  const containerRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (!token) return

    setLoading(true)
    fetchFiles(token)
      .then(setFiles)
      .catch(console.error)
      .finally(() => setLoading(false))
  }, [token])

  useEffect(() => {
    const el = containerRef.current
    if (!el) return

    const observer = new ResizeObserver((entries) => {
      for (const entry of entries) {
        setColumns(getColumnCount(entry.contentRect.width))
      }
    })

    observer.observe(el)
    return () => observer.disconnect()
  }, [])

  const distributeToColumns = useCallback(() => {
    if (files.length === 0) return []

    const cols: GalleryFile[][] = Array.from({ length: columns }, () => [])
    const heights = new Array(columns).fill(0)

    for (const file of files) {
      const shortest = heights.indexOf(Math.min(...heights))
      cols[shortest].push(file)
      heights[shortest] += 1
    }

    return cols
  }, [files, columns])

  const columns_data = distributeToColumns()

  if (loading) {
    return (
      <div className="flex items-center justify-center py-20">
        <Loader2Icon className="size-8 animate-spin text-muted-foreground" />
      </div>
    )
  }

  if (files.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center gap-3 py-20 text-center">
        <ImageOffIcon className="size-12 text-muted-foreground/40" />
        <p className="text-sm text-muted-foreground">No photos yet</p>
      </div>
    )
  }

  return (
    <div ref={containerRef} className="flex gap-1">
      {columns_data.map((col, i) => (
        <div key={i} className="flex flex-1 flex-col gap-1">
          {col.map((file) => (
            <Tile key={file.id} file={file} index={file.id} />
          ))}
        </div>
      ))}
    </div>
  )
}
