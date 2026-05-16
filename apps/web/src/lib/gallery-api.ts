import { authApi } from "./api"
import { useAuth } from "./auth-store"

export interface GalleryFile {
  id: number
  collection_id: string
  encrypted_key: string
  key_decryption_nonce: string
  file_decryption_header: string
  thumb_decryption_header: string | null
  encrypted_metadata: string
  encrypted_thumbnail: string | null
  thumbnail_size: number | null
  file_size: number
  mime_type: string
  content_hash: string
  created_at: number
  updation_time: number
  archived_at: number | null
}

export async function fetchFiles(
  token: string,
  sinceTime = 0,
  limit = 100
): Promise<GalleryFile[]> {
  const params = new URLSearchParams()
  params.set("since_time", String(sinceTime))
  params.set("limit", String(limit))

  return authApi(token)
    .get(`api/files?${params}`)
    .json<GalleryFile[]>()
}

export async function fetchCollections(
  token: string,
  sinceTime = 0
): Promise<CollectionItem[]> {
  const params = new URLSearchParams()
  params.set("since_time", String(sinceTime))

  return authApi(token)
    .get(`api/collections?${params}`)
    .json<CollectionItem[]>()
}

export interface CollectionItem {
  id: string
  encrypted_name: string
  encrypted_key: string
  key_decryption_nonce: string
  encrypted_metadata: string | null
  updation_time: number
}

export async function createCollection(
  token: string,
  encryptedName: string,
  encryptedKey: string,
  keyDecryptionNonce: string,
  encryptedMetadata?: string
): Promise<{ collection_id: string }> {
  return authApi(token)
    .post("api/collections", {
      json: {
        encrypted_name: encryptedName,
        encrypted_key: encryptedKey,
        key_decryption_nonce: keyDecryptionNonce,
        encrypted_metadata: encryptedMetadata,
      },
    })
    .json<{ collection_id: string }>()
}
