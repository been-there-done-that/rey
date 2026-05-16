import Link from "next/link";

export default function Home() {
  return (
    <div className="flex min-h-screen items-center justify-center">
      <Link href="/d/home">Go to Home</Link>
    </div>
  );
}
