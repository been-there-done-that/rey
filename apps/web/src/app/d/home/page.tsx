"use client";

export default function HomePage() {
  return (
    <div className="flex flex-1 items-center justify-center">
      <div className="flex flex-col items-center gap-4 text-center">
        <svg
          xmlns="http://www.w3.org/2000/svg"
          width="120"
          height="120"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="1"
          strokeLinecap="round"
          strokeLinejoin="round"
          className="text-muted-foreground/40"
        >
          <rect width="18" height="18" x="3" y="3" rx="2" />
          <path d="M3 16l4-4 3 3 4-4 5 5" />
          <circle cx="9" cy="9" r="1.5" />
          <path d="M12 3v3" />
          <path d="M15 3h3" />
        </svg>
        <div className="flex flex-col gap-1">
          <h3 className="text-lg font-medium">No photos yet</h3>
          <p className="text-sm text-muted-foreground">
            Connect a source to start importing your photos.
          </p>
        </div>
      </div>
    </div>
  );
}
