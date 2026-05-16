export type UploadStatus =
  | "queued"
  | "thumbnail"
  | "encrypting"
  | "uploading"
  | "registering"
  | "done"
  | "error"

export interface UploadFile {
  id: string
  file: File
  status: UploadStatus
  progress: number
  error: string | null
  thumbnail?: Blob
  encryptedFile?: ArrayBuffer
  encryptedThumbnail?: ArrayBuffer
}

export interface UploadProgress {
  total: number
  completed: number
  files: UploadFile[]
}
