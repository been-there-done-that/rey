"use client";

import { MasonryGrid } from "@/components/masonry-grid";
import { UploadDropzone } from "@/components/upload-dropzone";

export default function HomePage() {
  return (
    <div className="flex flex-1 flex-col gap-6 p-6">
      <div className="mx-auto w-full max-w-xl">
        <UploadDropzone />
      </div>
      <MasonryGrid />
    </div>
  );
}
