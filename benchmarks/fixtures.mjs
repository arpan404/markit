const common = '# Renderer benchmark\n\nA paragraph with **strong text**, *emphasis*, `inline code`, and a [documentation link](https://example.com/docs). Unicode stays intact: café 🦀.\n\n- First item with **bold**\n- Second item with *emphasis*\n\n> A quoted paragraph with a [link](https://example.com).\n\n---\n\n';
const gfm = common + '| Platform | Output |\n| :--- | ---: |\n| iOS | Native text |\n| Android | Native text |\n| Web | WebAssembly |\n\n- [x] Checked task\n- [ ] Open task\n\n';
const ts = 'export function greet(name: string): string {\n  // Preserve Unicode and nested syntax.\n  const value = `Hello ${name} 🦀`;\n  return value + "!";\n}\n\n';
export function fixture(bytes, kind = 'gfm') {
  const section = kind === 'native' ? common : kind === 'code' ? ts : gfm;
  const count = Math.max(1, Math.ceil(bytes / new TextEncoder().encode(section).length));
  return section.repeat(count);
}
export const markdownSizes = [4096, 32768, 131072];
export const codeSizes = [4096, 16384];
