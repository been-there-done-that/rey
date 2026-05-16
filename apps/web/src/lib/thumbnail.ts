const MAX_DIMENSION = 720
const MAX_SIZE = 100 * 1024
const MIN_QUALITY = 0.5
const START_QUALITY = 0.7
const QUALITY_STEP = 0.1

export async function generateThumbnail(
  file: File,
  maxWidth = MAX_DIMENSION,
  maxSize = MAX_SIZE
): Promise<{ blob: Blob; width: number; height: number }> {
  const img = await loadImage(file)
  const { width, height } = calculateDimensions(img.width, img.height, maxWidth)

  const canvas = document.createElement("canvas")
  canvas.width = width
  canvas.height = height

  const ctx = canvas.getContext("2d")
  if (!ctx) throw new Error("Failed to get canvas context")

  ctx.drawImage(img, 0, 0, width, height)

  let quality = START_QUALITY
  let blob: Blob | null = null

  while (quality >= MIN_QUALITY) {
    blob = await new Promise<Blob | null>((resolve) =>
      canvas.toBlob(resolve, "image/jpeg", quality)
    )

    if (blob && blob.size <= maxSize) {
      return { blob, width, height }
    }

    quality -= QUALITY_STEP
  }

  if (!blob) throw new Error("Failed to generate thumbnail")
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
