"use client";

import { UploadDropzone } from "@/components/upload-dropzone";

export default function HomePage() {
  return (
    <div className="flex flex-1 flex-col gap-6 p-6">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-lg font-semibold">Photos</h2>
          <p className="text-sm text-muted-foreground">
            Upload and manage your photo library
          </p>
        </div>
      </div>
      <div className="mx-auto w-full max-w-xl">
        <UploadDropzone />
      </div>
    </div>
  );
}
