import { create } from "zustand"
import { FFmpeg } from "@ffmpeg/ffmpeg"
import { fetchFile } from "@ffmpeg/util"

type FFmpegStatus = "idle" | "loading" | "ready" | "error"

interface FFmpegState {
  status: FFmpegStatus
  error: string | null
  loadTime: number | null
  ffmpeg: FFmpeg | null
  load: () => Promise<FFmpeg>
}

let ffmpegInstance: FFmpeg | null = null
let loadPromise: Promise<FFmpeg> | null = null

export const useFFmpegStore = create<FFmpegState>((set) => ({
  status: "loading",
  error: null,
  loadTime: null,
  ffmpeg: null,
  load: async () => {
    if (ffmpegInstance) return ffmpegInstance
    if (loadPromise) return loadPromise

    set({ status: "loading", error: null })
    const start = Date.now()

    loadPromise = (async () => {
      try {
        const ffmpeg = new FFmpeg()
        const baseURL = "https://unpkg.com/@ffmpeg/core@0.12.10/dist/umd"

        await ffmpeg.load({
          coreURL: `${baseURL}/ffmpeg-core.js`,
          wasmURL: `${baseURL}/ffmpeg-core.wasm`,
        })

        const elapsed = Date.now() - start
        ffmpegInstance = ffmpeg
        set({ status: "ready", loadTime: elapsed, ffmpeg })
        console.log(`[ffmpeg] loaded in ${elapsed}ms`)
        return ffmpeg
      } catch (err) {
        const message = err instanceof Error ? err.message : "Failed to load FFmpeg"
        set({ status: "error", error: message })
        throw err
      }
    })()

    return loadPromise
  },
}))

// Auto-load on module initialization (browser only)
if (typeof window !== "undefined") {
  useFFmpegStore.getState().load().catch(() => {})
}

export async function extractVideoFrame(file: File, timeSec = 0): Promise<Blob> {
  const ffmpeg = await useFFmpegStore.getState().load()

  const inputName = "input"
  const outputName = "output.jpg"

  await ffmpeg.writeFile(inputName, await fetchFile(file))
  await ffmpeg.exec(["-ss", String(timeSec), "-i", inputName, "-frames:v", "1", "-q:v", "3", outputName])

  const data = await ffmpeg.readFile(outputName)
  await ffmpeg.deleteFile(inputName)
  await ffmpeg.deleteFile(outputName)

  return new Blob([data], { type: "image/jpeg" })
}

export function isVideoFile(file: File): boolean {
  return file.type.startsWith("video/")
}
