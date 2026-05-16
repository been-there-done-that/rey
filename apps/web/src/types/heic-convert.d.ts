declare module "heic-convert" {
  interface ConvertOptions {
    buffer: Uint8Array
    format: "JPEG" | "PNG"
  }

  function convert(options: ConvertOptions): Promise<Uint8Array>
  export default convert
}
