// Convert the Rust UTF-16 range buffer to the public token shape. The source
// already lives in JS; it need not cross the bridge again inside token JSON.
const colors = new Map();
function color(rgb) {
  let value = colors.get(rgb);
  if (value === undefined) {
    value = `#${rgb.toString(16).padStart(6, '0')}`;
    colors.set(rgb, value);
  }
  return value;
}
export function unpackHighlight(source, words) {
  if (words[0] !== 1) throw new Error('Unsupported highlight range version');
  const firstRun = 3 + words[2];
  let language = '';
  for (let at = 3; at < firstRun; at += 4096) {
    language += String.fromCharCode(...words.subarray(at, Math.min(at + 4096, firstRun)));
  }
  const tokens = new Array((words.length - firstRun) / 3);
  let start = 0;
  for (let at = firstRun, index = 0; at < words.length; at += 3, index++) {
    const end = words[at];
    tokens[index] = { text: source.slice(start, end), light: color(words[at + 1]), dark: color(words[at + 2]) };
    start = end;
  }
  return { language, plain: words[1] !== 0, tokens };
}
