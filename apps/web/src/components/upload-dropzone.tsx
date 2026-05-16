"use client"

import { useCallback, useEffect, useRef, useState } from "react"
import { uploadManager } from "@/lib/upload-manager"
import type { UploadFile } from "@/lib/upload-types"
import { Button } from "@/components/ui/button"
import { cn } from "@/lib/utils"
import { UploadIcon, CheckIcon, XIcon, Loader2Icon, FileIcon } from "lucide-react"

export function UploadDropzone() {
  const [dragging, setDragging] = useState(false)
  const [files, setFiles] = useState<UploadFile[]>([])
  const inputRef = useRef<HTMLInputElement>(null)

  useEffect(() => {
    const unsub = uploadManager.onProgress((p) => setFiles([...p.files]))
    return () => { unsub() }
  }, [])

  const handleDrop = useCallback((e: React.DragEvent) => {
    e.preventDefault()
    setDragging(false)
    if (e.dataTransfer.files.length > 0) {
      uploadManager.addFiles(e.dataTransfer.files)
    }
  }, [])

  const handleSelect = useCallback(() => {
    inputRef.current?.click()
  }, [])

  const handleFileChange = useCallback(() => {
    if (inputRef.current?.files && inputRef.current.files.length > 0) {
      uploadManager.addFiles(inputRef.current.files)
      inputRef.current.value = ""
    }
  }, [])

  const statusIcon = (file: UploadFile) => {
    switch (file.status) {
      case "done":
        return <CheckIcon className="size-4 text-green-500" />
      case "error":
        return <XIcon className="size-4 text-destructive" />
      case "thumbnail":
      case "encrypting":
      case "uploading":
      case "registering":
        return <Loader2Icon className="size-4 animate-spin text-muted-foreground" />
      default:
        return <FileIcon className="size-4 text-muted-foreground" />
    }
  }

  if (files.length === 0) {
    return (
      <div
        role="button"
        tabIndex={0}
        onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") handleSelect() }}
        onClick={handleSelect}
        onDragOver={(e) => { e.preventDefault(); setDragging(true) }}
        onDragLeave={() => setDragging(false)}
        onDrop={handleDrop}
        className={cn(
          "flex cursor-pointer flex-col items-center justify-center gap-4 rounded-xl border-2 border-dashed p-12 transition-colors",
          dragging ? "border-primary bg-primary/5" : "border-border"
        )}
      >
        <UploadIcon className="size-10 text-muted-foreground" />
        <div className="text-center">
          <p className="text-sm font-medium">Drop photos or videos here</p>
          <p className="text-xs text-muted-foreground">or click to browse</p>
        </div>
        <input
          ref={inputRef}
          type="file"
          multiple
          accept="image/*,video/*"
          className="hidden"
          onChange={handleFileChange}
        />
      </div>
    )
  }

  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center justify-between">
        <p className="text-sm font-medium">Uploads</p>
        <Button variant="ghost" size="sm" onClick={() => uploadManager.reset()}>
          Clear
        </Button>
      </div>
      <div className="flex flex-col gap-1 rounded-lg border p-2">
        {files.map((file) => (
          <div key={file.id} className="flex items-center gap-2 rounded-md px-2 py-1.5">
            {statusIcon(file)}
            <div className="min-w-0 flex-1">
              <p className="truncate text-xs font-medium">{file.file.name}</p>
              <div className="flex items-center gap-2">
                <div className="h-1 flex-1 overflow-hidden rounded-full bg-muted">
                  <div
                    className={cn(
                      "h-full rounded-full transition-all",
                      file.status === "error" ? "bg-destructive" : "bg-primary"
                    )}
                    style={{ width: `${file.progress}%` }}
                  />
                </div>
                <span className="text-[10px] text-muted-foreground w-8 text-right">
                  {file.progress}%
                </span>
              </div>
            </div>
          </div>
        ))}
      </div>
      <input
        ref={inputRef}
        type="file"
        multiple
        accept="image/*,video/*"
        className="hidden"
        onChange={handleFileChange}
      />
    </div>
  )
}
