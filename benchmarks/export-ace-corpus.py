#!/usr/bin/env python3
"""Read saved assistant Markdown from Ace SQLite; keep corpus text outside the repo."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import sqlite3

parser = argparse.ArgumentParser()
parser.add_argument('--db', type=Path, default=Path.home() / '.ace/events.sqlite')
parser.add_argument('--workspace', required=True, help='Exact Ace workspace path')
parser.add_argument('--threads', type=int, default=200)
parser.add_argument('--out', type=Path, required=True, help='Private directory outside the checkout')
args = parser.parse_args()
if args.threads < 1:
    parser.error("--threads must be positive")
repo = Path(__file__).resolve().parent.parent
if args.out.resolve().is_relative_to(repo):
    raise SystemExit('Export private transcript text outside the repository')
c = sqlite3.connect(args.db.resolve().as_uri() + '?mode=ro', uri=True)
c.execute('BEGIN')
w = c.execute('SELECT id FROM workspaces WHERE path=?', (args.workspace,)).fetchone()
if not w:
    raise SystemExit('Workspace not found')
threads = c.execute('SELECT id FROM threads WHERE workspace_id=? ORDER BY updated_at DESC,id LIMIT ?', (w[0], args.threads)).fetchall()
messages = []
for (thread,) in threads:
    rows = c.execute("SELECT json_extract(item,'$.parts') FROM items WHERE thread_id=? AND json_extract(item,'$.type')='message' AND json_extract(item,'$.role')='assistant' AND json_extract(item,'$.complete')=1 ORDER BY created_seq", (thread,))
    for (parts,) in rows:
        pieces = []
        for part in json.loads(parts or '[]'):
            if part.get('type') != 'text':
                continue
            text = part.get('text', '')
            source = part.get('source')
            if source:
                chunks = c.execute('SELECT bytes FROM item_source_chunks WHERE stream_id=? AND offset < ? ORDER BY offset', (source['streamId'], source['bytes'])).fetchall()
                raw = b''.join(bytes(row[0]) for row in chunks)[:source['bytes']]
                if len(raw) != source['bytes']:
                    raise RuntimeError('Incomplete stored message text')
                text = raw.decode('utf-16le' if source.get('encoding') == 'utf-16le' else 'utf-8')
            pieces.append(text)
        text = '\n\n'.join(pieces)
        if text.strip():
            messages.append(text)
c.rollback(); c.close()
message_count = len(messages)
messages = list(dict.fromkeys(messages))
if not messages:
    raise SystemExit('No completed assistant Markdown found')
messages.sort(key=lambda text: (len(text.encode()), hashlib.sha256(text.encode()).hexdigest()))
fixtures = {}
for percentile in [0, 10, 25, 50, 75, 90, 95, 99, 100]:
    fixtures[f'real_p{percentile:03}'] = messages[(len(messages)-1)*percentile//100]
# Full messages selected for actual syntax-heavy use cases.
fixtures['real_most_fences'] = max(messages, key=lambda text: len(re.findall(r'^\s*(```|~~~)', text, re.M)))
fixtures['real_most_table_rows'] = max(messages, key=lambda text: len(re.findall(r'^\|.*\|\s*$', text, re.M)))
fixtures['real_most_list_items'] = max(messages, key=lambda text: len(re.findall(r'^\s*(?:[-*+] |\d+\. )', text, re.M)))
fence = re.compile(r'(?m)^(`{3,}|~{3,})[^\n]*\n[\s\S]*?^\1[ \t]*(?=\n|$)')
fences = sorted((match.group() for text in messages for match in fence.finditer(text)), key=len)
if fences:
    fixtures['code_real_largest_fence'] = fences[-1]
    fixtures['code_real_median_fence'] = fences[len(fences)//2]
tables = [match.group() for text in messages for match in re.finditer(r'(?m)(?:^\|.*\|[ \t]*(?:\n|$)){2,}',text)]
if tables:
    fixtures['table_real_largest'] = max(tables,key=len)
args.out.mkdir(parents=True, exist_ok=True, mode=0o700)
path = args.out / 'fixtures.json'
path.write_text(json.dumps(fixtures, ensure_ascii=False)); path.chmod(0o600)
summary = {
    'threadsExamined': len(threads), 'assistantMessagesBeforeDedup': message_count, 'uniqueAssistantMessages': len(messages),
    'selection': 'Completed assistant messages from latest workspace threads; byte-size quantiles and syntax-heavy full messages; exact extracted code fences',
    'populationBytes': {'min': len(messages[0].encode()), 'median': len(messages[len(messages)//2].encode()), 'max': len(messages[-1].encode())},
    'cases': {name: {'sourceBytes': len(text.encode()), 'lines': text.count('\n')+1, 'fenceMarkers': len(re.findall(r'^\s*(```|~~~)',text,re.M)), 'sha256': hashlib.sha256(text.encode()).hexdigest()} for name,text in fixtures.items()}
}
(args.out / 'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
print(json.dumps({'threadsExamined':len(threads),'uniqueAssistantMessages':len(messages),'cases':len(fixtures),'populationBytes':summary['populationBytes']}))
