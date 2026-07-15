import { type ClassValue, clsx } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

export function devWarn(message: string) {
  if (typeof window !== "undefined" && globalThis.location?.hostname === "localhost") {
    console.warn(message);
  }
}
