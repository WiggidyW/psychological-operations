// Cross-platform zipper for the viewer release artifact.
//
// Reads `dist/` produced by `vite build` and writes
// `../psychological-operations-viewer.zip` at the repo root. The
// zip's root contains `index.html` + assets (NOT a `dist/` prefix) —
// that's the layout the objectiveai host expects under
// `<plugins_dir>/<repository>/viewer/`.
//
// Used by `build.sh` (release flow) and CI; not part of `pnpm build`
// so day-to-day dev iteration doesn't pay for it.

import { ZipArchive } from "archiver";
import { createWriteStream, existsSync, rmSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const pkgRoot = resolve(here, "..");
const repoRoot = resolve(pkgRoot, "..");
const dist = resolve(pkgRoot, "dist");
const out = resolve(repoRoot, "psychological-operations-viewer.zip");

if (!existsSync(dist)) {
  console.error(`dist/ missing — run \`pnpm build\` first`);
  process.exit(1);
}

if (existsSync(out)) rmSync(out);

const output = createWriteStream(out);
const archive = new ZipArchive({ zlib: { level: 9 } });

output.on("close", () => {
  console.log(`Wrote ${out} (${archive.pointer()} bytes)`);
});
archive.on("warning", (err) => {
  if (err.code !== "ENOENT") throw err;
});
archive.on("error", (err) => {
  throw err;
});

archive.pipe(output);
archive.directory(dist, false);
await archive.finalize();
