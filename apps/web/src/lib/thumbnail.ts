import { extractVideoFrame, isVideoFile } from "./ffmpeg-store"

const MAX_DIMENSION = 720
const MAX_SIZE = 100 * 1024
const MIN_QUALITY = 0.5
const START_QUALITY = 0.7
const QUALITY_STEP = 0.1
const TIMEOUT_MS = 30_000

export async function generateThumbnail(
  file: File,
  maxWidth = MAX_DIMENSION,
  maxSize = MAX_SIZE
): Promise<{ blob: Blob; width: number; height: number }> {
  if (isVideoFile(file)) {
    return generateVideoThumbnail(file, maxWidth, maxSize)
  }
  return generateImageThumbnail(file, maxWidth, maxSize)
}

async function generateImageThumbnail(
  file: File,
  maxWidth: number,
  maxSize: number
): Promise<{ blob: Blob; width: number; height: number }> {
  const img = await loadImage(file)
  const { width, height } = calculateDimensions(img.width, img.height, maxWidth)

  const canvas = document.createElement("canvas")
  canvas.width = width
  canvas.height = height

  const ctx = canvas.getContext("2d")
  if (!ctx) throw new Error("Failed to get canvas context")

  ctx.drawImage(img, 0, 0, width, height)
  return compressToSize(canvas, maxSize, width, height)
}

async function generateVideoThumbnail(
  file: File,
  maxWidth: number,
  maxSize: number
): Promise<{ blob: Blob; width: number; height: number }> {
  try {
    return await generateVideoThumbnailUsingCanvas(file, maxWidth, maxSize)
  } catch {
    const frameBlob = await extractVideoFrame(file, 0.5)
    const img = await loadBlobAsImage(frameBlob)
    const { width, height } = calculateDimensions(img.width, img.height, maxWidth)

    const canvas = document.createElement("canvas")
    canvas.width = width
    canvas.height = height

    const ctx = canvas.getContext("2d")
    if (!ctx) throw new Error("Failed to get canvas context")

    ctx.drawImage(img, 0, 0, width, height)
    return compressToSize(canvas, maxSize, width, height)
  }
}

async function generateVideoThumbnailUsingCanvas(
  file: File,
  maxWidth: number,
  maxSize: number
): Promise<{ blob: Blob; width: number; height: number }> {
  const canvas = document.createElement("canvas")
  const ctx = canvas.getContext("2d")
  if (!ctx) throw new Error("Failed to get canvas context")

  const videoURL = URL.createObjectURL(file)

  await withTimeout(
    new Promise<void>((resolve, reject) => {
      const video = document.createElement("video")
      video.preload = "metadata"
      video.muted = true
      video.src = videoURL
      video.addEventListener("loadeddata", () => {
        try {
          URL.revokeObjectURL(videoURL)
          video.currentTime = Math.min(0.5, video.duration * 0.25)
        } catch (e) {
          reject(e)
        }
      })
      video.addEventListener("seeked", () => {
        try {
          const { width, height } = calculateDimensions(
            video.videoWidth,
            video.videoHeight,
            maxWidth
          )
          canvas.width = width
          canvas.height = height
          ctx.drawImage(video, 0, 0, width, height)
          resolve()
        } catch (e) {
          reject(e)
        }
      })
      video.addEventListener("error", () => {
        URL.revokeObjectURL(videoURL)
        reject(new Error("Video failed to load"))
      })
    }),
    TIMEOUT_MS
  )

  return compressToSize(canvas, maxSize, canvas.width, canvas.height)
}

async function compressToSize(
  canvas: HTMLCanvasElement,
  maxSize: number,
  width: number,
  height: number
): Promise<{ blob: Blob; width: number; height: number }> {
  let quality = START_QUALITY

  while (quality >= MIN_QUALITY) {
    const blob = await new Promise<Blob | null>((resolve) =>
      canvas.toBlob(resolve, "image/jpeg", quality)
    )

    if (blob && blob.size <= maxSize) {
      return { blob, width, height }
    }

    quality -= QUALITY_STEP
  }

  const blob = await new Promise<Blob>((resolve) =>
    canvas.toBlob((b) => resolve(b!), "image/jpeg", MIN_QUALITY)
  )

  return { blob, width, height }
}

function loadImage(file: File): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const img = new Image()
    img.onload = () => resolve(img)
    img.onerror = () => reject(new Error("Failed to load image"))
    img.src = URL.createObjectURL(file)
  })
}

function loadBlobAsImage(blob: Blob): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const img = new Image()
    img.onload = () => resolve(img)
    img.onerror = () => reject(new Error("Failed to load image from blob"))
    img.src = URL.createObjectURL(blob)
  })
}

function calculateDimensions(
  w: number,
  h: number,
  maxDim: number
): { width: number; height: number } {
  if (w <= maxDim && h <= maxDim) return { width: w, height: h }

  const ratio = Math.min(maxDim / w, maxDim / h)
  return {
    width: Math.round(w * ratio),
    height: Math.round(h * ratio),
  }
}

function withTimeout<T>(promise: Promise<T>, ms: number): Promise<T> {
  return Promise.race([
    promise,
    new Promise<T>((_, reject) =>
      setTimeout(() => reject(new Error("Timeout")), ms)
    ),
  ])
}
