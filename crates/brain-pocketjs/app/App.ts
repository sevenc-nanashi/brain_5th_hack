import { createSignal } from "solid-js";
import type { i32 } from "@pocketjs/framework/solid/std";

export const [count, setCount] = createSignal<i32>(0);

export function increment(): void {
  setCount((value) => (value < 9999 ? value + 1 : 0));
}

export function reset(): void {
  setCount(0);
}
