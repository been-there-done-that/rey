import type { Metadata } from "next";
import { Geist, Fira_Code, Inter, Roboto } from "next/font/google";
import "./globals.css";
import { cn } from "@/lib/utils";
import { DebugPanel } from "@/components/debug-panel";
import { WasmInit } from "@/components/wasm-init";

const robotoHeading = Roboto({subsets:['latin'],variable:'--font-heading'});

const inter = Inter({subsets:['latin'],variable:'--font-sans'});

const geistSans = Geist({
  variable: "--font-geist-sans",
  subsets: ["latin"],
});

const firaCode = Fira_Code({
  variable: "--font-mono",
  subsets: ["latin"],
});

export const metadata: Metadata = {
  title: "Rey",
  description: "Personal photo gallery",
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html
      lang="en"
      className={cn("h-full", "antialiased", geistSans.variable, firaCode.variable, "font-sans", inter.variable, robotoHeading.variable)}
    >
      <body className="h-screen overflow-hidden flex flex-col">
        <WasmInit />
        {children}
        {process.env.NODE_ENV === "development" && <DebugPanel />}
      </body>
    </html>
  );
}
