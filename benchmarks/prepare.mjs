import { mkdir, copyFile, writeFile, readFile } from 'node:fs/promises';
import { resolve, relative } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { fixture, markdownSizes } from './fixtures.mjs';
const root = fileURLToPath(new URL('../', import.meta.url));
const ace = resolve(process.env.ACE_ROOT ?? resolve(root, '../ace'));
const artifacts = resolve(root, 'benchmarks/artifacts');
await mkdir(artifacts, { recursive: true });
execFileSync('bun', ['build', resolve(ace,'apps/web/src/components/markdown/highlight.ts'), resolve(ace,'packages/ui-core/src/source-highlight.ts'), resolve(ace,'apps/web/src/components/markdown/block-stream.ts'), '--target=browser','--format=esm',`--outdir=${artifacts}/ace`], { stdio: 'inherit' });
const markedRoot = resolve(ace,'apps/web/node_modules/marked');
await copyFile(resolve(markedRoot,'lib/marked.esm.js'),resolve(artifacts,'marked.esm.js'));
await copyFile(resolve(ace,'apps/web/node_modules/katex/dist/katex.mjs'),resolve(artifacts,'katex.mjs'));
const sourceFiles = ['apps/ios/Sources/Models/Markdown.swift','apps/web/src/components/markdown/highlight.ts','apps/web/src/components/markdown/block-stream.ts','packages/ui-core/src/source-highlight.ts','apps/android/src/Thread.tsx'];
const metadata = {
  prepared_at: new Date().toISOString(),
  ace_root: relative(root,ace),
  ace_commit: execFileSync('git',['-C',ace,'rev-parse','HEAD'],{encoding:'utf8'}).trim(),
  markit_commit: execFileSync('git',['-C',root,'rev-parse','HEAD'],{encoding:'utf8'}).trim(),
  marked: JSON.parse(await readFile(resolve(markedRoot,'package.json'),'utf8')).version,
  katex: JSON.parse(await readFile(resolve(ace,'apps/web/node_modules/katex/package.json'),'utf8')).version,
  prism: JSON.parse(await readFile(resolve(ace,'packages/ui-core/node_modules/prismjs/package.json'),'utf8')).version,
  cpu: execFileSync('sysctl',['-n','machdep.cpu.brand_string'],{encoding:'utf8'}).trim(),
  os: execFileSync('sw_vers',['-productVersion'],{encoding:'utf8'}).trim(),
  node: process.version,
  bun: execFileSync('bun',['--version'],{encoding:'utf8'}).trim(),
  rust: execFileSync('rustc',['--version'],{encoding:'utf8'}).trim(),
  swift: execFileSync('swiftc',['--version'],{encoding:'utf8'}).trim(),
  native_optimization: 'swiftc -O; Rust release with thin LTO',
  native_autorelease_pool_per_operation: true,
  native_scope: 'Actual iOS model code executing on macOS; no UIKit layout or device execution',
  source_sha256: Object.fromEntries(await Promise.all(sourceFiles.map(async file=>[file,createHash('sha256').update(await readFile(resolve(ace,file))).digest('hex')]))),
  source_files: sourceFiles,
};
await writeFile(resolve(root,'benchmarks/results/environment.json'),JSON.stringify(metadata,null,2)+'\n');
await writeFile(resolve(artifacts,'native-fixtures.json'),JSON.stringify(markdownSizes.map(size=>({name:`${Math.round(size/1024)}KiB`,source:fixture(size,'native')})),null,2)+'\n');
console.log('Prepared benchmark fixtures and existing Ace implementations.');
