import { resolve, join } from "node:path";
import {
  mkdirSync,
  copyFileSync,
  readFileSync,
  writeFileSync,
  mkdtempSync,
  cpSync,
  rmSync,
} from "node:fs";

const source = process.argv[2];
const upstream = source
  ? resolve(source)
  : resolve(import.meta.dir, "../node_modules/@pocketjs/framework");
if (source) {
  const revision = Bun.spawnSync(["git", "-C", upstream, "rev-parse", "HEAD"]);
  if (
    revision.exitCode !== 0 ||
    revision.stdout.toString().trim() !== "8bc0525776efe779ef9592e0b91574cfbc91f60b"
  ) {
    throw new Error("PocketJS checkout must match the revision in brain-pocketjs/Cargo.toml");
  }
}
const root = resolve(import.meta.dir, "../crates/brain-pocketjs");
const { buildAot } = await import(join(upstream, "microts/compiler/aot-build.ts"));
const { bakeAtlases } = await import(join(upstream, "framework/compiler/bake-font.ts"));
const { fontSlotFor } = await import(join(upstream, "framework/compiler/tailwind.ts"));
// Resolve Solid and PocketJS imports against the compiler's installed dependencies.
const staging = mkdtempSync(join(upstream, ".brain-pocketjs-"));
try {
  cpSync(join(root, "app"), staging, { recursive: true });
  const result = await buildAot(join(staging, "App.tsx"), {
    outDir: join(root, "src/generated"),
    strict: true,
  });
  // The upstream printer emits vec!(...). This early lint needs a per-file allowance.
  for (const file of result.files.filter((file: string) => file.endsWith(".rs"))) {
    writeFileSync(
      file,
      "#![allow(clippy::nonstandard_macro_braces)]\n" + readFileSync(file, "utf8"),
    );
  }
} finally {
  rmSync(staging, { recursive: true });
}
mkdirSync(join(root, "assets"), { recursive: true });
for (const atlas of await bakeAtlases({
  codepoints: [],
  slots: [fontSlotFor(24, false), fontSlotFor(54, false)],
})) {
  writeFileSync(join(root, `assets/font-${atlas.px}.bin`), atlas.bytes);
}
copyFileSync(join(upstream, "assets/fonts/LICENSE.txt"), join(root, "assets/FONT-LICENSE.txt"));
copyFileSync(join(upstream, "LICENSE"), join(root, "assets/POCKETJS-LICENSE.txt"));
